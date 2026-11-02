# Database

`zeyn::sql` keeps entities in a SQL database. It lists them from the `DataQuery` a grid sends
(filters, sort, pages, totals, groups) and saves them with row version checks. PostgreSQL is
behind the `postgres` Cargo feature and Oracle behind `oracle`, so an app without a database
compiles none of it.

```toml
zeyn = { version = "0.0.4", features = ["postgres"] }   # or ["oracle"], or both
```

## Setup

Register one `Db` as a service and take it in actions:

```rust
let db = Db::from_config(&config).await?;
App::new("Stock").config(config).service(db)

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, db: Db) -> Result<Page<Product>> {
    db.find(&q).await
}

#[action(permission = "stock.product.write")]
pub async fn save_product(p: Valid<Product>, db: Db) -> Result<Product> {
    if p.id.0 == 0 { db.insert(&*p).await } else { db.update(&*p).await }
}
```

`Db::from_config` reads `database.url` as a secret and the pool limits from `database.*`:

| Key | Default | Meaning |
|---|---|---|
| `database.url` | required | `postgres://user:password@host:5432/db` or `oracle://user:password@host:1521/service` |
| `database.max_connections` | 10 | Pool size; requests wait for a free connection |
| `database.min_connections` | 0 | Connections kept open when idle |
| `database.acquire_timeout_ms` | 5000 | Longest wait for a connection |
| `database.idle_timeout_s` | 600 | Idle connections above the minimum close after this |
| `database.statement_timeout_ms` | 30000 | Longest a statement may run; 0 keeps the server default |
| `database.count_cap` | 1000000 | Largest total counted for a page |

`Db::connect(url, PoolLimits { .. })` does the same without `Config`. The URL scheme picks the
driver. Characters such as `@` or `:` in an Oracle password are written percent encoded (`%40`).

PostgreSQL connections open lazily, so the app starts while the database is down and the first
query reports the problem. Oracle asks the server for its version when it connects, unless the
URL says which SQL to write (`?compat=11` or `?compat=12`); with `compat` set, it starts without
the database too.

The `inventory` example keeps its products in memory, or in the database `ZEYN_DATABASE_URL`
names (build it with `--features postgres` or `--features oracle`):

```
docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55432:5432 postgres:17-alpine
ZEYN_DATABASE_URL=postgres://postgres:zeyn@127.0.0.1:55432/postgres cargo run -p zeyn-example-inventory --features postgres
```

## Oracle Instant Client

The Oracle driver is the `oracle` crate on ODPI-C. It compiles without Oracle software, but at
run time it loads the Oracle Client library (`oci.dll`, `libclntsh.so`). Oracle Instant Client is
free to use:

1. Download Instant Client Basic Light for your platform from Oracle's Instant Client page. On
   Windows x64 take the zip `instantclient-basiclite-windows.x64-19.*.zip`. Version 19 connects to
   Oracle Database 11.2 through 23ai; newer clients no longer connect to 11g.
2. Unpack it into a folder of its own, for example `C:\oracle\instantclient_19`.
3. Put that folder first on `PATH`, or set `ZEYN_ORACLE_CLIENT_DIR` to it. The variable wins over
   `PATH`, which helps when another Oracle home is installed. On Linux, install the RPM or set
   `LD_LIBRARY_PATH`; Basic Light also needs `libaio`.

Without the library the first connection fails with `DPI-1047: Cannot locate a 64-bit Oracle
Client library`.

## Dialects

The SQL builder is shared. A `Dialect` supplies only what differs, and `Postgres` and
`Oracle { compat }` implement it:

| | PostgreSQL | Oracle 12c and later (`V12`) | Oracle 11g (`V11`) |
|---|---|---|---|
| Parameters | `$1` | `:1` | `:1` |
| Identifiers | `"code"`, up to 63 characters | `"CODE"`, up to 30 characters | `"CODE"`, up to 30 characters |
| Pages | `LIMIT $n OFFSET $m` | `OFFSET :m ROWS FETCH NEXT :n ROWS ONLY` | `ROWNUM` window over the ordered query |
| New keys | identity, `RETURNING` | identity, `RETURNING id INTO :n` | `<table>_seq.NEXTVAL` |
| Read back after a write | `RETURNING` the row | read by key | read by key |
| Yes/no | `BOOLEAN` | `NUMBER(1)` | `NUMBER(1)` |
| Empty text | a value | NULL | NULL |

`compat` follows the idea of EF Core's `UseOracleSQLCompatibility("11")`: the server version
picks it when the URL does not. Identifiers stay within 30 characters on every Oracle version
because 12.1 has the same limit as 11g; a longer entity or field name is an error naming it.

Oracle stores the empty string as NULL. Zeyn reads a NULL in a required `String` field as `""`,
writes text columns as nullable, and sorts and pages text as a nullable column, so cursors stay
exact. An `Option<String>` holding `Some("")` reads back as `None`.

Oracle's driver is blocking. Every call runs on Tokio's blocking threads with a connection taken
from the bounded pool, so it never stalls the async runtime; at most `max_connections` calls run
at once and the others wait up to `acquire_timeout_ms`.

## Tables

Columns are named after the fields. Types:

| Field | PostgreSQL | Oracle |
|---|---|---|
| `#[key]` entity id | `BIGINT GENERATED BY DEFAULT AS IDENTITY` | `NUMBER(19)` identity (12c+) or with a sequence (11g) |
| `String` | `VARCHAR(max_len)` or `TEXT` | `VARCHAR2(max_len CHAR)` or `VARCHAR2(4000)`, nullable |
| `i32`, `i64` | `BIGINT` (an `INTEGER` column works too) | `NUMBER(19)` |
| `bool` | `BOOLEAN` | `NUMBER(1)` |
| `Money` | `BIGINT` amount in the smallest unit, plus `<field>_currency VARCHAR(3)` | `NUMBER(19)`, plus `<field>_currency` |
| `Decimal` | `NUMERIC(precision, scale)` from the field's rules, `NUMERIC(18, 2)` without them | `NUMBER(precision, scale)` |
| `Date` | `DATE` | `DATE` |
| `Time` | `TIME` | `VARCHAR2(8)` holding `HH:MM:SS` |
| `DateTime` | `TIMESTAMPTZ(3)` | `TIMESTAMP(3)` holding UTC |
| `LocalDateTime` | `TIMESTAMP` | `TIMESTAMP(0)` |
| `#[choice]` enum | `VARCHAR` holding the variant name | `VARCHAR2` |
| Custom field types | `TEXT` holding JSON | `CLOB` holding JSON |
| `Option<T>` | the same column, nullable | the same column, nullable |

Currencies are stored as `TRY`, `USD` and `EUR`. Dates and times travel as ISO text and the SQL
converts them with fixed formats (`TO_DATE(:1, 'YYYY-MM-DD')`, `to_char(...)`), so session
settings such as `NLS_DATE_FORMAT` or the PostgreSQL time zone do not change results. Decimals
are bound as exact text without a decimal separator (`12345E-2`, read by `CAST(... AS NUMERIC)`
and `TO_NUMBER(...)`) and read back with `CAST(... AS TEXT)` or
`TO_CHAR(..., 'TM9', 'NLS_NUMERIC_CHARACTERS=''.,''')`, so `NLS_NUMERIC_CHARACTERS` does not
change them either.

`create_table::<E>(dialect)` (or `db.create_table::<E>()`) writes the `CREATE TABLE` for
scaffolding and tests, with a constraint named `<table>_<field>_key` for every
`#[rule(unique)]` field and, on 11g, the key sequence. Production schemas belong in migrations.
On PostgreSQL:

```rust
db.migrate(&sqlx::migrate!("./migrations")).await?;
db.migrate_dir("./migrations").await?;
```

The first embeds the migrations at compile time, the second reads the folder at run time.
`sqlx::migrate!` needs `sqlx` with its `macros` and `migrate` features in the app's own
`Cargo.toml`; `migrate_dir` needs nothing more. Oracle schemas are migrated with the team's own
tool; `create_table` gives the starting script.

## Reading

`db.find::<E>(&query)` runs at most three statements on one connection: the page, the count
and the totals. The filters are read by the same code as `DataQuery::apply`, so a list behaves
the same on memory data and in SQL:

| Filter | SQL |
|---|---|
| `contains`, `starts_with` | `LOWER(col) LIKE $1 ESCAPE '\'` with `%` and `_` in the text escaped |
| `equals` | `LOWER(col) = $1` |
| `from`, `to` on numbers and money | `col >= $1`, `col <= $2` |
| `from`, `to` on dates and times | `col >= CAST($1 AS date)`; a date bound on a moment covers the whole day in the user's time zone |
| `any_of` | `col IN ($1, $2, ...)` |
| `is` | `col = $1` |
| `blank` | `col IS NULL`, `col IS NOT NULL` |

`contains` on money and yes/no columns is ignored in SQL. Text sorts by the database collation
(Oracle: `NLS_SORT`).

Every sort ends with the key, so the order is total. NULLs come first going up and last going
down, as in memory; the `NULLS` clause is written only for nullable columns, so plain indexes
serve the other sorts.

`get(key)` reads one row and fails with `Error::NotFound` when there is none. Any table or view
can be read through an entity, for example Oracle's `user_sequences`.

## Cursors

Numbered pages use offsets. Infinite and virtual scroll use keyset cursors, which do not slow
down deep into a list and do not skip or repeat rows when others are inserted:

```sql
WHERE "code" <= $1 AND (("code" < $2) OR ("code" = $3 AND "id" > $4))
ORDER BY "code" DESC, "id" ASC LIMIT $5
```

A page fetches one row more than asked to know whether more follow. `Page::next` then carries a
cursor: base64url JSON with the sort fields, their directions and the last row's value of each
sort key (the key last). The cursor is opaque to clients but not secret, and the server never
trusts it. It is decoded with a size limit, refused when its order differs from the query's, and
every value is checked against its column type before it is bound. A bad cursor is a validation
error, not a query. It only says where to continue; filters and permissions apply again.

Grouped pages use offsets for their cursors.

## Totals and counts

`with_total` counts at most `database.count_cap` rows (a `COUNT(*)` over the capped query), so a
total over a huge table stays cheap; when the count reaches the cap, `Page::total` is the cap.

Column totals are one aggregate statement over every row that matches the filters. They follow
the in memory rules: an empty sum is 0, an empty average is empty, money averages round half away
from zero, and money in more than one currency has no total.

`DataQuery::group` turns into `GROUP BY` with the count and the asked totals per group.

## Writing

- `insert(&row)` leaves a new key (0) to the database, sets the row version to 1 and returns the
  stored row.
- `update(&row)` runs `UPDATE ... SET version = version + 1 WHERE id = $1 AND version = $2`.
  Zero rows means someone else saved first (or deleted the row): `Error::Conflict`.
- `delete::<E>(key)` fails with `Error::NotFound` when nothing was deleted.
- `#[readonly]` fields are never written from a model, so a forged form cannot change them. Server
  code changes them with SQL of its own: `db.execute("UPDATE products SET stock = stock - $1 WHERE id = $2", args)`.
  `execute` takes a string literal, so values can only arrive as parameters.

A unique violation (PostgreSQL `23505`, Oracle `ORA-00001`) becomes a validation error on the
field: the `#[rule(unique)]` field whose name is in the constraint name, or the only unique field.
The form shows it under the input like any other rule. Name constraints `<table>_<column>_key`.

## Transactions

```rust
let mut tx = db.begin().await?;
let order = tx.insert(&order).await?;
tx.execute("UPDATE products SET stock = stock - $1 WHERE id = $2", vec![qty.into(), id.0.into()]).await?;
tx.commit().await?;
```

`Tx` has the same calls as `Db` on one connection. Dropping it without `commit` rolls back.
Calls on `Db` itself commit one by one.

## Streams

`db.stream::<E>(&query)` returns every matching row in order without paging, for exports. A task
reads rows into a channel of `STREAM_BUFFER` (64) rows and waits while the reader is behind, so
memory stays flat for any result size. Dropping the stream stops the query and frees the
connection. `RowStream` implements `futures_core::Stream`.

## Security: SQL injection

- Identifiers (table and column names) come only from the entity: `E::TABLE` and its
  `FieldMeta` list. Names a client sends in filters and sorts are looked up in that list and
  dropped when they are not there. Every identifier is checked (letters, digits, `_`, at most the
  dialect's length) and quoted.
- Values are always bound parameters, including `LIKE` patterns, cursor values, limits and
  offsets. No value is ever written into the SQL text.
- Hand written SQL goes through `execute(&'static str, args)`: the statement must be a literal.
- Limits are clamped on the server (`MAX_PAGE_SIZE`, `MAX_FILTERS`, `MAX_SORT`, filter text length,
  `any_of` size), whatever the client asks for.
- Driver errors are logged as `Error::Internal` and never shown to users. The connection URL is
  read as a `Secret` and never logged.

## Performance

- Statements are prepared and cached per connection (sqlx's statement cache, Oracle's statement
  cache of 64).
- Rows are read into a few plain cell types and moved into the model by a serde deserializer:
  no intermediate map and no copies of the text.
- The pools are bounded and each `find` uses one connection for its two or three statements.
- Oracle fetches 128 rows per round trip. A page (at most 501 rows) is read on a blocking thread
  and handed over at once; streams go row by row.
- Streams hold at most `STREAM_BUFFER` rows.

## Tests

The SQL builder has unit tests for both dialects that need no database. The integration tests
compare `Db::find` with `DataQuery::apply` over the same rows, walk every keyset cursor, and
check saves, conflicts, unique errors, transactions and streams. They run when their variable
holds one or more URLs (separated by spaces) and say they were skipped otherwise:

```
docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55432:5432 postgres:17-alpine
ZEYN_TEST_PG=postgres://postgres:zeyn@127.0.0.1:55432/postgres cargo test -p zeyn --features postgres --test postgres

docker run -d --name zeyn-ora11 -p 15211:1521 -e ORACLE_PASSWORD=zeyn -e APP_USER=zeyn -e APP_USER_PASSWORD=zeyn gvenzl/oracle-xe:11-slim
docker run -d --name zeyn-ora21 -p 15221:1521 -e ORACLE_PASSWORD=zeyn -e APP_USER=zeyn -e APP_USER_PASSWORD=zeyn gvenzl/oracle-xe:21-slim
ZEYN_TEST_ORACLE="oracle://zeyn:zeyn@127.0.0.1:15211/XE oracle://zeyn:zeyn@127.0.0.1:15221/XEPDB1" cargo test -p zeyn --features oracle --test oracle
```

The Oracle containers take a few minutes to start; wait for `DATABASE IS READY TO USE!` in
`docker logs`.
