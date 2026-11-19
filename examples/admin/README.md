# Admin template

A back office app to copy and change: a dashboard, orders, customers, products, settings, users
and roles, on PostgreSQL or on sample data in memory.

```
cargo run -p zeyn-example-admin                    # http://127.0.0.1:3011, sample data in memory
```

| Before: the showcase dashboard | After: the admin dashboard |
|---|---|
| ![Showcase dashboard, light, 1440 px](../../docs/screenshots/admin/before-1440-light.png) | ![Admin dashboard, light, 1440 px](../../docs/screenshots/admin/after-1440-light.png) |
| ![Showcase dashboard, dark, 1440 px](../../docs/screenshots/admin/before-1440-dark.png) | ![Admin dashboard, dark, 1440 px](../../docs/screenshots/admin/after-1440-dark.png) |
| ![Showcase dashboard, light, 390 px](../../docs/screenshots/admin/before-390-light.png) | ![Admin dashboard, light, 390 px](../../docs/screenshots/admin/after-390-light.png) |
| ![Showcase dashboard, dark, 390 px](../../docs/screenshots/admin/before-390-dark.png) | ![Admin dashboard, dark, 390 px](../../docs/screenshots/admin/after-390-dark.png) |

## What is in it

| Screen | Path | Built with |
|---|---|---|
| Dashboard | `/` | `Kpi` tiles against the previous 30 days, a bar chart and a donut from one grouped query each, the latest orders |
| Orders | `/orders` | Server paged `DataGrid` with search, column filters, the filter builder, filter chips, saved views and CSV export |
| Order | `/orders/{id}` | `Tabs` with the details, a status form for roles that may change orders, and the history as a `Timeline` |
| Customers | `/customers`, `/customers/{id}` | A list, an editor window and a detail page with key figures and the customer's orders |
| Products | `/products` | The catalog with category badges, stock bars and an editor window |
| Settings | `/admin/settings` | `Form::settings()`: sections, help tips and a save bar that sticks to the bottom once something changes |
| Users, Roles | `/admin/users`, `/admin/roles` | Users with their role, and a table of what each role may do |

The side menu has nested groups (Administration, then Access), a count of open orders and a
"New" badge. The command palette (Alt+Shift+P, or `/`) lists the screens and two commands: "Go to
orders" (Alt+Shift+O) and "New customer" (Alt+Shift+C). List screens open their editor with
Alt+N.

## Live updates

Saving an order calls `socket.notify(&ctx, "orders")`. Every open page of a user who may read
orders refreshes in place: the dashboard's figures and charts, the order grids and the count in
the menu. Open the dashboard in one tab and an order in another, change its status, and the
dashboard follows without a page load.

## Roles

Roles are defined in code, in `src/roles.rs`: Admin, Manager and Viewer, each a list of
permissions. Screens and actions name the permissions and the server checks them on every
request; parts of a screen a role may not use (an editor, a "New" button) are left out.

The demo signs every visitor in as one user with the role in `auth.role` of
`config/app.json` (`ZEYN_ADMIN_AUTH__ROLE=Viewer` tries another). For real sign in, replace
`DevAuth` in `src/main.rs` with the `accounts` feature (`docs/accounts.md`) or an identity
provider (`docs/auth.md`) and give each user the permissions of their role.

## PostgreSQL

```
docker run -d --name zeyn-data-pg -e POSTGRES_USER=zeyn -e POSTGRES_PASSWORD=zeyn \
    -e POSTGRES_DB=zeyn -p 55437:5432 postgres:17-alpine
ZEYN_ADMIN_DATABASE__URL=postgres://zeyn:zeyn@127.0.0.1:55437/zeyn \
    cargo run -p zeyn-example-admin --features postgres
```

On the first start `migrations/` creates the tables and fills them with 500 customers,
200 products, 5,000 orders and 12 users. Configuration comes from `config/app.json` and from
variables that start with `ZEYN_ADMIN_` (`ZEYN_ADMIN_DATABASE__URL` is `database.url`; the pool
settings are in `docs/database.md`). Without `database.url` the same sample data is kept in
memory, which suits a demo; saved changes are then lost when the app stops.

## Tests

```
cargo test -p zeyn-example-admin
ZEYN_TEST_PG=postgres://zeyn:zeyn@127.0.0.1:55437/zeyn cargo test -p zeyn-example-admin --features postgres
```

`tests/app.rs` opens every screen, checks what each role may open and change, and saves an order
over the page socket while another user listens to the `orders` topic. `tests/postgres.rs` runs
the migrations, the screens and a save with an old row version on the database.

## Layout

```
src/lib.rs        the app: languages, services, socket, features
src/main.rs       configuration, the store and the server
src/models.rs     the entities, one table each
src/store.rs      memory or PostgreSQL behind one Store service
src/roles.rs      roles and their permissions
src/features/     dashboard, orders, customers, products, admin
migrations/       tables and sample data for PostgreSQL
locales/          English and Turkish texts
```
