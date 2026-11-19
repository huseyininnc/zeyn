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
| Refresh topics | `Effect::refresh("orders")` or `ui.refresh(["orders", "stock"])` | Refreshes in place the nodes of the page that listen to a topic (see [Topics](#topics)) |

Params are typed: an effect for a screen with the wrong params does not compile.

`Effect::navigate` to a screen whose route has path segments (see [Params in the path](screens.md)) lands on the path form, such as `/orders/42`; the params that are not in the path go in the `params` query.

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

## Topics

When one part of a screen changes, other parts often show the same data: a saved order changes the total, a filter changes the grid and the chart. A topic connects them without data passing through the browser. Mark the nodes that show the data with `.listens(topic)`; they refresh in place when the topic is named.

```rust
Screen::new(t!("sales.title"))
    .body(Kpi::new(t!("sales.total"), total).listens("sales"))
    .body(Chart::bar(t!("sales.by_month")).labels(months).series(t!("sales.amount"), amounts).listens("sales"))
    .body(DataGrid::of(search_sales).column(Sale::amount).listens("sales"))
```

A topic is named in three ways:

- `ui.refresh("sales")` (or `Effect::refresh`) in an `on_success` handler, after an action changed the data.
- A filter control that `.drives("sales")`: filter chips, a search box or a form without a submit button (selects, date ranges, text). See [Filter forms](forms.md#filter-forms).
- `hub.notify(&ctx, "sales")` from the realtime channel (feature `realtime`), when someone else changed the data. See `docs/realtime.md`.

`.listens` works on every component: call it last, after the component's own builder methods. A listener that holds a grid reloads its grid through the grid endpoint, keeping the page, sort and filters (a driven change starts on the first page). Every other listener is rendered again on the server with one `POST /_zeyn/nodes` for all of them, so a KPI and a chart cost one request. Only topic names travel; the server opens the screen with the user's access and params as for any request, so a refresh never shows what the user may not see.

Topics named within one animation frame are handled once, a newer refresh cancels the one still running on the same screen, and only nodes that are on the page refresh. Focus and open `<details>` stay where they were when the new content has the same element (by id, test id or the row key of a list).

Topic names are 1 to 64 letters, digits, `-`, `_` or `.`. `App::check_page` reports a topic driven on a page that nothing on it listens to, and `zeyn check` warns about a topic that is refreshed or driven but never listened to, or listened to but never refreshed, driven or notified.

@live topics

## Toasts

`ui.toast(text)` shows an info toast. `ui.notify(Toast::success(text))` picks the tone: `Toast::info`, `success`, `warning` or `danger`. `.action(label, effect)` adds a button that runs another effect, such as opening the record that was saved:

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

Toasts stay four seconds (danger ones eight), and the time stops while the pointer is over a toast or it holds focus, so its buttons can be reached. Info, success and warning toasts are read politely by a status region; danger toasts are alerts, announced at once. Every toast has a dismiss button, the tone is also given by its color bar and text, and at most three toasts show at once (`MAX_TOASTS`): a new one removes the oldest.

@live toasts
