# Data and databases

Zeyn does not hide the database behind an object mapper. Data lives in services (stores) that the app registers; actions receive them as arguments. A store can keep rows in memory, call a database, or call another service. Screens, grids and the API only see actions.

## A store

```rust
#[derive(Clone)]
pub struct ProductStore {
    db: Pool,
}

impl Service for ProductStore {}

impl ProductStore {
    pub async fn get(&self, id: ProductId) -> Result<Product> {
        self.db.product(id).await.map_err(|e| Error::Internal(e.to_string()))?.ok_or(Error::NotFound)
    }
}
```

Register it once with `App::service(store)`. The app refuses to start when a screen or an API needs a store that is not registered, and says which one.

## Lists: DataQuery and Page

A list action takes a `DataQuery` and returns `Page<M>`:

| Field | Meaning |
|---|---|
| `page` | `Offset { offset, limit }` for numbered pages, or `After { cursor, limit }` for infinite scroll |
| `sort` | Fields and directions, in order |
| `filters` | Field, operator and value, typed by the column |
| `with_total` | Whether the client needs the total count |
| `aggregates` | Column totals to compute over every filtered row |
| `time_zone` | Zone for date filters on `DateTime` columns, set by the server |
| `group` | Group rows by a field, set by the grid |

`DataQuery::apply(vec)` does all of it in memory. For a SQL database use `zeyn::sql`: `Db::find` turns the same query into `WHERE`, `ORDER BY`, `LIMIT` and `GROUP BY` for PostgreSQL or Oracle. Only the entity's own fields can be filtered or sorted, every value is bound as a parameter, and the page size is capped by the server, so client input never reaches the SQL text. Write SQL by hand only for queries `Db::find` cannot express: `Db::query::<M>(sql, args)` reads the rows of a `SELECT` into a model, takes the SQL only as a string literal and every value as a parameter, and reads at most 1000 rows.

```rust
let hits: Vec<Hit> = db
    .query("SELECT id, title FROM items WHERE title % $1 ORDER BY similarity(title, $1) DESC LIMIT 20", vec![q.into()])
    .await?;
```

```toml
zeyn = { version = "0.0.7", features = ["postgres"] }   # or ["oracle"]
```

```rust
let db = Db::from_config(&config).await?;            // reads the secret database.url
App::new(t!("app.name")).config(config).service(db)

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, db: Db) -> Result<Page<Product>> {
    db.find(&q).await
}
```

The [database reference](/reference/database) covers connection settings, Oracle, migrations, transactions, streams and the SQL each query becomes.

`Page` carries the rows, the total when asked, the cursor of the next batch and the totals:

```rust
#[action(permission = "sales.order.read")]
pub async fn search_orders(q: DataQuery, store: OrderStore) -> Result<Page<Order>> {
    store.search(&q).await
}
```

## Row versions

A `#[version] pub version: i64` field turns on optimistic locking. Forms send the version they loaded; the store compares it and returns `Error::Conflict` when someone saved in between. The form then tells the user instead of overwriting their colleague's change.

```rust
if current.version != product.version {
    return Err(Error::Conflict);
}
product.version += 1;
```

## Validation in the store

Rules that need the data, such as `unique`, are checked in the store and returned as field errors:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## Tenants

With `App::tenancy(resolver)` every request carries a tenant (`ctx.tenant()`), from the host name (`SubdomainTenant`) or a header (`HeaderTenant`), or the user's own tenant when the request names none. A signed in user must belong to the request's tenant (`Identity::member_of`), otherwise the request runs signed out. A store uses it to pick a schema or filter rows; `TenantRequired` refuses requests without one.

## Saved user state

Grid views (columns, sort, filters), dock layouts and the chosen theme can be kept per user on the server with `App::datagrid_views(store)`. `MemoryViewStore` keeps them in memory for development; an app implements `DataGridViewStore` with its database for production.

## Background work

Imports, exports and other long work run as background jobs with progress (`Jobs`), so a request never waits for them. See [Audit, masking and jobs](enterprise.md).

Jobs live in memory unless the app gives them a store. With `SqlJobStore` queued runs and the next run of every schedule are kept in the database: they survive a restart, several servers share one queue, and a lease makes sure two workers never run the same run.

```rust
let jobs = SqlJobStore::new(db.clone());
jobs.create_tables().await?;

App::new("Billing")
    .job_store(jobs)
    .queued_job("send_invoice", Schedule::manual().retry(5, Duration::from_secs(10)), send_invoice)
    .job("close_day", Schedule::every(Duration::from_secs(86_400)), close_day)

async fn send_invoice(ctx: Ctx, invoice_id: String) -> Result<()> { ... }

#[action(permission = "billing.invoice.send")]
pub async fn send(id: InvoiceId, jobs: Jobs) -> Result<()> {
    jobs.enqueue("send_invoice", id.0.to_string()).await?;
    Ok(())
}
```

- `jobs.enqueue(name, payload)` queues a run; `enqueue_after` delays it. The payload is text of at most 4000 bytes, such as an id or small JSON.
- A worker claims a due run with a lease of one minute and renews it while the run works. If the server stops, the run is claimed again after the lease ends; that try counts.
- A failed try waits for the backoff, which doubles up to ten minutes, until the retries are used up. Then the run stays in the table as `failed` with its error code. A schedule never fails for good: it moves on to its next interval.
- PostgreSQL claims with `FOR UPDATE SKIP LOCKED`, Oracle (11g and later) the same way through a select of ids. The table is `zeyn_jobs`; `create_job_tables(dialect, prefix)` gives the statements for a migration.
- The in memory store (`MemoryJobStore`) holds at most 10,000 runs.
