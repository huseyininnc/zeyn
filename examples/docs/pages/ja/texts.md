# テキストと t!

ユーザーに見えるすべてのテキストは、`t!` を通してロケールファイルから取得されます。すべてのロケールファイルにそのキーがあること、引数がプレースホルダーに一致することをコンパイラが検査するため、翻訳の欠落は空白のラベルではなくビルドエラーになります。

## ロケールファイル

`locales/<lang>.json` は入れ子のキーを持ちます。すべてのファイルが同じキーを持つ必要があります。

```json
{
  "stock": {
    "products": "Products",
    "saved": "Product {code} saved."
  }
}
```

## t!

```rust
Screen::new(t!("stock.products"))
ui.toast(t!("stock.saved", code = p.code));
```

`t!` は `Text` を返します。キーと引数の組で、ページのレンダリング時にユーザーの言語で解決されます。タイプミスはコンパイル時に失敗します。

```sh
error: t!: key `stock.prodcts` is missing in locales/tr.json
  = help: add it to every file in locales/
```

語形を変える数は plural プレースホルダーに、値で選ぶ語は select プレースホルダーに書きます。plural のケースは言語の規則 (CLDR の名前で `one`、`few`、`many` など) で決まり、`=0` のような完全一致のケースが優先され、`#` は数を表し、どのプレースホルダーにも `other` ケースが必要です。plural の引数は整数でなければならず、`n = "two"` や小数はコンパイルできません。

```json
"orders": {
  "count": "{n, plural, =0 {No orders} one {# order} other {# orders}}",
  "by": "{role, select, admin {An administrator} other {A colleague}} changed it."
}
```

```rust
t!("orders.count", n = orders.len())
```

## フィールドと選択肢のラベル

フィールドのラベルには `Product.code` のように `<Model>.<field>` のキーを使います。選択肢のラベルには `<Choice>.<Variant>` を使います。キーがない場合、ラベルは `#[label("...")]` か、名前を読みやすくした形にフォールバックします。`zeyn check` は、ある言語にあって別の言語にないキーを報告します。

```json
{
  "Product": { "code": "Code", "name": "Product name" },
  "Unit": { "Piece": "Piece", "Kg": "Kilogram" }
}
```

## 言語の登録

```rust
let i18n = I18n::new("en")
    .catalog("en", include_str!("../locales/en.json"))
    .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))?;
App::new(t!("app.name")).i18n(i18n)
```

`I18n::new` の最初の引数が既定の言語です。カタログが複数あると、シェルに言語ピッカーが表示されます。フレームワーク自身のテキスト（グリッド、フォーム、シェル）は、英語とトルコ語で提供されます。

## ほかの言語のフレームワークテキスト

フレームワーク自身のテキストは、中国語、ロシア語、スペイン語、韓国語、日本語、ヒンディー語、アラビア語でも用意されています。言語ごとに Cargo feature があり、アプリは有効にした言語の分だけを負担します（1 言語あたりバイナリに 35 から 60 KB のテキストが加わります）。

```toml
zeyn = { version = "0.0", features = ["lang-ru", "lang-ar"] }
```

`lang-all` はすべての言語を有効にします。アプリがその言語を提供するのは、上のように自分のカタログを登録したときだけです。feature がない場合、フレームワークのテキストは既定の言語で表示されます。

## 画面の外でのテキスト

- `ctx.t(&text)` は、メールなどのために `Text` をユーザーの言語の `String` に変換します。
- `Text::english()` は、ログとテスト向けに英語のテキストを返します。
- `Text::from("literal")` は、商品コードのように、決して翻訳されないテキストです。

言語がどのように選ばれ、数値、金額、日付がどのように書式化されるかは、[時刻と言語](time-and-languages.md) を参照してください。
