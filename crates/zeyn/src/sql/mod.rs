//! Data layer: entities in a SQL database, listed from a [`DataQuery`] and saved with row version
//! checks. Behind the `postgres` and `oracle` Cargo features; apps without a database do not
//! compile it.
//!
//! ```no_run
//! use zeyn::prelude::*;
//! use zeyn::sql::Db;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     #[rule(required, max_len = 20, unique)]
//!     pub code: String,
//!     #[version]
//!     pub version: i64,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(q: DataQuery, db: Db) -> Result<Page<Product>> {
//!     db.find(&q).await
//! }
//!
//! #[action(permission = "stock.product.write")]
//! pub async fn save_product(p: Valid<Product>, db: Db) -> Result<Product> {
//!     if p.id.0 == 0 { db.insert(&*p).await } else { db.update(&*p).await }
//! }
//! ```
//!
//! SQL is built from the entity's fields only: client strings never become identifiers, and
//! every value is a bound parameter. See `docs/database.md`.

mod build;
mod decode;
#[cfg(feature = "oracle")]
mod ora;
#[cfg(feature = "postgres")]
mod pg;

pub use build::{Cast, Dialect, Oracle, OracleCompat, Postgres, Sql, create_table};
pub use decode::{Arg, Cell, Col};

use crate::action::{DataQuery, Error, Page, Result};
use crate::ctx::Service;
use crate::model::{Entity, Model};
use crate::platform::Config;
use crate::value::FieldType;
use build::{FindPlan, Shape};
use std::sync::Arc;
use std::time::Duration;

/// Rows a [`RowStream`] buffers ahead of its reader. Memory stays bounded whatever the result size.
pub const STREAM_BUFFER: usize = 64;

/// Largest total a `find` counts by default; beyond it `Page::total` is this number.
pub const DEFAULT_COUNT_CAP: u64 = 1_000_000;

/// Connection pool limits.
///
/// Read from [`Config`] by [`Db::from_config`]:
///
/// | Key | Default |
/// |---|---|
/// | `database.url` | required |
/// | `database.max_connections` | 10 |
/// | `database.min_connections` | 0 |
/// | `database.acquire_timeout_ms` | 5000 |
/// | `database.idle_timeout_s` | 600 |
/// | `database.statement_timeout_ms` | 30000 (0 turns it off) |
/// | `database.count_cap` | 1000000 |
#[derive(Clone, Debug, PartialEq)]
pub struct PoolLimits {
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Duration,
    /// Longest a statement may run; `None` keeps the database default.
    pub statement_timeout: Option<Duration>,
    /// Largest total counted for `with_total`.
    pub count_cap: u64,
}

impl Default for PoolLimits {
    fn default() -> Self {
        PoolLimits {
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(600),
            statement_timeout: Some(Duration::from_secs(30)),
            count_cap: DEFAULT_COUNT_CAP,
        }
    }
}

impl PoolLimits {
    /// Limits from `database.*` keys, defaults for missing ones. A value that is not a number is
    /// an error naming the key.
    pub fn from_config(config: &Config) -> Result<PoolLimits> {
        fn num(config: &Config, key: &str, default: u64) -> Result<u64> {
            match config.get(key) {
                None => Ok(default),
                Some(v) => v.trim().parse().map_err(|_| {
                    Error::Internal(format!("configuration `{key}` is not a whole number"))
                }),
            }
        }
        let d = PoolLimits::default();
        let max = num(
            config,
            "database.max_connections",
            u64::from(d.max_connections),
        )?
        .clamp(1, 1000) as u32;
        let statement = num(config, "database.statement_timeout_ms", 30_000)?;
        Ok(PoolLimits {
            max_connections: max,
            min_connections: (num(config, "database.min_connections", 0)? as u32).min(max),
            acquire_timeout: Duration::from_millis(num(
                config,
                "database.acquire_timeout_ms",
                5_000,
            )?),
            idle_timeout: Duration::from_secs(num(config, "database.idle_timeout_s", 600)?),
            statement_timeout: (statement > 0).then(|| Duration::from_millis(statement)),
            count_cap: num(config, "database.count_cap", DEFAULT_COUNT_CAP)?.max(1),
        })
    }
}

enum Backend {
    #[cfg(feature = "postgres")]
    Postgres(sqlx::PgPool),
    #[cfg(feature = "oracle")]
    Oracle(ora::OraPool, OracleCompat),
}

struct Inner {
    backend: Backend,
    dialect: Box<dyn Dialect>,
    count_cap: u64,
}

/// A database: a bounded connection pool and its dialect. Cheap to clone; register it once with
/// `App::service` and take it as an action argument.
#[derive(Clone)]
pub struct Db {
    inner: Arc<Inner>,
}

impl Service for Db {}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Db({})", self.inner.dialect.name())
    }
}

/// A connection for one call, or the connection of a transaction.
pub(crate) enum Link {
    #[cfg(feature = "postgres")]
    Pg(sqlx::pool::PoolConnection<sqlx::Postgres>),
    #[cfg(feature = "postgres")]
    PgTx(sqlx::Transaction<'static, sqlx::Postgres>),
    #[cfg(feature = "oracle")]
    Ora(ora::OraLink),
}

/// How a driver error becomes a Zeyn error: unique violations name the field of `M`.
pub(crate) type OnError = fn(Option<&str>, String) -> Error;

fn on_error<M: Model>(unique: Option<&str>, detail: String) -> Error {
    match unique {
        Some(constraint) => build::unique_error::<M>(Some(constraint)),
        None => Error::Internal(detail),
    }
}

/// Errors of hand written SQL: a unique violation belongs to the whole record.
fn raw_error(unique: Option<&str>, detail: String) -> Error {
    match unique {
        Some(_) => build::duplicate_error(),
        None => Error::Internal(detail),
    }
}

impl Db {
    /// Connects to `url` with the given limits. The password is never logged or put in errors.
    ///
    /// - `postgres://user:password@host:5432/db` (feature `postgres`)
    /// - `oracle://user:password@host:1521/service` (feature `oracle`); add `?compat=11` or
    ///   `?compat=12` to choose the SQL without asking the database for its version
    pub async fn connect(url: &str, limits: PoolLimits) -> Result<Db> {
        let scheme = url
            .split("://")
            .next()
            .unwrap_or_default();
        let (backend, dialect): (Backend, Box<dyn Dialect>) = match scheme {
            #[cfg(feature = "postgres")]
            "postgres" | "postgresql" => (
                Backend::Postgres(pg::connect(url, &limits).await?),
                Box::new(Postgres),
            ),
            #[cfg(feature = "oracle")]
            "oracle" => {
                let (pool, dialect) = ora::connect(url, &limits).await?;
                (Backend::Oracle(pool, dialect.compat), Box::new(dialect))
            }
            _ => {
                return Err(Error::Internal(format!(
                    "no database driver for `{scheme}://` URLs; enable the matching Zeyn feature"
                )));
            }
        };
        Ok(Db {
            inner: Arc::new(Inner {
                backend,
                dialect,
                count_cap: limits.count_cap,
            }),
        })
    }

    /// Connects with `database.url` and the limits in `database.*` (see [`PoolLimits`]).
    pub async fn from_config(config: &Config) -> Result<Db> {
        let url = config.secret("database.url")?;
        Db::connect(url.expose(), PoolLimits::from_config(config)?).await
    }

    pub fn dialect(&self) -> &dyn Dialect {
        &*self.inner.dialect
    }

    async fn link(&self) -> Result<Link> {
        match &self.inner.backend {
            #[cfg(feature = "postgres")]
            Backend::Postgres(pool) => pool
                .acquire()
                .await
                .map(Link::Pg)
                .map_err(|e| Error::Internal(format!("database connection: {e}"))),
            #[cfg(feature = "oracle")]
            Backend::Oracle(pool, compat) => ora::OraLink::acquire(pool, *compat, false)
                .await
                .map(Link::Ora),
        }
    }

    /// One page of entities matching the query: filters, sort, offset or keyset paging, a capped
    /// total when asked, totals and groups. See [`Page`].
    pub async fn find<E: Entity>(&self, q: &DataQuery) -> Result<Page<E>> {
        self.link()
            .await?
            .find(self.dialect(), q, self.inner.count_cap)
            .await
    }

    /// The entity with this key, or [`Error::NotFound`].
    pub async fn get<E: Entity>(&self, key: impl FieldType) -> Result<E> {
        self.link()
            .await?
            .get(self.dialect(), key)
            .await
    }

    /// Inserts a new entity and returns it as stored (generated key, version 1, defaults).
    pub async fn insert<E: Entity>(&self, row: &E) -> Result<E> {
        self.link()
            .await?
            .insert(self.dialect(), row)
            .await
    }

    /// Updates an entity when its row version is still the stored one, and returns it with the new
    /// version. [`Error::Conflict`] when someone else saved it first (or it was deleted).
    pub async fn update<E: Entity>(&self, row: &E) -> Result<E> {
        self.link()
            .await?
            .update(self.dialect(), row)
            .await
    }

    /// Deletes the entity with this key; [`Error::NotFound`] when there is none.
    pub async fn delete<E: Entity>(&self, key: impl FieldType) -> Result<()> {
        self.link()
            .await?
            .delete::<E>(self.dialect(), key)
            .await
    }

    /// Rows of a statement built inside the crate from fixed text and checked identifiers, such as
    /// the account store's. Values arrive only as `args`.
    #[cfg(any(feature = "accounts", feature = "server"))]
    pub(crate) async fn rows(&self, sql: Sql) -> Result<Vec<Vec<Cell>>> {
        let mut rows = Vec::new();
        self.link()
            .await?
            .fetch(sql, raw_error, &mut |cells| {
                rows.push(std::mem::take(cells));
                Ok(())
            })
            .await?;
        Ok(rows)
    }

    /// Runs a statement built inside the crate (see [`Db::rows`]) and returns the changed rows.
    #[cfg(any(feature = "accounts", feature = "server"))]
    pub(crate) async fn run(&self, sql: Sql) -> Result<u64> {
        self.link()
            .await?
            .execute(sql, raw_error)
            .await
    }

    /// Runs a statement written by hand and returns the number of rows it changed. The SQL must
    /// be a literal, so values can only arrive as `args` (`$1`, `$2` in PostgreSQL).
    pub async fn execute(&self, sql: &'static str, args: Vec<Arg>) -> Result<u64> {
        self.link()
            .await?
            .execute(
                Sql {
                    text: sql.to_string(),
                    args,
                    cols: Vec::new(),
                },
                raw_error,
            )
            .await
    }

    /// Starts a transaction. Writes through it are committed together by [`Tx::commit`]; dropping
    /// it without a commit rolls everything back.
    pub async fn begin(&self) -> Result<Tx> {
        let link = match &self.inner.backend {
            #[cfg(feature = "postgres")]
            Backend::Postgres(pool) => Link::PgTx(
                pool.begin()
                    .await
                    .map_err(|e| Error::Internal(format!("begin: {e}")))?,
            ),
            #[cfg(feature = "oracle")]
            Backend::Oracle(pool, compat) => {
                Link::Ora(ora::OraLink::acquire(pool, *compat, true).await?)
            }
        };
        Ok(Tx {
            db: self.clone(),
            link,
        })
    }

    /// Every entity matching the query's filters, in its order, without paging, as a stream.
    /// Rows are read while the stream is consumed, at most [`STREAM_BUFFER`] ahead, so an export
    /// of millions of rows holds only a few in memory. Dropping the stream stops the query.
    pub fn stream<E: Entity>(&self, q: &DataQuery) -> RowStream<E> {
        let (tx, rx) = tokio::sync::mpsc::channel(STREAM_BUFFER);
        match build::plan_stream::<E>(self.dialect(), q) {
            Err(e) => {
                let _ = tx.try_send(Err(e));
            }
            Ok(sql) => match &self.inner.backend {
                #[cfg(feature = "postgres")]
                Backend::Postgres(pool) => pg::stream::<E>(pool.clone(), sql, tx),
                #[cfg(feature = "oracle")]
                Backend::Oracle(pool, _) => ora::stream::<E>(pool.clone(), sql, tx),
            },
        }
        RowStream { rx }
    }

    /// Creates the entity's table (see [`create_table`]). For scaffolding and tests.
    pub async fn create_table<E: Entity>(&self) -> Result<()> {
        for statement in create_table::<E>(self.dialect())? {
            self.link()
                .await?
                .execute(
                    Sql {
                        text: statement,
                        args: Vec::new(),
                        cols: Vec::new(),
                    },
                    on_error::<E>,
                )
                .await?;
        }
        Ok(())
    }

    /// Runs pending migrations, such as the ones `sqlx::migrate!("./migrations")` embeds.
    /// PostgreSQL only.
    #[cfg(feature = "postgres")]
    pub async fn migrate(&self, migrator: &sqlx::migrate::Migrator) -> Result<()> {
        match &self.inner.backend {
            Backend::Postgres(pool) => migrator
                .run(pool)
                .await
                .map_err(|e| Error::Internal(format!("migration: {e}"))),
            #[cfg(feature = "oracle")]
            Backend::Oracle(..) => Err(Error::Internal(
                "sqlx migrations run on PostgreSQL only".into(),
            )),
        }
    }

    /// Runs the migrations in a folder read at run time (`0001_create_products.sql`, ...).
    #[cfg(feature = "postgres")]
    pub async fn migrate_dir(&self, dir: impl AsRef<std::path::Path>) -> Result<()> {
        let migrator = sqlx::migrate::Migrator::new(dir.as_ref())
            .await
            .map_err(|e| Error::Internal(format!("migration: {e}")))?;
        self.migrate(&migrator).await
    }
}

/// Checks a table prefix for the crate's own tables: lower case letters, digits and `_`, at most
/// 16 characters, not starting with a digit.
#[cfg(any(feature = "accounts", feature = "server"))]
pub(crate) fn checked_prefix(prefix: &str) -> Result<String> {
    let ok = prefix.len() <= 16
        && prefix
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !prefix.starts_with(|c: char| c.is_ascii_digit());
    if ok {
        Ok(prefix.to_string())
    } else {
        Err(Error::Internal(
            "a table prefix may hold lower case letters, digits and `_`, at most 16".into(),
        ))
    }
}

/// SQL written inside the crate: `{p}` becomes the checked table prefix and each `?` the
/// dialect's placeholder. Values arrive only as `args`.
#[cfg(any(feature = "accounts", feature = "server"))]
pub(crate) fn bind(
    d: &dyn Dialect,
    prefix: &str,
    template: &str,
    args: Vec<Arg>,
    cols: &[Col],
) -> Sql {
    let template = template.replace("{p}", prefix);
    let mut text = String::with_capacity(template.len() + 16);
    let mut n = 0;
    for ch in template.chars() {
        if ch == '?' {
            n += 1;
            d.param(&mut text, n, Cast::Plain);
        } else {
            text.push(ch);
        }
    }
    Sql {
        text,
        args,
        cols: cols.to_vec(),
    }
}

/// A transaction: the same calls as [`Db`], on one connection, committed together.
pub struct Tx {
    db: Db,
    link: Link,
}

impl Tx {
    pub async fn find<E: Entity>(&mut self, q: &DataQuery) -> Result<Page<E>> {
        self.link
            .find(&*self.db.inner.dialect, q, self.db.inner.count_cap)
            .await
    }

    pub async fn get<E: Entity>(&mut self, key: impl FieldType) -> Result<E> {
        self.link
            .get(&*self.db.inner.dialect, key)
            .await
    }

    pub async fn insert<E: Entity>(&mut self, row: &E) -> Result<E> {
        self.link
            .insert(&*self.db.inner.dialect, row)
            .await
    }

    pub async fn update<E: Entity>(&mut self, row: &E) -> Result<E> {
        self.link
            .update(&*self.db.inner.dialect, row)
            .await
    }

    pub async fn delete<E: Entity>(&mut self, key: impl FieldType) -> Result<()> {
        self.link
            .delete::<E>(&*self.db.inner.dialect, key)
            .await
    }

    pub async fn execute(&mut self, sql: &'static str, args: Vec<Arg>) -> Result<u64> {
        self.link
            .execute(
                Sql {
                    text: sql.to_string(),
                    args,
                    cols: Vec::new(),
                },
                raw_error,
            )
            .await
    }

    /// Rows of a statement built inside the crate, on this transaction (see [`Db::rows`]).
    #[cfg(feature = "server")]
    pub(crate) async fn rows(&mut self, sql: Sql) -> Result<Vec<Vec<Cell>>> {
        let mut rows = Vec::new();
        self.link
            .fetch(sql, raw_error, &mut |cells| {
                rows.push(std::mem::take(cells));
                Ok(())
            })
            .await?;
        Ok(rows)
    }

    /// Runs a statement built inside the crate on this transaction and returns the changed rows.
    #[cfg(feature = "server")]
    pub(crate) async fn run(&mut self, sql: Sql) -> Result<u64> {
        self.link.execute(sql, raw_error).await
    }

    pub async fn commit(self) -> Result<()> {
        self.link.commit().await
    }

    pub async fn rollback(self) -> Result<()> {
        self.link.rollback().await
    }
}

/// Entities read one by one from the database. See [`Db::stream`].
pub struct RowStream<E> {
    rx: tokio::sync::mpsc::Receiver<Result<E>>,
}

impl<E> RowStream<E> {
    /// The next entity, or `None` at the end. An error ends the stream.
    pub async fn next(&mut self) -> Option<Result<E>> {
        self.rx.recv().await
    }
}

impl<E> futures_core::Stream for RowStream<E> {
    type Item = Result<E>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

impl Link {
    async fn find<E: Entity>(
        &mut self,
        d: &dyn Dialect,
        q: &DataQuery,
        cap: u64,
    ) -> Result<Page<E>> {
        let FindPlan {
            main,
            limit,
            skip,
            shape,
            count,
            totals,
        } = build::plan_find::<E>(d, q, cap)?;
        let fields = E::fields();
        let mut page = Page::default();
        match shape {
            Shape::Rows(keys) => {
                let mut items: Vec<E> = Vec::with_capacity(limit + 1);
                self.fetch(main, on_error::<E>, &mut |cells| {
                    items.push(decode::decode_row::<E>(cells)?);
                    Ok(())
                })
                .await?;
                if items.len() > limit {
                    items.truncate(limit);
                    if let Some(last) = items.last() {
                        page.next = Some(build::next_cursor(d, &keys, last, skip, limit));
                    }
                }
                page.items = items;
            }
            Shape::Groups {
                field,
                aggs,
                offset,
            } => {
                let kind = &fields[field].kind;
                let money = matches!(kind, crate::value::FieldKind::Money);
                let mut groups = Vec::with_capacity(limit + 1);
                self.fetch(main, on_error::<E>, &mut |cells| {
                    let mut it = cells.drain(..);
                    let first = it.next().unwrap_or(Cell::Null);
                    let currency = if money { it.next() } else { None };
                    let value = decode::value_of(kind, first, currency);
                    let count = match it.next() {
                        Some(Cell::Int(n)) => u64::try_from(n).unwrap_or(0),
                        _ => 0,
                    };
                    let totals = build::read_aggs(fields, &aggs, &mut it);
                    groups.push(crate::action::Group {
                        value,
                        count,
                        totals,
                    });
                    Ok(())
                })
                .await?;
                if groups.len() > limit {
                    groups.truncate(limit);
                    page.next = Some(build::group_cursor(offset + limit as u64));
                }
                page.groups = groups;
            }
        }
        if let Some(count) = count {
            let mut total = 0;
            self.fetch(count, on_error::<E>, &mut |cells| {
                if let Some(Cell::Int(n)) = cells.drain(..).next() {
                    total = u64::try_from(n).unwrap_or(0);
                }
                Ok(())
            })
            .await?;
            page.total = Some(total);
        }
        if let Some((sql, aggs)) = totals {
            let mut out = Vec::new();
            self.fetch(sql, on_error::<E>, &mut |cells| {
                out = build::read_aggs(fields, &aggs, &mut cells.drain(..));
                Ok(())
            })
            .await?;
            page.totals = out;
        }
        Ok(page)
    }

    async fn one<E: Entity>(&mut self, sql: Sql) -> Result<Option<E>> {
        let mut row = None;
        self.fetch(sql, on_error::<E>, &mut |cells| {
            row = Some(decode::decode_row::<E>(cells)?);
            Ok(())
        })
        .await?;
        Ok(row)
    }

    async fn get<E: Entity>(&mut self, d: &dyn Dialect, key: impl FieldType) -> Result<E> {
        self.get_by(d, key.to_value()).await
    }

    async fn get_by<E: Entity>(&mut self, d: &dyn Dialect, key: crate::value::Value) -> Result<E> {
        self.one(build::plan_get::<E>(d, key)?)
            .await?
            .ok_or(Error::NotFound)
    }

    async fn insert<E: Entity>(&mut self, d: &dyn Dialect, row: &E) -> Result<E> {
        match self {
            #[cfg(feature = "postgres")]
            Link::Pg(_) | Link::PgTx(_) => {
                let sql = build::plan_insert(d, row, None, true)?;
                self.one(sql)
                    .await?
                    .ok_or_else(|| Error::Internal("insert returned no row".into()))
            }
            #[cfg(feature = "oracle")]
            Link::Ora(link) => {
                let dialect = Oracle {
                    compat: link.compat(),
                };
                let key = link
                    .insert(&dialect, row, on_error::<E>)
                    .await?;
                self.get_by(d, key).await
            }
        }
    }

    /// Without `RETURNING` (Oracle) the row is read again on the same connection after the
    /// update, so a transaction sees its own write.
    async fn update<E: Entity>(&mut self, d: &dyn Dialect, row: &E) -> Result<E> {
        let returning = d.returning();
        let (sql, versioned) = build::plan_update(d, row, returning)?;
        let missing = if versioned {
            Error::Conflict
        } else {
            Error::NotFound
        };
        if returning {
            return self.one(sql).await?.ok_or(missing);
        }
        if self.execute(sql, on_error::<E>).await? == 0 {
            return Err(missing);
        }
        let key = E::fields()
            .iter()
            .position(|f| f.key)
            .and_then(|i| row.value_at(i))
            .unwrap_or(crate::value::Value::Null);
        self.get_by(d, key).await
    }

    async fn delete<E: Entity>(&mut self, d: &dyn Dialect, key: impl FieldType) -> Result<()> {
        match self
            .execute(build::plan_delete::<E>(d, key.to_value())?, on_error::<E>)
            .await?
        {
            0 => Err(Error::NotFound),
            _ => Ok(()),
        }
    }

    async fn fetch(
        &mut self,
        sql: Sql,
        err: OnError,
        each: &mut (dyn FnMut(&mut Vec<Cell>) -> Result<()> + Send),
    ) -> Result<()> {
        match self {
            #[cfg(feature = "postgres")]
            Link::Pg(c) => pg::fetch(c, sql, err, each).await,
            #[cfg(feature = "postgres")]
            Link::PgTx(c) => pg::fetch(c, sql, err, each).await,
            #[cfg(feature = "oracle")]
            Link::Ora(c) => c.fetch(sql, err, each).await,
        }
    }

    async fn execute(&mut self, sql: Sql, err: OnError) -> Result<u64> {
        match self {
            #[cfg(feature = "postgres")]
            Link::Pg(c) => pg::execute(c, sql, err).await,
            #[cfg(feature = "postgres")]
            Link::PgTx(c) => pg::execute(c, sql, err).await,
            #[cfg(feature = "oracle")]
            Link::Ora(c) => c.execute(sql, err).await,
        }
    }

    async fn commit(self) -> Result<()> {
        match self {
            #[cfg(feature = "postgres")]
            Link::PgTx(t) => t
                .commit()
                .await
                .map_err(|e| Error::Internal(format!("commit: {e}"))),
            #[cfg(feature = "postgres")]
            Link::Pg(_) => Ok(()),
            #[cfg(feature = "oracle")]
            Link::Ora(c) => c.commit().await,
        }
    }

    async fn rollback(self) -> Result<()> {
        match self {
            #[cfg(feature = "postgres")]
            Link::PgTx(t) => t
                .rollback()
                .await
                .map_err(|e| Error::Internal(format!("rollback: {e}"))),
            #[cfg(feature = "postgres")]
            Link::Pg(_) => Ok(()),
            #[cfg(feature = "oracle")]
            Link::Ora(c) => c.rollback().await,
        }
    }
}

#[cfg(test)]
mod tests;
