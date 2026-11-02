# Effects and windows

Effects are what the browser does after a click or a successful action: open a window, go to a screen, show a toast, ask for confirmation, or close a window and refresh the screen under it. They are declared in Rust and run by the runtime, so no JavaScript is written.

## The effects

| Effect | Built with | Result |
|---|---|---|
| Open a window | `Window::open(screen, params)` | Opens the screen in a modal window |
| Navigate | `Effect::navigate(screen, params)` | Goes to the screen in place, like a link |
| Toast | `Effect::Toast { message }` | Shows a short message |
| Confirm | `Effect::confirm(message, then)` | Runs `then` after the user confirms |
| Close and refresh | `Effect::CloseAndRefresh` | Closes the window and reloads the screen that opened it |

Params are typed: an effect for a screen with the wrong params does not compile.

## After an action

Forms and action buttons take an `on_success` handler. It receives the action's result and a `UiCtx` that collects effects:

```rust
Form::of(product)
    .field(Product::name)
    .submit(t!("stock.save"), save_product)
    .on_success(|saved, ui| {
        if let Ok(p) = saved {
            ui.toast(t!("stock.saved", code = p.code));
        }
        ui.close_and_refresh();
    })
```

`UiCtx` has `toast`, `close_and_refresh`, `navigate` and `effect` for anything else, such as a confirmation:

```rust
ActionButton::new(t!("ui.check_stock"), check_stock, ()).on_success(|_, ui| {
    ui.effect(Effect::confirm(t!("ui.low_stock_confirm"), Effect::navigate(product_list, ())));
})
```

## Windows

A window is a screen without a route, built with `Screen::window(title)`. Windows stack: a window can open another one. Escape or the close button closes the top window and focus goes back to the element that opened it. `.width(px)` sets its width.

```rust
#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::name).submit(t!("stock.save"), save_product)))
}
```

## Where effects come from

- Grid links and row opens: `.link(|p| Window::open(...))`, `.on_open(|p| ...)`.
- Toolbar buttons: `.opens(screen, params)`.
- Context menus: `.item(label, effect)`.
- Commands: `Command::open`, `Command::navigate`, `Command::effect`.
- `on_success` handlers of forms and action buttons.

Effects never reload the page. A navigation loads only the new screen; a refresh reloads only the screen under the window.
