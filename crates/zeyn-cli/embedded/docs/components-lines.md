# Line editor

A `Vec<M>` field of a model is a list of lines, such as the lines of an invoice. `.lines(M::f, ...)` edits them as a small grid inside the form: add, remove and move lines, move between cells with the keyboard, and see totals as you type. The whole form, lines included, is validated and saved by one action.

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

## Columns and totals

`.column(Line::f)` adds an editable column with the control of its type. `.total(quantity, price)` shows quantity times price for each line and the sum under the lines, in the form's currency.

## Validation

Each line is checked with its model's rules and errors appear in the cell. `#[rule(required)]` on the `Vec` field means at least one line. The number of lines is capped by the server (`MAX_LINES`), whatever the client sends.

## Saving

The action takes `Valid<Invoice>`, so every line has passed its rules before the action runs:

```rust
#[action(permission = "sales.invoice.write", audit)]
pub async fn save_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    Ok(Money::tl(invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum()))
}
```
