# Screens and params

A screen is a page with a route, or a window opened over another screen. `#[screen]` declares its access like an action does.

## A page

```rust
#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products"))
        .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).shortcut("Alt+N").opens(product_editor, ProductParams { id: None }))
        .body(DataGrid::of(search_products).column(Product::code).column(Product::name).flex(1))
}
```

- `Screen::new(title)` is a page; `Screen::window(title)` is a window.
- Several `.body(...)` calls place several components one under the other.
- A screen returns `Screen` or `Result<Screen>`.

## Params

The first argument of a screen is its params: a `#[params]` struct, or `_: ()` when it has none. Other arguments are injected, like in actions.

```rust
#[params]
pub struct ProductParams {
    pub id: Option<ProductId>,
}

#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::code).submit(t!("stock.save"), save_product)))
}
```

Params are typed everywhere they are used: `Window::open(product_editor, 7)` does not compile, because the editor takes `ProductParams`.

A screen without `route` has no URL. It opens as a window from a link, a toolbar button, a menu or a command.

## Params in the path

A route can name params fields as path segments. Each `{name}` binds the field of the same name and reads as that field's type:

```rust
#[params]
pub struct OrderParams {
    pub id: OrderId,
    #[serde(default)]
    pub tab: Option<String>,
}

#[screen(route = "/orders/{id}", permission = "sales.order.read")]
pub fn order(p: OrderParams, store: OrderStore) -> Result<Screen> {
    let order = store.get(p.id).ok_or(Error::NotFound)?;
    Ok(Screen::new(t!("sales.order")).body(DescriptionList::new().item(t!("sales.customer"), order.customer)))
}
```

- `/orders/42` opens order 42. Fields that are not in the path travel as before in the JSON query: `/orders/42?params={"tab":"lines"}`.
- Segments take integers, entity keys (`OrderId`), `String` and `Slug`. A value only parses in the form the framework writes it (`7`, not `007` or `+7`); anything else answers 404.
- A segment that names no field of the params, a route with segments on a screen without params, or a field type that cannot be a segment does not compile, and the error says what to change.
- Several segments work the same way: `#[screen(route = "/catalog/{slug}/{n}", public)]` with `slug: Slug` and `n: u32`.
- Links the framework writes use the path form: `Effect::navigate`, breadcrumbs, commands and `return_to` after sign in. Screens without segments keep `?params={json}`.
- A screen with segments cannot be a menu item or the home screen, because nothing could fill them.
- Access is checked on every request, as for any screen.

### Slugs

`Slug::from_title("Çay Bardağı")` is `cay-bardagi`: lowercase ASCII, words joined by `-`, Turkish letters mapped (`ı` and `İ` to `i`, `ş` to `s`, `ğ` to `g`, `ç` to `c`, `ö` to `o`, `ü` to `u`), accents of other Latin letters dropped, and anything else left out. A slug is at most 80 bytes. In a path only the canonical form parses, so `/catalog/Cay-Bardagi/1` answers 404.

### Not found and moved

Return `Err(Error::NotFound)` when the record is not there; the user gets the 404 page. When the address changed, for example after a product was renamed, answer with the new one. The browser gets a permanent redirect (301), and in app navigation follows it without a page load:

```rust
#[screen(route = "/catalog/{slug}/{n}", public)]
pub fn catalog_page(p: CatalogParams, store: Catalog) -> Result<Screen> {
    let item = store.find(p.n).ok_or(Error::NotFound)?;
    if item.slug != p.slug {
        return Ok(Screen::moved_to(catalog_page, CatalogParams { slug: item.slug, n: p.n }));
    }
    Ok(Screen::new(item.name))
}
```

`Screen::redirect(screen, params)` sends one request on to another screen, such as a random pick or the newest item, while the address itself stays valid. The browser gets `303 See Other` with the target's canonical path and `Cache-Control: no-store`; in app navigation follows it without a page load, and the address bar shows the target. The target is always a typed screen with a route, so a redirect can never lead to an address outside the app, and a target the app does not serve answers 404.

```rust
#[screen(route = "/random", public)]
pub async fn random_item(store: Catalog) -> Result<Screen> {
    let item = store.random().await?.ok_or(Error::NotFound)?;
    Ok(Screen::redirect(catalog_page, CatalogParams { slug: item.slug, n: item.n }))
}
```

@live redirect

## Access

| Option | Meaning |
|---|---|
| `permission = "..."` | The identity needs this permission |
| `public` | Anyone. Visitors without a session get the page without cookies (see [Public pages](public-pages.md)) |
| `cache = 300` | Public screens only: browsers and shared caches may keep the page of a visitor without a session for this many seconds (at most 86400) |
| `sensitive` | Opening it is recorded as a read of personal data |
| `reason` | The user must give a reason before it opens (see [Audit, masking and jobs](enterprise.md)) |

A page also tells search engines and link previews about itself with `.description(..)`, `.canonical(..)`, `.og_image(..)`, `.json_ld(..)` and `.robots(..)` on the `Screen`; see [Public pages](public-pages.md#search-engines-and-link-previews).

Menu items, toolbar buttons and commands that open a screen the user may not open are left out of the HTML.

A screen for visitors who are not signed in declares `public`. Its actions need their own access: a public screen that lists data calls a `public` action, or the list stays empty for visitors.

```rust
#[action(public)]
pub async fn latest_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/catalog", public)]
pub fn catalog() -> Screen {
    Screen::new(t!("catalog.title")).body(DataGrid::of(latest_products).column(Product::code).column(Product::price))
}
```

Leaving out both `permission` and `public` is a compile error, so a screen is never open by accident.

## Toolbar and context menu

```rust
Screen::new(t!("stock.products"))
    .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).opens(product_editor, ProductParams { id: None }).export(Export::Csv))
    .context_menu(ContextMenu::new().item(t!("stock.new_product"), Window::open(product_editor, ProductParams { id: None })).action(t!("stock.restock_low"), restock_low, ()))
```

Shortcuts like `Alt+N` work on the screen and are shown in the palette. Shortcuts the browser uses (`Ctrl+P`, `Ctrl+S`, `F5`) are not bound unless the button calls `.override_browser()`, and ones it never passes on (`Ctrl+N`, `Ctrl+T`, `Ctrl+W`) are never bound; free combinations are `Alt+Shift+<letter>` and `Ctrl+Alt+<letter>`.

## Features and the menu

A screen belongs to a feature. The feature's route prefix comes before the route, its guards run before the screen opens, and its menu lists the screens:

```rust
Feature::new("stock")
    .route_prefix("/stock")
    .guard(SignedIn)
    .screen(product_list)
    .screen(product_editor)
    .menu(|m| m.group(t!("menu.stock"), Icon::Box).item(t!("stock.products"), product_list))
```

`App::home(screen)` makes `/` open a screen.

## Error pages

`App::not_found(screen)` and `App::server_error(screen)` give the app its own pages for 404 and for server errors:

```rust
#[screen(public)]
pub fn not_found_page() -> Screen {
    Screen::new(t!("errors.not_found")).body(EmptyState::new(t!("errors.not_found_text")))
}

App::new(t!("app.name")).not_found(not_found_page).server_error(server_error_page)
```

- The 404 screen answers unknown paths and every screen that returns `Err(Error::NotFound)`, including path values that do not parse.
- The server error screen answers `Error::Internal`, timeouts (504) and panics in handlers. The error's details go to the server log only; the screen gets no params and learns nothing about the request.
- Both are shown inside the shell with the right status code. In app navigation gets only the screen and swaps it in without a page load. Framework endpoints and the API keep answering JSON.
- Error screens take no params, need no route and are not part of a feature. Make them `public`: a user who may not open one, or an error screen that fails itself, gets the short built in message.

## How a screen reaches the browser

The first request gets the full page: menu, screen and assets. Links inside the app then load only the screen and swap it in place, so the menu, open state and scroll position stay. The back button, new tabs and bookmarks work as with normal links.
