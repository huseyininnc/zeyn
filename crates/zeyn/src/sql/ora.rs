//! Oracle driver on the `oracle` crate (ODPI-C, Oracle Instant Client).
//!
//! The driver is blocking, so every call runs on Tokio's blocking threads and never stalls the
//! async runtime. The connection pool is bounded by `max_connections`; a call takes a connection
//! into a blocking task and gets it back when the task ends.

use super::build::{self, Oracle, OracleCompat, Sql};
use super::decode::{Arg, Cell, Col, decode_row};
use super::{OnError, PoolLimits};
use crate::action::{Error, Result};
use crate::model::Entity;
use crate::value::Value;
use oracle::pool::{GetMode, Pool, PoolBuilder};
use oracle::sql_type::{OracleType, ToSql};
use oracle::{Connection, Row};
use std::time::Duration;

/// Rows fetched from the server per round trip.
const PREFETCH: u32 = 128;

/// A pool with the settings every connection taken from it gets.
#[derive(Clone)]
pub(crate) struct OraPool {
    pool: Pool,
    statement_timeout: Option<Duration>,
}

/// Where to connect, read from `oracle://user:password@host:1521/service?compat=11`.
#[derive(Debug, PartialEq)]
struct Target {
    user: String,
    password: String,
    connect: String,
    compat: Option<OracleCompat>,
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (bytes[i], bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (b, ..) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_url(url: &str) -> Result<Target> {
    let bad = || Error::Internal("database.url is not a valid Oracle URL (oracle://user:password@host:1521/service)".into());
    let rest = url.strip_prefix("oracle://").ok_or_else(bad)?;
    let (creds, rest) = rest.rsplit_once('@').ok_or_else(bad)?;
    let (user, password) = creds.split_once(':').ok_or_else(bad)?;
    let (connect, query) = rest.split_once('?').unwrap_or((rest, ""));
    let mut compat = None;
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        compat = Some(match pair {
            "compat=11" => OracleCompat::V11,
            "compat=12" => OracleCompat::V12,
            _ => return Err(Error::Internal(format!("unknown Oracle URL option `{pair}`; expected compat=11 or compat=12"))),
        });
    }
    if user.is_empty() || connect.is_empty() {
        return Err(bad());
    }
    Ok(Target { user: percent_decode(user), password: percent_decode(password), connect: connect.to_string(), compat })
}

/// Runs blocking driver code on Tokio's blocking threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f).await.map_err(|e| Error::Internal(format!("database task: {e}")))?
}

fn internal(e: oracle::Error) -> Error {
    Error::Internal(e.to_string())
}

/// Maps a driver error: ORA-00001 (unique violation) carries the constraint name from
/// `unique constraint (SCHEMA.NAME) violated`.
fn error(e: oracle::Error, on: OnError) -> Error {
    if let Some(db) = e.db_error()
        && db.code() == 1
    {
        let name = db.message().split_once('(').and_then(|(_, r)| r.split_once(')')).map(|(n, _)| n.rsplit('.').next().unwrap_or(n));
        return on(Some(name.unwrap_or_default()), e.to_string());
    }
    on(None, e.to_string())
}

/// Opens a bounded pool. The database version decides the SQL (11g or 12c and later) unless
/// the URL sets `compat`; without `compat` the database must be reachable at startup.
/// `ZEYN_ORACLE_CLIENT_DIR` names the Instant Client folder when it is not the first one on
/// `PATH` (Windows) or the library path.
pub(crate) async fn connect(url: &str, limits: &PoolLimits) -> Result<(OraPool, Oracle)> {
    let target = parse_url(url)?;
    let limits = limits.clone();
    blocking(move || {
        if let Ok(dir) = std::env::var("ZEYN_ORACLE_CLIENT_DIR")
            && !dir.is_empty()
        {
            oracle::InitParams::new().oracle_client_lib_dir(dir).and_then(|p| p.init()).map_err(internal)?;
        }
        let mut builder = PoolBuilder::new(&target.user, &target.password, &target.connect);
        builder
            .max_connections(limits.max_connections)
            .min_connections(limits.min_connections)
            .get_mode(GetMode::TimedWait(limits.acquire_timeout))
            .stmt_cache_size(64);
        builder.timeout(limits.idle_timeout).map_err(internal)?;
        let pool = builder.build().map_err(|e| Error::Internal(format!("database connection: {e}")))?;
        let compat = match target.compat {
            Some(c) => c,
            None => {
                let conn = pool.get().map_err(internal)?;
                let (version, _) = conn.server_version().map_err(internal)?;
                if version.major() < 12 { OracleCompat::V11 } else { OracleCompat::V12 }
            }
        };
        Ok((OraPool { pool, statement_timeout: limits.statement_timeout }, Oracle { compat }))
    })
    .await
}

/// A connection for one call (autocommit) or for a transaction.
pub(crate) struct OraLink {
    conn: Option<Connection>,
    tx: bool,
    compat: OracleCompat,
}

impl OraLink {
    pub(crate) async fn acquire(pool: &OraPool, compat: OracleCompat, tx: bool) -> Result<OraLink> {
        let pool = pool.clone();
        let conn = blocking(move || {
            let mut conn = pool.pool.get().map_err(|e| Error::Internal(format!("database connection: {e}")))?;
            conn.set_autocommit(!tx);
            conn.set_call_timeout(pool.statement_timeout).map_err(internal)?;
            Ok(conn)
        })
        .await?;
        Ok(OraLink { conn: Some(conn), tx, compat })
    }

    pub(crate) fn compat(&self) -> OracleCompat {
        self.compat
    }

    /// Moves the connection into a blocking task for `f` and takes it back.
    async fn with<T: Send + 'static>(&mut self, f: impl FnOnce(&Connection) -> Result<T> + Send + 'static) -> Result<T> {
        let conn = self.conn.take().ok_or_else(|| Error::Internal("the database connection was lost".into()))?;
        let (conn, result) = tokio::task::spawn_blocking(move || {
            let result = f(&conn);
            (conn, result)
        })
        .await
        .map_err(|e| Error::Internal(format!("database task: {e}")))?;
        self.conn = Some(conn);
        result
    }

    /// Runs a query. A page is at most `MAX_PAGE_SIZE + 1` rows, so its cells are collected in the
    /// blocking task and handed over at once.
    pub(crate) async fn fetch(
        &mut self,
        sql: Sql,
        on: OnError,
        each: &mut (dyn FnMut(&mut Vec<Cell>) -> Result<()> + Send),
    ) -> Result<()> {
        let rows = self.with(move |c| query_rows(c, sql, on)).await?;
        for mut cells in rows {
            each(&mut cells)?;
        }
        Ok(())
    }

    pub(crate) async fn execute(&mut self, sql: Sql, on: OnError) -> Result<u64> {
        self.with(move |c| execute(c, sql, on)).await
    }

    /// Inserts a row and returns its key: from the table's sequence on 11g, from the identity
    /// column through `RETURNING ... INTO` on 12c and later, or the key the row already has.
    pub(crate) async fn insert<E: Entity>(&mut self, d: &Oracle, row: &E, on: OnError) -> Result<Value> {
        let (ki, kf) = E::fields().iter().enumerate().find(|(_, f)| f.key).ok_or_else(|| Error::Internal("entity has no key".into()))?;
        let key = row.value_at(ki).unwrap_or(Value::Null);
        if !build::is_new_key(&key) {
            self.execute(build::plan_insert(d, row, None, false)?, on).await?;
            return Ok(key);
        }
        match self.compat {
            OracleCompat::V11 => {
                let seq = build::ident(d, &Oracle::sequence(E::TABLE))?;
                let next = Sql { text: format!("SELECT {seq}.NEXTVAL AS c0 FROM DUAL"), args: Vec::new(), cols: vec![Col::Int] };
                let mut id = None;
                self.fetch(next, on, &mut |cells| {
                    if let Some(Cell::Int(n)) = cells.drain(..).next() {
                        id = Some(n);
                    }
                    Ok(())
                })
                .await?;
                let id = id.ok_or_else(|| Error::Internal("the key sequence returned nothing".into()))?;
                self.execute(build::plan_insert(d, row, Some(Arg::Int(id)), false)?, on).await?;
                Ok(Value::Int(id))
            }
            OracleCompat::V12 => {
                let mut sql = build::plan_insert(d, row, None, false)?;
                sql.push(" RETURNING ");
                sql.push(&build::ident(d, kf.name)?);
                sql.push(" INTO :");
                let out = sql.args.len() + 1;
                sql.push(&out.to_string());
                let id = self
                    .with(move |c| {
                        let Sql { text, args, .. } = sql;
                        let mut values = params(args);
                        values.push(Box::new(OracleType::Int64));
                        let refs: Vec<&dyn ToSql> = values.iter().map(|v| &**v).collect();
                        let stmt = c.execute(&text, &refs).map_err(|e| error(e, on))?;
                        let ids: Vec<i64> = stmt.returned_values(out).map_err(internal)?;
                        ids.first().copied().ok_or_else(|| Error::Internal("insert returned no key".into()))
                    })
                    .await?;
                Ok(Value::Int(id))
            }
        }
    }

    pub(crate) async fn commit(mut self) -> Result<()> {
        self.tx = false;
        self.with(|c| c.commit().map_err(|e| Error::Internal(format!("commit: {e}")))).await
    }

    pub(crate) async fn rollback(mut self) -> Result<()> {
        self.tx = false;
        self.with(|c| c.rollback().map_err(|e| Error::Internal(format!("rollback: {e}")))).await
    }
}

/// A transaction dropped without commit is rolled back before its connection goes back to the
/// pool, on a blocking thread when a runtime is there.
impl Drop for OraLink {
    fn drop(&mut self) {
        if !self.tx {
            return;
        }
        if let Some(conn) = self.conn.take() {
            let rollback = move || {
                let _ = conn.rollback();
            };
            match tokio::runtime::Handle::try_current() {
                Ok(rt) => {
                    rt.spawn_blocking(rollback);
                }
                Err(_) => rollback(),
            }
        }
    }
}

/// Bound values. Yes/no is a number in Oracle; a NULL keeps its type.
fn params(args: Vec<Arg>) -> Vec<Box<dyn ToSql>> {
    args.into_iter()
        .map(|a| -> Box<dyn ToSql> {
            match a {
                Arg::Int(n) => Box::new(n),
                Arg::Text(s) => Box::new(s),
                Arg::Bool(b) => Box::new(i64::from(b)),
                Arg::Null(Col::Int | Col::Bool) => Box::new(None::<i64>),
                Arg::Null(Col::Float) => Box::new(None::<f64>),
                Arg::Null(Col::Text) => Box::new(None::<String>),
            }
        })
        .collect()
}

fn read(row: &Row, cols: &[Col], out: &mut Vec<Cell>) -> Result<()> {
    for (i, col) in cols.iter().enumerate() {
        let cell = match col {
            Col::Int => row.get::<usize, Option<i64>>(i).map(|v| v.map_or(Cell::Null, Cell::Int)),
            Col::Float => row.get::<usize, Option<f64>>(i).map(|v| v.map_or(Cell::Null, Cell::Float)),
            Col::Text => row.get::<usize, Option<String>>(i).map(|v| v.map_or(Cell::Null, Cell::Text)),
            Col::Bool => row.get::<usize, Option<i64>>(i).map(|v| v.map_or(Cell::Null, |n| Cell::Bool(n != 0))),
        };
        out.push(cell.map_err(|e| Error::Internal(format!("reading column {i}: {e}")))?);
    }
    Ok(())
}

fn query_rows(conn: &Connection, sql: Sql, on: OnError) -> Result<Vec<Vec<Cell>>> {
    let Sql { text, args, cols } = sql;
    let values = params(args);
    let refs: Vec<&dyn ToSql> = values.iter().map(|v| &**v).collect();
    let mut stmt = conn.statement(&text).prefetch_rows(PREFETCH).build().map_err(|e| error(e, on))?;
    let rows = stmt.query(&refs).map_err(|e| error(e, on))?;
    let mut out = Vec::new();
    for row in rows {
        let row = row.map_err(|e| error(e, on))?;
        let mut cells = Vec::with_capacity(cols.len());
        read(&row, &cols, &mut cells)?;
        out.push(cells);
    }
    Ok(out)
}

fn execute(conn: &Connection, sql: Sql, on: OnError) -> Result<u64> {
    let Sql { text, args, .. } = sql;
    let values = params(args);
    let refs: Vec<&dyn ToSql> = values.iter().map(|v| &**v).collect();
    let stmt = conn.execute(&text, &refs).map_err(|e| error(e, on))?;
    stmt.row_count().map_err(internal)
}

/// Streams rows from a blocking thread into a bounded channel; `blocking_send` waits while the
/// reader is behind and fails once it is gone, which ends the query.
pub(crate) fn stream<E: Entity>(pool: OraPool, sql: Sql, tx: tokio::sync::mpsc::Sender<Result<E>>) {
    tokio::task::spawn_blocking(move || {
        let run = || -> Result<()> {
            let mut conn = pool.pool.get().map_err(|e| Error::Internal(format!("database connection: {e}")))?;
            conn.set_autocommit(true);
            let Sql { text, args, cols } = sql;
            let values = params(args);
            let refs: Vec<&dyn ToSql> = values.iter().map(|v| &**v).collect();
            let mut stmt = conn.statement(&text).prefetch_rows(PREFETCH).fetch_array_size(PREFETCH).build().map_err(internal)?;
            let rows = stmt.query(&refs).map_err(internal)?;
            let mut cells = Vec::with_capacity(cols.len());
            for row in rows {
                cells.clear();
                let item = row.map_err(internal).and_then(|row| read(&row, &cols, &mut cells)).and_then(|_| decode_row::<E>(&mut cells));
                let failed = item.is_err();
                if tx.blocking_send(item).is_err() || failed {
                    return Ok(());
                }
            }
            Ok(())
        };
        if let Err(e) = run() {
            let _ = tx.blocking_send(Err(e));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_read_with_encoded_passwords_and_compat() {
        let t = parse_url("oracle://app:p%40ss%3Aw@db.local:1521/XEPDB1?compat=11").unwrap();
        assert_eq!(
            t,
            Target { user: "app".into(), password: "p@ss:w".into(), connect: "db.local:1521/XEPDB1".into(), compat: Some(OracleCompat::V11) }
        );
        assert_eq!(parse_url("oracle://a:b@h/XE").unwrap().compat, None);
        assert!(parse_url("oracle://h/XE").is_err());
        assert!(parse_url("oracle://a:b@h/XE?compat=9").is_err());
        assert!(parse_url("postgres://a:b@h/x").is_err());
    }
}
