# Entity और मॉडल

डेटा का वर्णन एक बार होता है, एट्रिब्यूट वाले Rust struct के रूप में। वही वर्णन फ़ॉर्म, ग्रिड कॉलम, सर्वर और ब्राउज़र में सत्यापन, JSON API और उसके OpenAPI स्कीमा को चलाता है।

## Entity

`#[entity(table = "...")]` तालिका की पंक्ति को चिह्नित करता है। इसे एक `#[key]` फ़ील्ड चाहिए और यह टाइप्ड आईडी, जैसे `ProductId`, और हर फ़ील्ड के लिए फ़ील्ड संदर्भ, जैसे `Product::code`, बनाता है।

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

फ़ील्ड संदर्भ टाइप्ड हैं: `Product::price` एक `Field<Product, Money>` है। टेक्स्ट कॉलम पर ग्रिड योग या दूसरे मॉडल का फ़ॉर्म फ़ील्ड कंपाइल नहीं होता।

## मॉडल

`#[model]` ऐसा डेटा है जो तालिका की पंक्ति नहीं है: फ़िल्टर, विज़ार्ड चरण, फ़ॉर्म इनपुट।

```rust
#[model]
pub struct Visit {
    #[rule(required, min = 2026_01_01, max = 2027_12_31)]
    pub day: Option<Date>,
    pub arrives: Time,
}
```

## Choice

`#[choice]` यूनिट वेरिएंट वाले enum को select, रेडियो समूह या segmented कंट्रोल में बदलता है। लेबल locale फ़ाइलों से आते हैं।

```rust
#[choice]
pub enum Unit {
    Piece,
    Kg,
    Liter,
}
```

## फ़ील्ड एट्रिब्यूट

| एट्रिब्यूट | अर्थ |
|---|---|
| `#[key]` | Entity की प्राथमिक कुंजी |
| `#[version]` | `i64` पंक्ति संस्करण। पुराने संस्करण से सेव `Error::Conflict` के साथ विफल होते हैं |
| `#[readonly]` | दिखाया जाता है, फ़ॉर्म द्वारा कभी लिखा नहीं जाता |
| `#[label("...")]` | locale फ़ाइलों में न होने पर वैकल्पिक लेबल |
| `#[rule(...)]` | सत्यापन नियम, नीचे |

## नियम

| नियम | किस पर | अर्थ |
|---|---|---|
| `required` | कोई भी | मान होना चाहिए |
| `min_len = n`, `max_len = n` | टेक्स्ट | अक्षरों में लंबाई |
| `email` | टेक्स्ट | ईमेल पता |
| `embed` | टेक्स्ट | `Embed` से embed करने का पता: ऐप के `frame-src` (`App::csp`) के होस्ट पर `https`। फ़ॉर्म चिपकाया गया `<iframe>` कोड भी लेते हैं और केवल उसका कैनोनिकल पता, ट्रैकिंग पैरामीटर के बिना, सहेजते हैं। देखें [डिस्प्ले](display.md#embed) |
| `min = n`, `max = n` | संख्याएँ, मुद्रा, decimal, तारीख़ें | सीमाएँ। तारीख़ें `2026_01_01` के रूप में लिखी जाती हैं |
| `scale = n`, `precision = n` | `Decimal` | अधिकतम दशमलव (नियम के बिना 2) और कुल अधिकतम अंक (नियम के बिना 18)। ये कॉलम का प्रकार भी तय करते हैं, `NUMERIC(precision, scale)` |
| `mask = "..."` | टेक्स्ट | कच्चा मान एक पैटर्न भरता है: `9` अंक, `a` अक्षर, `A` बड़ा अक्षर, `*` दोनों में से कोई; अन्य वर्ण टाइप करते समय दिखते हैं पर सहेजे नहीं जाते। देखें [कंट्रोल](controls.md#masked-input) |
| `unique` | कोई भी | स्टोर जाँचता है; फ़ॉर्म action की त्रुटि दिखाता है |

जो नियम फ़ील्ड के प्रकार से मेल नहीं खाता वह कंपाइल त्रुटि है, ऐसे संदेश के साथ जो बताता है कि क्या करना है। फ़ील्ड के पार के नियम फ़ंक्शन में जाते हैं:

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

सहेजने वाली action `Valid<M>` लेती है। `Valid` मान केवल हर नियम चलाकर बन सकता है, इसलिए action सत्यापन करना भूल नहीं सकती:

```rust
#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

## फ़ील्ड प्रकार

`String`, `i32`, `i64`, `bool`, `Money`, `Decimal`, `Date`, `Time`, `DateTime`, `LocalDateTime`, `#[choice]` enum, entity आईडी और इनमें से किसी का `Option<T>`। किसी मॉडल का `Vec<M>` फ़ॉर्म में लाइन एडिटर बन जाता है (देखें [लाइन एडिटर](lines.md))।

मुद्रा पूर्णांक न्यूनतम इकाइयों में अपनी करेंसी के साथ सहेजी जाती है। मुद्रा के लिए कभी `f64` इस्तेमाल न करें। `Decimal` भिन्न वाली हर दूसरी चीज़ के लिए सटीक संख्या है: मात्राएँ, दो से अधिक दशमलव वाली इकाई कीमतें, दरें। यह 38 अंकों तक रखता है, कभी फ़्लोट से नहीं गुज़रता, और JSON में स्ट्रिंग (`"12.50"`) के रूप में जाता है। तारीख़ें और समय [समय और भाषाएँ](time-and-languages.md) में हैं।
