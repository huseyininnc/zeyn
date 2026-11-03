# Effects и окна

Effect (эффект) это то, что делает браузер после щелчка или успешного action: открывает окно, переходит на экран, показывает уведомление (toast), запрашивает подтверждение либо закрывает окно и обновляет экран под ним. Они объявляются в Rust и выполняются runtime, поэтому JavaScript писать не нужно.

## Эффекты

| Эффект | Создаётся через | Результат |
|---|---|---|
| Открыть окно | `Window::open(screen, params)` | Открывает экран в модальном окне |
| Перейти | `Effect::navigate(screen, params)` | Переходит на экран на месте, как по ссылке |
| Toast | `Effect::Toast { message }` | Показывает короткое сообщение |
| Toast с тоном | `Toast::success(message).action(label, effect).into()` | Toast в нужном тоне (info, success, warning, danger) с необязательной кнопкой, выполняющей другой эффект |
| Подтверждение | `Effect::confirm(message, then)` | Выполняет `then` после подтверждения пользователем |
| Закрыть и обновить | `Effect::CloseAndRefresh` | Закрывает окно и перезагружает экран, который его открыл |

Params типизированы: эффект для экрана с неверными params не компилируется.

`Effect::navigate` на экран, чей маршрут содержит сегменты пути (см. [Params в пути](screens.md)), приводит к форме с путём, например `/orders/42`; params, которых нет в пути, передаются в запросе `params`.

## После action

Формы и кнопки действий принимают обработчик `on_success`. Он получает результат action и `UiCtx`, который собирает эффекты:

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

У `UiCtx` есть `toast`, `notify`, `close_and_refresh`, `navigate` и `effect` для всего остального, например для подтверждения:

```rust
ActionButton::new(t!("ui.check_stock"), check_stock, ()).on_success(|_, ui| {
    ui.effect(Effect::confirm(t!("ui.low_stock_confirm"), Effect::navigate(product_list, ())));
})
```

## Окна

Окно это экран без маршрута, создаваемый через `Screen::window(title)`. Окна накладываются друг на друга: окно может открыть другое. Escape или кнопка закрытия закрывает верхнее окно, и фокус возвращается на элемент, который его открыл. `.width(px)` задаёт ширину.

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

## Откуда берутся эффекты

- Ссылки таблицы и открытие строк: `.link(|p| Window::open(...))`, `.on_open(|p| ...)`.
- Кнопки панели инструментов: `.opens(screen, params)`.
- Контекстные меню: `.item(label, effect)`.
- Команды: `Command::open`, `Command::navigate`, `Command::effect`.
- Обработчики `on_success` форм и кнопок действий.

Эффекты никогда не перезагружают страницу. Переход загружает только новый экран; обновление перезагружает только экран под окном.

## Toast

`ui.toast(text)` показывает информационный toast. `ui.notify(Toast::success(text))` выбирает тон: `Toast::info`, `success`, `warning` или `danger`. `.action(label, effect)` добавляет кнопку, выполняющую другой эффект, например открытие сохранённой записи:

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

Toast остаются на четыре секунды (опасные, danger, на восемь), а время останавливается, пока указатель находится над toast или он в фокусе, чтобы до его кнопок можно было дотянуться. Toast info, success и warning вежливо зачитываются областью статуса; toast danger это оповещения (alert), которые объявляются сразу. У каждого toast есть кнопка закрытия, тон передаётся также цветной полосой и текстом, и одновременно показывается не более трёх toast (`MAX_TOASTS`): новый вытесняет самый старый.

@live toasts
