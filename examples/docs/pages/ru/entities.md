# Сущности и модели

Данные описываются один раз, как структура Rust с атрибутами. Это же описание управляет формами, столбцами таблицы, проверкой на сервере и в браузере, JSON API и его схемой OpenAPI.

## Entity

`#[entity(table = "...")]` помечает строку таблицы. Ему нужно одно поле `#[key]`; он создаёт типизированный идентификатор, например `ProductId`, и ссылку на каждое поле, например `Product::code`.

```rust
#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20, unique)]
    pub code: String,
    #[rule(required, max_len = 200)]
    pub name: String,
    pub unit: Unit,
    #[rule(min = 0)]
    pub price: Money,
    #[readonly]
    pub stock: i64,
    pub active: bool,
    #[version]
    pub version: i64,
}
```

Ссылки на поля типизированы: `Product::price` это `Field<Product, Money>`. Итог таблицы по текстовому столбцу или поле формы из другой модели не скомпилируются.

## Model

`#[model]` это данные, которые не являются строкой таблицы: фильтры, шаги мастера, ввод формы.

```rust
#[model]
pub struct Visit {
    #[rule(required, min = 2026_01_01, max = 2027_12_31)]
    pub day: Option<Date>,
    pub arrives: Time,
}
```

## Choice

`#[choice]` превращает enum из вариантов без данных в выпадающий список, группу переключателей или сегментированный переключатель. Подписи берутся из файлов локалей.

```rust
#[choice]
pub enum Unit {
    Piece,
    Kg,
    Liter,
}
```

## Атрибуты полей

| Атрибут | Значение |
|---|---|
| `#[key]` | Первичный ключ entity |
| `#[version]` | Версия строки типа `i64`. Сохранение со старой версией завершается ошибкой `Error::Conflict` |
| `#[readonly]` | Показывается, но формы никогда его не записывают |
| `#[label("...")]` | Запасная подпись, если в файлах локалей её нет |
| `#[rule(...)]` | Правила проверки, см. ниже |

## Правила

| Правило | Для | Значение |
|---|---|---|
| `required` | любого | Значение обязательно |
| `min_len = n`, `max_len = n` | текста | Длина в символах |
| `email` | текста | Адрес электронной почты |
| `embed` | текста | Адрес для встраивания через `Embed`: `https` на хосте из `frame-src` приложения (`App::csp`). Формы также принимают вставленный код `<iframe>` и сохраняют только его канонический адрес, без параметров отслеживания. См. [Display](display.md#embed) |
| `min = n`, `max = n` | чисел, денег, десятичных чисел, дат | Границы. Даты записываются как `2026_01_01` |
| `scale = n`, `precision = n` | `Decimal` | Наибольшее число знаков после запятой (2 без правила) и наибольшее общее число цифр (18 без правила). Они также задают тип столбца, `NUMERIC(precision, scale)` |
| `mask = "..."` | текста | Исходное значение заполняет шаблон: `9` цифра, `a` буква, `A` заглавная буква, `*` любой из них; остальные символы показываются при вводе, но не сохраняются. См. [Controls](controls.md#masked-input) |
| `unique` | любого | Проверяется хранилищем; форма показывает ошибку из action |

Правило, не подходящее к типу поля, вызывает ошибку компиляции с сообщением о том, что нужно сделать. Правила, охватывающие несколько полей, задаются функцией:

```rust
#[model(validate = check_stay)]
pub struct Stay {
    pub from: Option<Date>,
    pub to: Option<Date>,
}

fn check_stay(s: &Stay, errors: &mut ValidationErrors) {
    if let (Some(a), Some(b)) = (s.from, s.to) {
        if b < a {
            errors.add(Stay::to, t!("stay.before_start"));
        }
    }
}
```

## Valid

Action, который сохраняет данные, принимает `Valid<M>`. Значение `Valid` можно создать только выполнив все правила, поэтому action не может забыть о проверке:

```rust
#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

## Типы полей

`String`, `i32`, `i64`, `bool`, `Money`, `Decimal`, `Date`, `Time`, `DateTime`, `LocalDateTime`, enum `#[choice]`, идентификаторы entity и `Option<T>` любого из них. `Vec<M>` из модели становится в формах редактором строк (см. [Line editor](lines.md)).

Money хранится в целых минимальных единицах вместе с валютой. Никогда не используйте `f64` для денег. `Decimal` это точное число для всего остального, что имеет дробную часть: количеств, цен с более чем двумя знаками после запятой, ставок. Он вмещает до 38 цифр, никогда не проходит через число с плавающей запятой и передаётся в JSON как строка (`"12.50"`). Даты и время описаны в разделе [Time and languages](time-and-languages.md).
