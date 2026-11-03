# Actions

Action (действие) это серверный код, который могут вызвать экран, таблица, форма, команда, JSON API или агент. `#[action]` объявляет, кто может его вызывать; фреймворк проверяет это до выполнения кода.

## Объявление action

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

- Первый аргумент это входные данные. Они приходят от клиента, поэтому фреймворк читает их как недоверенный JSON.
- Все остальные аргументы внедряются: `Ctx` или зарегистрированный `Service`.
- Возвращаемый тип: `Result<T>`.

## Доступ

Каждый action объявляет ровно одно из:

| Параметр | Значение |
|---|---|
| `permission = "feature.resource.verb"` | Субъекту нужно это разрешение |
| `public` | Любой, включая пользователей, которые не вошли в систему |

Action без доступа не компилируется. Имена разрешений пишутся строчными буквами через точки, например `sales.order.write`.

Другие параметры:

| Параметр | Значение |
|---|---|
| `audit` | Записывает запись аудита при каждом вызове |
| `reauth` | Пользователь должен был недавно войти в систему (`App::reauth_within`) |

## Ошибки

| Ошибка | Статус | Применение |
|---|---|---|
| `Error::NotFound` | 404 | Записи не существует или пользователю не следует знать о её существовании |
| `Error::Forbidden` | 403 | Доступ запрещён |
| `Error::Denied(t!("..."))` | 403 | Отказ с причиной, понятной пользователю |
| `Error::Conflict` | 409 | Версия строки изменилась после загрузки формы |
| `Error::Validation(errors)` | 422 | Ошибки полей, показываются рядом с полями |
| `Error::Internal("detail")` | 500 | Подробности попадают только в журнал сервера |

Ошибка поля из кода:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## Списки

Списки принимают `DataQuery` и возвращают `Page<M>`. Action списка никогда не возвращает все строки; таблица запрашивает страницы, а сервер ограничивает размер страницы независимо от того, что отправит клиент.

## Контекст

`Ctx` это запрос: `ctx.identity()`, `ctx.locale()`, `ctx.time_zone()`, `ctx.now()`, `ctx.today()`, `ctx.tenant()`, `ctx.license()`, `ctx.config()`, `ctx.theme()` и `ctx.t(&text)`. Используйте `ctx.now()` и `ctx.today()` вместо системных часов, чтобы тесты могли зафиксировать время через `App::fixed_now`.

## Где используются action

- `DataGrid::of(action)` загружает строки; `.edit_with(action)`, `.row_action(...)` и `.bulk_action(...)` изменяют их.
- `Form::submit(label, action)` сохраняет форму.
- `ActionButton::new(label, action, input)` запускает action по кнопке.
- `Command::action(...)` запускает его из палитры команд или сочетанием клавиш.
- `Feature::api(action)` публикует его как JSON (см. [API and OpenAPI](api.md)).
- `Feature::search(action)` добавляет серверный поиск в палитру команд.

Каждый путь выполняет одни и те же проверки: CSRF для вызовов из браузера, guard Feature, доступ action, затем перехватчики (interceptors).
