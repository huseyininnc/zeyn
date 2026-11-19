# Entities and models

Data is described once, as a Rust struct with attributes. The same description drives forms, grid columns, validation on the server and in the browser, the JSON API and its OpenAPI schema.

## Entity

`#[entity(table = "...")]` marks a table row. It needs one `#[key]` field and creates a typed id, such as `ProductId`, and a field reference for each field, such as `Product::code`.

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

Field references are typed: `Product::price` is a `Field<Product, Money>`. A grid total on a text column or a form field from another model does not compile.

## Model

`#[model]` is data that is not a table row: filters, wizard steps, form input.

```rust
#[model]
pub struct Visit {
    #[rule(required, min = 2026_01_01, max = 2027_12_31)]
    pub day: Option<Date>,
    pub arrives: Time,
}
```

## Choice

`#[choice]` turns an enum of unit variants into a select, a radio group or a segmented control. Labels come from the locale files.

```rust
#[choice]
pub enum Unit {
    Piece,
    Kg,
    Liter,
}
```

## Field attributes

| Attribute | Meaning |
|---|---|
| `#[key]` | The primary key of an entity |
| `#[version]` | An `i64` row version. Saves with an old version fail with `Error::Conflict` |
| `#[readonly]` | Shown, never written by forms |
| `#[label("...")]` | Fallback label when the locale files have none |
| `#[rule(...)]` | Validation rules, below |

## Rules

| Rule | On | Meaning |
|---|---|---|
| `required` | any | Must have a value |
| `min_len = n`, `max_len = n` | text | Length in characters |
| `email` | text | An email address |
| `embed` | text | An address to embed with `Embed`: `https` on a host of the app's `frame-src` (`App::csp`). Forms also take pasted `<iframe>` code and store only its canonical address, without tracking parameters. See [Display](display.md#embed) |
| `min = n`, `max = n` | numbers, money, decimals, dates | Bounds. Dates are written `2026_01_01` |
| `scale = n`, `precision = n` | `Decimal` | Most decimals (2 without the rule) and most digits in all (18 without the rule). They also set the column type, `NUMERIC(precision, scale)` |
| `mask = "..."` | text | The raw value fills a pattern: `9` a digit, `a` a letter, `A` an upper case letter, `*` either; other characters are shown while typing but not stored. See [Controls](controls.md#masked-input) |
| `unique` | any | Checked by the store; the form shows the error from the action |

A rule that does not fit the field type is a compile error with a message that says what to do. Rules across fields go in a function:

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

An action that saves takes `Valid<M>`. A `Valid` value can only be made by running every rule, so an action cannot forget to validate:

```rust
#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

## Field types

`String`, `i32`, `i64`, `bool`, `Money`, `Decimal`, `Date`, `Time`, `DateTime`, `LocalDateTime`, `#[choice]` enums, entity ids and `Option<T>` of any of them. `Vec<M>` of a model becomes a line editor in forms (see [Line editor](lines.md)).

Money is stored in integer minor units with its currency. Never use `f64` for money. `Decimal` is an exact number for everything else with a fraction: quantities, unit prices with more than two decimals, rates. It holds up to 38 digits, never goes through a float, and travels in JSON as a string (`"12.50"`). Dates and times are covered in [Time and languages](time-and-languages.md).
