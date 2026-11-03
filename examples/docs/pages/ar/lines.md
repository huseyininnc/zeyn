# محرر البنود

الحقل من النوع `Vec<M>` في نموذج (model) هو قائمة بنود، مثل بنود فاتورة. يحرّرها `.lines(M::f, ...)` كجدول صغير داخل النموذج: إضافة البنود وحذفها ونقلها، والتنقل بين الخلايا بلوحة المفاتيح، ورؤية المجاميع أثناء الكتابة. يُتحقق من النموذج كله، بما فيه البنود، ويُحفظ بإجراء واحد.

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

## الأعمدة والمجاميع

يضيف `.column(Line::f)` عموداً قابلاً للتحرير بعنصر التحكم المناسب لنوعه. يعرض `.total(quantity, price)` الكمية مضروبة في السعر لكل بند والمجموع تحت البنود، بعملة النموذج.

## التحقق

يُفحص كل بند بقواعد نموذجه وتظهر الأخطاء في الخلية. يعني `#[rule(required)]` على حقل `Vec` وجود بند واحد على الأقل. يحدد الخادم الحد الأقصى لعدد البنود (`MAX_LINES`) مهما أرسل العميل.

## الحفظ

يأخذ الإجراء `Valid<Invoice>`، لذلك يكون كل بند قد اجتاز قواعده قبل تشغيل الإجراء:

```rust
#[action(permission = "sales.invoice.write", audit)]
pub async fn save_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    Ok(Money::tl(invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum()))
}
```
