# Entity 与 model

数据只描述一次，写成带属性的 Rust 结构体。同一份描述驱动表单、grid 列、服务器端和浏览器端的验证、JSON API 及其 OpenAPI schema。

## Entity

`#[entity(table = "...")]` 标记一个表的行。它需要一个 `#[key]` 字段，并创建一个带类型的 id（例如 `ProductId`），以及每个字段的字段引用（例如 `Product::code`）。

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

字段引用是带类型的：`Product::price` 是 `Field<Product, Money>`。对文本列求 grid 合计，或使用来自另一个 model 的表单字段，都无法编译。

## Model

`#[model]` 是不属于表的行的数据：过滤条件、向导步骤、表单输入。

```rust
#[model]
pub struct Visit {
    #[rule(required, min = 2026_01_01, max = 2027_12_31)]
    pub day: Option<Date>,
    pub arrives: Time,
}
```

## Choice

`#[choice]` 把由单元变体组成的枚举变成下拉框、单选组或分段控件。标签来自语言文件。

```rust
#[choice]
pub enum Unit {
    Piece,
    Kg,
    Liter,
}
```

## 字段属性

| 属性 | 含义 |
|---|---|
| `#[key]` | entity 的主键 |
| `#[version]` | `i64` 类型的行版本。以旧版本保存会以 `Error::Conflict` 失败 |
| `#[readonly]` | 显示，但表单永远不会写入 |
| `#[label("...")]` | 语言文件中没有标签时的备用标签 |
| `#[rule(...)]` | 验证规则，见下文 |

## 规则

| 规则 | 适用于 | 含义 |
|---|---|---|
| `required` | 任意 | 必须有值 |
| `min_len = n`、`max_len = n` | 文本 | 以字符计的长度 |
| `email` | 文本 | 电子邮件地址 |
| `embed` | 文本 | 用 `Embed` 嵌入的地址：位于应用 `frame-src`（`App::csp`）中某个主机上的 `https`。表单还接受粘贴的 `<iframe>` 代码，并且只保存其规范地址，不含跟踪参数。参见[展示](display.md#embed) |
| `min = n`、`max = n` | 数字、金额、小数、日期 | 边界。日期写作 `2026_01_01` |
| `scale = n`、`precision = n` | `Decimal` | 最多的小数位数（没有该规则时为 2）和总共最多的位数（没有该规则时为 18）。它们还决定列类型 `NUMERIC(precision, scale)` |
| `mask = "..."` | 文本 | 原始值填入一个模式：`9` 表示数字，`a` 表示字母，`A` 表示大写字母，`*` 表示两者之一；其他字符在输入时显示但不保存。参见[控件](controls.md#masked-input) |
| `unique` | 任意 | 由 store 检查；表单显示来自 action 的错误 |

不适合字段类型的规则会产生编译错误，并附有说明该怎么做的消息。跨字段的规则写在一个函数中：

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

负责保存的 action 接收 `Valid<M>`。`Valid` 值只能通过运行每条规则来创建，因此 action 不会忘记验证：

```rust
#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

## 字段类型

`String`、`i32`、`i64`、`bool`、`Money`、`Decimal`、`Date`、`Time`、`DateTime`、`LocalDateTime`、`#[choice]` 枚举、entity id，以及上述任意类型的 `Option<T>`。model 的 `Vec<M>` 在表单中会变成行编辑器（参见[行编辑器](lines.md)）。

金额以整数最小货币单位连同其币种存储。绝不要用 `f64` 表示金额。`Decimal` 是用于其他所有带小数的情况的精确数值：数量、超过两位小数的单价、费率。它最多容纳 38 位数字，从不经过浮点数，并在 JSON 中以字符串形式传输（`"12.50"`）。日期和时间见[时间与语言](time-and-languages.md)。
