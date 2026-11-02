//! PostgreSQL driver on sqlx: pool, binding, reading rows into cells, streams.

use super::build::Sql;
use super::decode::{Arg, Cell, Col, decode_row};
use super::{OnError, PoolLimits};
use crate::action::{Error, Result};
use crate::model::Entity;
use futures_util::TryStreamExt;
use sqlx::postgres::{PgArguments, PgConnectOptions, PgConnection, PgPool, PgPoolOptions, PgRow};
use sqlx::query::Query;
use sqlx::{AssertSqlSafe, Row};
use std::str::FromStr;

/// Opens a bounded pool. Connections are made lazily, so the app starts even when the database is
/// down; health checks and the first query report it.
pub(crate) async fn connect(url: &str, limits: &PoolLimits) -> Result<PgPool> {
    let mut options = PgConnectOptions::from_str(url)
        .map_err(|_| Error::Internal("database.url is not a valid PostgreSQL URL".into()))?;
    if let Some(t) = limits.statement_timeout {
        options = options.options([("statement_timeout", t.as_millis().to_string())]);
    }
    Ok(PgPoolOptions::new()
        .max_connections(limits.max_connections)
        .min_connections(limits.min_connections)
        .acquire_timeout(limits.acquire_timeout)
        .idle_timeout(limits.idle_timeout)
        .connect_lazy_with(options))
}

/// A prepared, cached statement with its parameters bound. Values never enter the SQL text.
fn query(text: String, args: Vec<Arg>) -> Query<'static, sqlx::Postgres, PgArguments> {
    let mut q = sqlx::query(AssertSqlSafe(text));
    for a in args {
        q = match a {
            Arg::Int(n) => q.bind(n),
            Arg::Text(s) => q.bind(s),
            Arg::Bool(b) => q.bind(b),
            Arg::Null(Col::Int) => q.bind(None::<i64>),
            Arg::Null(Col::Bool) => q.bind(None::<bool>),
            Arg::Null(Col::Float) => q.bind(None::<f64>),
            Arg::Null(Col::Text) => q.bind(None::<String>),
        };
    }
    q
}

fn read(row: &PgRow, cols: &[Col], out: &mut Vec<Cell>) -> Result<()> {
    for (i, col) in cols.iter().enumerate() {
        let cell = match col {
            Col::Int => row.try_get::<Option<i64>, _>(i).map(|v| v.map_or(Cell::Null, Cell::Int)),
            Col::Float => row.try_get::<Option<f64>, _>(i).map(|v| v.map_or(Cell::Null, Cell::Float)),
            Col::Text => row.try_get::<Option<String>, _>(i).map(|v| v.map_or(Cell::Null, Cell::Text)),
            Col::Bool => row.try_get::<Option<bool>, _>(i).map(|v| v.map_or(Cell::Null, Cell::Bool)),
        };
        out.push(cell.map_err(|e| Error::Internal(format!("reading column {i}: {e}")))?);
    }
    Ok(())
}

/// Maps a driver error: `23505` (unique violation) carries its constraint name.
fn error(e: sqlx::Error, on: OnError) -> Error {
    if let sqlx::Error::Database(db) = &e
        && db.code().as_deref() == Some("23505")
    {
        return on(Some(db.constraint().unwrap_or_default()), e.to_string());
    }
    on(None, e.to_string())
}

pub(crate) async fn fetch(
    conn: &mut PgConnection,
    sql: Sql,
    on: OnError,
    each: &mut (dyn FnMut(&mut Vec<Cell>) -> Result<()> + Send),
) -> Result<()> {
    let Sql { text, args, cols } = sql;
    let mut rows = query(text, args).fetch(&mut *conn);
    let mut cells = Vec::with_capacity(cols.len());
    while let Some(row) = rows.try_next().await.map_err(|e| error(e, on))? {
        cells.clear();
        read(&row, &cols, &mut cells)?;
        each(&mut cells)?;
    }
    Ok(())
}

pub(crate) async fn execute(conn: &mut PgConnection, sql: Sql, on: OnError) -> Result<u64> {
    let Sql { text, args, .. } = sql;
    query(text, args).execute(&mut *conn).await.map(|r| r.rows_affected()).map_err(|e| error(e, on))
}

/// Streams rows into a bounded channel from a task of its own. The task stops when the reader
/// drops the stream, which also ends the query.
pub(crate) fn stream<E: Entity>(pool: PgPool, sql: Sql, tx: tokio::sync::mpsc::Sender<Result<E>>) {
    tokio::spawn(async move {
        let Sql { text, args, cols } = sql;
        let mut conn = match pool.acquire().await {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(Err(Error::Internal(format!("database connection: {e}")))).await;
                return;
            }
        };
        let mut rows = query(text, args).fetch(&mut *conn);
        let mut cells = Vec::with_capacity(cols.len());
        loop {
            let item = match rows.try_next().await {
                Ok(None) => return,
                Ok(Some(row)) => {
                    cells.clear();
                    read(&row, &cols, &mut cells).and_then(|_| decode_row::<E>(&mut cells))
                }
                Err(e) => Err(Error::Internal(e.to_string())),
            };
            let failed = item.is_err();
            if tx.send(item).await.is_err() || failed {
                return;
            }
        }
    });
}
