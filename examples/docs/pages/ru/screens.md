# Screens и params

Screen (экран) это страница с маршрутом или окно, открываемое поверх другого экрана. `#[screen]` объявляет доступ так же, как action.

## Страница

```rust
#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products"))
        .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).shortcut("Alt+N").opens(product_editor, ProductParams { id: None }))
        .body(DataGrid::of(search_products).column(Product::code).column(Product::name).flex(1))
}
```

- `Screen::new(title)` это страница; `Screen::window(title)` это окно.
- Несколько вызовов `.body(...)` размещают несколько компонентов один под другим.
- Экран возвращает `Screen` или `Result<Screen>`.

## Params

Первый аргумент экрана это его params (параметры): структура `#[params]` или `_: ()`, если параметров нет. Остальные аргументы внедряются, как в action.

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

Params типизированы везде, где используются: `Window::open(product_editor, 7)` не компилируется, потому что редактор принимает `ProductParams`.

Если все поля структуры `#[params]` имеют тип `Option`, экран открывается и без параметров, из ссылки меню или по простому адресу, и все поля равны `None`. Обязательному полю значение по-прежнему нужно: без него экран отвечает 404.

У экрана без `route` нет URL. Он открывается как окно по ссылке, кнопке панели инструментов, из меню или командой.

## Params в пути

Маршрут может называть поля params как сегменты пути. Каждый `{name}` привязывается к полю с тем же именем и читается как тип этого поля:

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

- `/orders/42` открывает заказ 42. Поля, которых нет в пути, передаются как раньше в JSON-запросе: `/orders/42?params={"tab":"lines"}`.
- Сегменты принимают целые числа, ключи entity (`OrderId`), `String` и `Slug`. Значение разбирается только в той форме, в которой его пишет фреймворк (`7`, а не `007` или `+7`); всё остальное даёт ответ 404.
- Сегмент, не называющий ни одно поле params, маршрут с сегментами у экрана без params или тип поля, который не может быть сегментом, не компилируются, и ошибка говорит, что нужно изменить.
- Несколько сегментов работают так же: `#[screen(route = "/catalog/{slug}/{n}", public)]` с `slug: Slug` и `n: u32`.
- Ссылки, которые пишет фреймворк, используют форму с путём: `Effect::navigate`, навигационная цепочка (breadcrumbs), команды и `return_to` после входа. Экраны без сегментов сохраняют `?params={json}`.
- Экран с сегментами не может быть пунктом меню или домашним экраном, потому что нечем заполнить сегменты.
- Доступ проверяется при каждом запросе, как для любого экрана.

### Slug

`Slug::from_title("Çay Bardağı")` даёт `cay-bardagi`: строчный ASCII, слова соединены через `-`, турецкие буквы заменены (`ı` и `İ` на `i`, `ş` на `s`, `ğ` на `g`, `ç` на `c`, `ö` на `o`, `ü` на `u`), диакритические знаки других латинских букв отбрасываются, всё остальное опускается. Slug занимает не более 80 байт. В пути разбирается только канонический вид, поэтому `/catalog/Cay-Bardagi/1` даёт ответ 404.

### Не найдено и перенесено

Верните `Err(Error::NotFound)`, когда записи нет; пользователь получит страницу 404. Когда адрес изменился, например после переименования товара, ответьте новым адресом. Браузер получит постоянное перенаправление (301), а при навигации внутри приложения следует за ним без загрузки страницы:

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

`Screen::redirect(screen, params)` отправляет один запрос дальше на другой screen, например на случайный выбор или на самый новый элемент, при этом сам адрес остаётся действительным. Браузер получает `303 See Other` с каноническим путём цели и `Cache-Control: no-store`; навигация внутри приложения следует за ним без загрузки страницы, а адресная строка показывает цель. Цель всегда типизированный screen с маршрутом, поэтому перенаправление никогда не ведёт на адрес вне приложения, а цель, которую приложение не обслуживает, получает ответ 404.

```rust
#[screen(route = "/random", public)]
pub async fn random_item(store: Catalog) -> Result<Screen> {
    let item = store.random().await?.ok_or(Error::NotFound)?;
    Ok(Screen::redirect(catalog_page, CatalogParams { slug: item.slug, n: item.n }))
}
```

@live redirect

## Доступ

| Параметр | Значение |
|---|---|
| `permission = "..."` | Субъекту нужно это разрешение |
| `public` | Любой. Посетители без сессии получают страницу без cookie (см. [Public pages](public-pages.md)) |
| `cache = 300` | Только для публичных экранов: браузеры и общие кэши могут хранить страницу посетителя без сессии указанное число секунд (не более 86400) |
| `sensitive` | Открытие записывается как чтение персональных данных |
| `reason` | Пользователь должен указать причину, прежде чем экран откроется (см. [Audit, masking and jobs](enterprise.md)) |

Страница также сообщает поисковым системам и предпросмотрам ссылок о себе через `.description(..)`, `.canonical(..)`, `.og_image(..)`, `.json_ld(..)` и `.robots(..)` у `Screen`; см. [Public pages](public-pages.md#search-engines-and-link-previews).

Пункты меню, кнопки панели инструментов и команды, открывающие экран, который пользователю открывать нельзя, исключаются из HTML.

Экран для посетителей, не вошедших в систему, объявляется как `public`. Его action нужен собственный доступ: публичный экран со списком данных вызывает `public` action, иначе список для посетителей останется пустым.

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

Если опустить и `permission`, и `public`, возникает ошибка компиляции, поэтому экран никогда не окажется открытым случайно.

## Панель инструментов и контекстное меню

```rust
Screen::new(t!("stock.products"))
    .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).opens(product_editor, ProductParams { id: None }).export(Export::Csv))
    .context_menu(ContextMenu::new().item(t!("stock.new_product"), Window::open(product_editor, ProductParams { id: None })).action(t!("stock.restock_low"), restock_low, ()))
```

Сочетания клавиш вроде `Alt+N` работают на экране и показываются в палитре. Сочетания, которые использует браузер (`Ctrl+P`, `Ctrl+S`, `F5`), не привязываются, если кнопка не вызывает `.override_browser()`, а те, которые браузер никогда не передаёт странице (`Ctrl+N`, `Ctrl+T`, `Ctrl+W`), не привязываются никогда; свободные комбинации: `Alt+Shift+<letter>` и `Ctrl+Alt+<letter>`.

Значки для кнопок, групп меню и инструментов берутся из `Icon`: `Plus`, `Edit`, `Delete`, `Save`, `Search`, `Box`, `Cart`, `Table`, `Home`, `Chart`, `Users`, `Settings`, `Bell`, `Shield`, `Clock`, `Tag`, `Mail`, `SignOut`, `ArrowUp`, `ArrowDown`. Это маски в `zeyn.css`, нарисованные на сетке 24 px линией 2 px со скруглёнными концами; они принимают цвет текста и скрыты от экранных дикторов, поэтому смысл всегда несёт подпись.

## Заголовок

`.subtitle(text)` ставит короткую строку под заголовком, а `.primary(|t| ...)` помещает основные действия экрана в конец строки заголовка в виде основных кнопок. Вместе они дают шапку страницы, которая есть у большинства экранов администрирования.

```rust
Screen::new(t!("orders.title"))
    .subtitle(t!("orders.intro"))
    .primary(|t| t.button(t!("orders.new")).icon(Icon::Plus).shortcut("Alt+N").opens(order_editor, OrderParams { id: None }))
    .toolbar(|t| t.export(Export::Csv))
```

`.primary` принимает те же кнопки, что и `.toolbar`, с теми же сочетаниями клавиш и пунктами палитры; панель инструментов остаётся под шапкой для остальных инструментов. Подзаголовок служит описанием экрана для экранных дикторов (`aria-describedby`). Если строке не хватает ширины, действия переносятся под заголовок, а на телефоне выравниваются по началу. Без подзаголовка и основных действий экран выводится как раньше, с простым заголовком.

## Feature и меню

Экран принадлежит Feature. Префикс маршрута Feature ставится перед маршрутом, её guard выполняются до открытия экрана, а её меню перечисляет экраны:

```rust
Feature::new("stock")
    .route_prefix("/stock")
    .guard(SignedIn)
    .screen(product_list)
    .screen(product_editor)
    .menu(|m| m.group(t!("menu.stock"), Icon::Box).item(t!("stock.products"), product_list))
```

`App::home(screen)` делает так, что `/` открывает экран.

## Страницы ошибок

`App::not_found(screen)` и `App::server_error(screen)` дают приложению собственные страницы для 404 и для ошибок сервера:

```rust
#[screen(public)]
pub fn not_found_page() -> Screen {
    Screen::new(t!("errors.not_found")).body(EmptyState::new(t!("errors.not_found_text")))
}

App::new(t!("app.name")).not_found(not_found_page).server_error(server_error_page)
```

- Экран 404 отвечает на неизвестные пути и на каждый экран, возвращающий `Err(Error::NotFound)`, включая значения пути, которые не разбираются.
- Экран ошибки сервера отвечает на `Error::Internal`, тайм-ауты (504) и паники в обработчиках. Подробности ошибки попадают только в журнал сервера; экран не получает params и ничего не узнаёт о запросе.
- Оба показываются внутри оболочки (shell) с правильным кодом статуса. При навигации внутри приложения приходит только экран, и он подставляется без загрузки страницы. Конечные точки фреймворка и API продолжают отвечать JSON.
- Экраны ошибок не принимают params, не требуют маршрута и не входят в Feature. Делайте их `public`: пользователь, которому нельзя открыть такой экран, или экран ошибки, который сам даёт сбой, получит короткое встроенное сообщение.

## Как экран попадает в браузер

Первый запрос получает полную страницу: меню, экран и ресурсы. Ссылки внутри приложения затем загружают только экран и подставляют его на место, поэтому меню, состояние открытых элементов и положение прокрутки сохраняются. Кнопка «назад», новые вкладки и закладки работают как с обычными ссылками.
