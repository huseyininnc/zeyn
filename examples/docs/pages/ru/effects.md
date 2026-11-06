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
| Обновить темы | `Effect::refresh("orders")` или `ui.refresh(["orders", "stock"])` | Обновляет на месте узлы страницы, которые слушают тему (см. [Темы](#topics)) |

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

## Темы

Когда меняется одна часть экрана, другие части часто показывают те же данные: сохранённый заказ меняет итог, фильтр меняет сетку и график. Тема связывает их, и данные не проходят через браузер. Отметьте узлы, которые показывают данные, с помощью `.listens(topic)`; они обновляются на месте, когда тема названа.

```rust
Screen::new(t!("sales.title"))
    .body(Kpi::new(t!("sales.total"), total).listens("sales"))
    .body(Chart::bar(t!("sales.by_month")).labels(months).series(t!("sales.amount"), amounts).listens("sales"))
    .body(DataGrid::of(search_sales).column(Sale::amount).listens("sales"))
```

Тему называют тремя способами:

- `ui.refresh("sales")` (или `Effect::refresh`) в обработчике `on_success`, после того как action изменило данные.
- Элемент фильтра, который `.drives("sales")`: чипы фильтра, поле поиска или форма без кнопки отправки (списки выбора, диапазоны дат, текст). См. [Формы фильтров](forms.md#filter-forms).
- `hub.notify(&ctx, "sales")` из канала realtime (feature `realtime`), когда данные изменил кто-то другой. См. `docs/realtime.md`.

`.listens` работает на любом компоненте: вызывайте его последним, после собственных методов builder компонента. Слушатель с сеткой перезагружает сетку через endpoint сетки и сохраняет страницу, сортировку и фильтры (изменение от фильтра начинается с первой страницы). Любой другой слушатель заново рендерится на сервере одним `POST /_zeyn/nodes` для всех сразу, поэтому KPI и график стоят одного запроса. Передаются только имена тем; сервер открывает экран с доступом и params пользователя, как при любом запросе, поэтому обновление никогда не покажет то, что пользователю видеть нельзя.

Темы, названные в пределах одного кадра анимации, обрабатываются один раз, более новое обновление отменяет ещё идущее на том же экране, и обновляются только узлы, которые есть на странице. Фокус и открытые `<details>` остаются на месте, если в новом содержимом есть тот же элемент (по id, test id или ключу строки списка).

Имена тем состоят из 1 до 64 букв, цифр, `-`, `_` или `.`. `App::check_page` сообщает о теме, которой управляют на странице, но которую там никто не слушает, а `zeyn check` предупреждает о теме, которую обновляют или которой управляют, но никто не слушает, или которую слушают, но никто не обновляет, не управляет ею и не уведомляет.

@live topics

## Toast

`ui.toast(text)` показывает информационный toast. `ui.notify(Toast::success(text))` выбирает тон: `Toast::info`, `success`, `warning` или `danger`. `.action(label, effect)` добавляет кнопку, выполняющую другой эффект, например открытие сохранённой записи:

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

Toast остаются на четыре секунды (опасные, danger, на восемь), а время останавливается, пока указатель находится над toast или он в фокусе, чтобы до его кнопок можно было дотянуться. Toast info, success и warning вежливо зачитываются областью статуса; toast danger это оповещения (alert), которые объявляются сразу. У каждого toast есть кнопка закрытия, тон передаётся также цветной полосой и текстом, и одновременно показывается не более трёх toast (`MAX_TOASTS`): новый вытесняет самый старый.

@live toasts
