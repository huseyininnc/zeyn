# 明細エディタ

モデルの `Vec<M>` フィールドは、請求書の明細のような行のリストです。`.lines(M::f, ...)` は、それらをフォーム内の小さなグリッドとして編集します。行の追加、削除、移動ができ、キーボードでセル間を移動でき、入力しながら合計が表示されます。明細を含むフォーム全体が、1 つのアクションで検証、保存されます。

```rust
#[model]
pub struct Line {
    #[rule(required, max_len = 80)]
    pub item: String,
    #[rule(min = 1, max = 999)]
    pub quantity: i64,
    #[rule(min = 0)]
    pub unit_price: Money,
}

#[model]
pub struct Invoice {
    #[rule(required, max_len = 80)]
    pub customer: String,
    #[rule(required)]
    pub lines: Vec<Line>,
}

Form::of(invoice)
    .field(Invoice::customer)
    .lines(Invoice::lines, |l| {
        l.column(Line::item)
            .column(Line::quantity)
            .column(Line::unit_price)
            .total(Line::quantity, Line::unit_price)
    })
    .submit(t!("docs.live.save"), save_invoice)
```

@live lines

## 列と合計

`.column(Line::f)` は、その型のコントロールを持つ編集可能な列を追加します。`.total(quantity, price)` は、各行の数量かける単価と、明細の下に合計を、フォームの通貨で表示します。

## 検証

各行はそのモデルのルールで検査され、エラーはセルに表示されます。`Vec` フィールドの `#[rule(required)]` は、少なくとも 1 行が必要という意味です。クライアントが何を送っても、行数にはサーバーが上限（`MAX_LINES`）を設けます。

## 保存

アクションは `Valid<Invoice>` を受け取るため、アクションが実行される前にすべての行がルールを通過しています。

```rust
#[action(permission = "sales.invoice.write", audit)]
pub async fn save_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    Ok(Money::tl(invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum()))
}
```
