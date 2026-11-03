# Satır editörü

Bir modelin `Vec<M>` alanı, bir faturanın satırları gibi bir satır listesidir. `.lines(M::f, ...)` onları form içinde küçük bir grid olarak düzenler: satır ekleyin, kaldırın ve taşıyın, hücreler arasında klavyeyle gezinin ve yazdıkça toplamları görün. Satırlar dahil formun tamamı tek bir action tarafından doğrulanır ve kaydedilir.

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

## Sütunlar ve toplamlar

`.column(Line::f)` türünün denetimiyle düzenlenebilir bir sütun ekler. `.total(quantity, price)` her satır için miktar çarpı fiyatı ve satırların altında toplamı, formun para biriminde gösterir.

## Doğrulama

Her satır kendi modelinin kurallarıyla denetlenir ve hatalar hücrede görünür. `Vec` alanındaki `#[rule(required)]` en az bir satır anlamına gelir. İstemci ne gönderirse göndersin satır sayısı sunucu tarafından sınırlanır (`MAX_LINES`).

## Kaydetmek

Action `Valid<Invoice>` alır; böylece action çalışmadan önce her satır kurallarından geçmiştir:

```rust
#[action(permission = "sales.invoice.write", audit)]
pub async fn save_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    Ok(Money::tl(invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum()))
}
```
