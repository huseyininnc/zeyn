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

`DataQuery::apply(vec)` does all of it in memory. A store backed by SQL turns the same query into `WHERE`, `ORDER BY`, `LIMIT` and `GROUP BY`, with every value bound as a parameter. Field names come from the client: map them to known columns before they reach SQL, never paste them in. The page size is capped by the server.

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
