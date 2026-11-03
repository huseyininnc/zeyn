# 明细行编辑器

模型中的 `Vec<M>` 字段是一组明细行，例如发票的明细行。`.lines(M::f, ...)` 将它们作为表单内的一个小型表格进行编辑：添加、删除和移动行，用键盘在单元格之间移动，并在输入时看到合计。整个表单（包括明细行）由同一个 action 校验并保存。

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

## 列与合计

`.column(Line::f)` 添加一个可编辑的列，使用与其类型对应的控件。`.total(quantity, price)` 显示每行的数量乘以单价，并在明细行下方以表单的币种显示总和。

## 校验

每一行都按其模型的规则检查，错误显示在单元格中。`Vec` 字段上的 `#[rule(required)]` 表示至少有一行。无论客户端发送什么，明细行的数量都由服务器限制（`MAX_LINES`）。

## 保存

action 接受 `Valid<Invoice>`，因此在 action 运行之前，每一行都已通过其规则：

```rust
#[action(permission = "sales.invoice.write", audit)]
pub async fn save_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    Ok(Money::tl(invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum()))
}
```
