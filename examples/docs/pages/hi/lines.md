# लाइन एडिटर

किसी मॉडल का `Vec<M>` फ़ील्ड पंक्तियों की सूची है, जैसे इनवॉइस की पंक्तियाँ। `.lines(M::f, ...)` उन्हें फ़ॉर्म के भीतर छोटे ग्रिड के रूप में संपादित करता है: पंक्तियाँ जोड़ें, हटाएँ और हिलाएँ, कीबोर्ड से सेल के बीच चलें, और टाइप करते हुए योग देखें। पूरा फ़ॉर्म, पंक्तियों सहित, एक action द्वारा सत्यापित और सहेजा जाता है।

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

## कॉलम और योग

`.column(Line::f)` अपने प्रकार के कंट्रोल के साथ एक संपादन योग्य कॉलम जोड़ता है। `.total(quantity, price)` हर पंक्ति के लिए मात्रा गुणा कीमत और पंक्तियों के नीचे योग, फ़ॉर्म की करेंसी में, दिखाता है।

## सत्यापन

हर पंक्ति अपने मॉडल के नियमों से जाँची जाती है और त्रुटियाँ सेल में दिखती हैं। `Vec` फ़ील्ड पर `#[rule(required)]` का अर्थ है कम से कम एक पंक्ति। क्लाइंट कुछ भी भेजे, पंक्तियों की संख्या सर्वर सीमित रखता है (`MAX_LINES`)।

## सहेजना

action `Valid<Invoice>` लेती है, इसलिए action चलने से पहले हर पंक्ति अपने नियम पास कर चुकी होती है:

```rust
#[action(permission = "sales.invoice.write", audit)]
pub async fn save_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    Ok(Money::tl(invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum()))
}
```
