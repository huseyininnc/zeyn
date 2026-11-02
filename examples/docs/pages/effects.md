# Effects and windows

Effects are what the browser does after a click or a successful action: open a window, go to a screen, show a toast, ask for confirmation, or close a window and refresh the screen under it. They are declared in Rust and run by the runtime, so no JavaScript is written.

## The effects

| Effect | Built with | Result |
|---|---|---|
| Open a window | `Window::open(screen, params)` | Opens the screen in a modal window |
| Navigate | `Effect::navigate(screen, params)` | Goes to the screen in place, like a link |
| Toast | `Effect::Toast { message }` | Shows a short message |
| Toast with a tone | `Toast::success(message).action(label, effect).into()` | A toast in a tone (info, success, warning, danger) with an optional button that runs another effect |
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

`UiCtx` has `toast`, `notify`, `close_and_refresh`, `navigate` and `effect` for anything else, such as a confirmation:

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

## Toasts

`ui.toast(text)` shows an info toast. `ui.notify(Toast::success(text))` picks the tone: `Toast::info`, `success`, `warning` or `danger`. `.action(label, effect)` adds a button that runs another effect, such as opening the record that was saved:

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

Toasts stay four seconds (danger ones eight), and the time stops while the pointer is over a toast or it holds focus, so its buttons can be reached. Info, success and warning toasts are read politely by a status region; danger toasts are alerts, announced at once. Every toast has a dismiss button, the tone is also given by its color bar and text, and at most three toasts show at once (`MAX_TOASTS`): a new one removes the oldest.

@live toasts
