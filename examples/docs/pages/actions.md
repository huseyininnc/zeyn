# Actions

An action is server code that a screen, a grid, a form, a command, the JSON API or an agent can call. `#[action]` declares who may call it; the framework checks that before the code runs.

## Declaring an action

```rust
#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

- The first argument is the input. It comes from the client, so the framework reads it as untrusted JSON.
- Every other argument is injected: `Ctx` or a registered `Service`.
- The return type is `Result<T>`.

## Access

Every action declares exactly one of:

| Option | Meaning |
|---|---|
| `permission = "feature.resource.verb"` | The identity needs this permission |
| `public` | Anyone, including users who have not signed in |

An action without access does not compile. Permission names are lower case and dotted, such as `sales.order.write`.

Other options:

| Option | Meaning |
|---|---|
| `audit` | Writes an audit record for every call |
| `reauth` | The user must have signed in recently (`App::reauth_within`) |

## Errors

| Error | Status | Use |
|---|---|---|
| `Error::NotFound` | 404 | The record does not exist or the user may not know it exists |
| `Error::Forbidden` | 403 | Access denied |
| `Error::Denied(t!("..."))` | 403 | Denied with a reason the user can read |
| `Error::Conflict` | 409 | The row version changed since the form loaded |
| `Error::Validation(errors)` | 422 | Field errors, shown next to the fields |
| `Error::Internal("detail")` | 500 | The detail goes to the server log only |

A field error from code:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## Lists

Lists take a `DataQuery` and return `Page<M>`. A list action never returns every row; the grid asks for pages, and the server caps the page size whatever the client sends.

## The context

`Ctx` is the request: `ctx.identity()`, `ctx.locale()`, `ctx.time_zone()`, `ctx.now()`, `ctx.today()`, `ctx.tenant()`, `ctx.license()`, `ctx.config()`, `ctx.theme()` and `ctx.t(&text)`. Use `ctx.now()` and `ctx.today()` instead of the system clock, so tests can fix the time with `App::fixed_now`.

## Where actions are used

- `DataGrid::of(action)` loads rows; `.edit_with(action)`, `.row_action(...)` and `.bulk_action(...)` change them.
- `Form::submit(label, action)` saves a form.
- `ActionButton::new(label, action, input)` runs an action from a button.
- `Command::action(...)` runs one from the command palette or a shortcut.
- `Feature::api(action)` publishes it as JSON (see [API and OpenAPI](api.md)).
- `Feature::search(action)` adds a server search to the command palette.

Every path runs the same checks: CSRF for browser calls, the feature's guards, the action's access, then the interceptors.
