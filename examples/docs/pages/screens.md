# Screens and params

A screen is a page with a route, or a window opened over another screen. `#[screen]` declares its access like an action does.

## A page

```rust
#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products"))
        .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).shortcut("Ctrl+N").opens(product_editor, ProductParams { id: None }))
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

## Access

| Option | Meaning |
|---|---|
| `permission = "..."` | The identity needs this permission |
| `public` | Anyone |
| `sensitive` | Opening it is recorded as a read of personal data |
| `reason` | The user must give a reason before it opens (see [Audit, masking and jobs](enterprise.md)) |

Menu items, toolbar buttons and commands that open a screen the user may not open are left out of the HTML.

## Toolbar and context menu

```rust
Screen::new(t!("stock.products"))
    .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).opens(product_editor, ProductParams { id: None }).export(Export::Csv))
    .context_menu(ContextMenu::new().item(t!("stock.new_product"), Window::open(product_editor, ProductParams { id: None })).action(t!("stock.restock_low"), restock_low, ()))
```

Shortcuts like `Ctrl+N` work on the screen and are shown in the palette.

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

## How a screen reaches the browser

The first request gets the full page: menu, screen and assets. Links inside the app then load only the screen and swap it in place, so the menu, open state and scroll position stay. The back button, new tabs and bookmarks work as with normal links.
