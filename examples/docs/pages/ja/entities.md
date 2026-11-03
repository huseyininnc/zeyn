# エンティティとモデル

データは、属性付きの Rust の構造体として一度だけ記述します。同じ記述が、フォーム、グリッドの列、サーバーとブラウザでの検証、JSON API とその OpenAPI スキーマを駆動します。

## Entity

`#[entity(table = "...")]` はテーブルの行を表します。`#[key]` フィールドが 1 つ必要で、`ProductId` のような型付き ID と、`Product::code` のような各フィールドへのフィールド参照が生成されます。

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

フィールド参照には型があります。`Product::price` は `Field<Product, Money>` です。テキスト列に対するグリッドの合計や、別のモデルのフォームフィールドはコンパイルできません。

## Model

`#[model]` は、テーブルの行ではないデータです。フィルター、ウィザードのステップ、フォームの入力などに使います。

```rust
#[model]
pub struct Visit {
    #[rule(required, min = 2026_01_01, max = 2027_12_31)]
    pub day: Option<Date>,
    pub arrives: Time,
}
```

## Choice

`#[choice]` は、単位バリアントだけの列挙型を、セレクト、ラジオグループ、セグメントコントロールに変えます。ラベルはロケールファイルから取得されます。

```rust
#[choice]
pub enum Unit {
    Piece,
    Kg,
    Liter,
}
```

## フィールド属性

| 属性 | 意味 |
|---|---|
| `#[key]` | エンティティの主キー |
| `#[version]` | `i64` の行バージョン。古いバージョンでの保存は `Error::Conflict` で失敗します |
| `#[readonly]` | 表示のみで、フォームからは書き込まれません |
| `#[label("...")]` | ロケールファイルにラベルがない場合の代替ラベル |
| `#[rule(...)]` | 検証ルール（以下） |

## ルール

| ルール | 対象 | 意味 |
|---|---|---|
| `required` | すべて | 値が必要です |
| `min_len = n`, `max_len = n` | テキスト | 文字数による長さ |
| `email` | テキスト | メールアドレス |
| `embed` | テキスト | `Embed` で埋め込むアドレス。アプリの `frame-src`（`App::csp`）にあるホストの `https` です。フォームは貼り付けられた `<iframe>` コードも受け付け、トラッキングパラメータを除いた正規のアドレスだけを保存します。[表示](display.md#embed) を参照 |
| `min = n`, `max = n` | 数値、金額、小数、日付 | 範囲。日付は `2026_01_01` のように書きます |
| `scale = n`, `precision = n` | `Decimal` | 小数部の最大桁数（ルールなしでは 2）と、全体の最大桁数（ルールなしでは 18）。列の型 `NUMERIC(precision, scale)` も決まります |
| `mask = "..."` | テキスト | 生の値がパターンに当てはめられます。`9` は数字、`a` は文字、`A` は大文字、`*` はそのどちらでも。それ以外の文字は入力中に表示されますが保存されません。[コントロール](controls.md#masked-input) を参照 |
| `unique` | すべて | ストアが検査します。フォームにはアクションからのエラーが表示されます |

フィールドの型に合わないルールは、何をすべきかを示すメッセージ付きのコンパイルエラーになります。フィールドをまたぐルールは関数に書きます。

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

保存するアクションは `Valid<M>` を受け取ります。`Valid` 値はすべてのルールを実行しないと作れないため、アクションが検証を忘れることはありません。

```rust
#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

## フィールドの型

`String`、`i32`、`i64`、`bool`、`Money`、`Decimal`、`Date`、`Time`、`DateTime`、`LocalDateTime`、`#[choice]` の列挙型、エンティティ ID、およびそれらの `Option<T>`。モデルの `Vec<M>` は、フォームでは明細エディタになります（[明細エディタ](lines.md) を参照）。

Money は通貨とともに、整数の最小単位で保存されます。金額に `f64` を使ってはいけません。`Decimal` は、数量、小数点以下 3 桁以上の単価、レートなど、小数を持つそれ以外すべてのための正確な数値です。最大 38 桁を保持し、浮動小数点を経由せず、JSON では文字列（`"12.50"`）として運ばれます。日付と時刻は [時刻と言語](time-and-languages.md) で説明します。
