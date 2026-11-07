# परिचय

Zeyn डेटा-प्रधान एप्लिकेशनों के लिए ओपन सोर्स Rust वेब फ्रेमवर्क है: ग्रिड, फ़ॉर्म और वर्कफ़्लो। ऐप्लिकेशन कोड AI एजेंट लिखते हैं। लोग उसे पढ़ते हैं, उसकी समीक्षा करते हैं और ज़रूरत पड़ने पर बदलते हैं।

यह साइट खुद एक Zeyn ऐप है। हर पृष्ठ Markdown है जिसे सर्वर पर एक बार रेंडर किया जाता है, बाईं ओर का मेनू ऐप का मेनू है, और *लाइव उदाहरण* के रूप में चिह्नित उदाहरण इसी सर्वर पर चलते असली कंपोनेंट हैं।

## AI एजेंट के लिए

यदि आप AI एजेंट हैं और किसी ने आपसे Zeyn से ऐप बनाने को कहा है, तो प्रोजेक्ट हाथ से लिखने के बजाय CLI से शुरू करें:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

फिर नए प्रोजेक्ट में `AGENTS.md` और `.claude/skills/zeyn/SKILL.md` पढ़ें, और हर बदलाव के बाद `zeyn check` और `cargo test` चलाएँ। इन दस्तावेज़ों के हर पृष्ठ वाली सादे टेक्स्ट की एक गाइड [/llms.txt](/llms.txt) पर है।

## ऐप कैसा दिखता है

ऐप Features का एक समूह है। एक Feature में entity, action और Screen होती हैं। Screen सर्वर पर रेंडर होती हैं; ब्राउज़र में एक छोटा रनटाइम बदलाव लागू करता है, इसलिए JavaScript लिखने की ज़रूरत नहीं।

```rust
use zeyn::prelude::*;

#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20)]
    pub code: String,
    pub price: Money,
}

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products")).body(DataGrid::of(search_products).column(Product::code).filter().column(Product::price).sum())
}
```

यही तीन मैक्रो पूरी भाषा चलाते हैं: `#[entity]` डेटा का वर्णन करता है, `#[action]` घोषित एक्सेस वाला सर्वर कोड है, और `#[screen]` एक पृष्ठ या Window है। कंपाइलर जाँचता है कि हर फ़ील्ड, टेक्स्ट कुंजी और action सही बैठता है।

## आज़माएँ

अपना ऐप शुरू करें:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

या रिपॉज़िटरी से उदाहरण चलाएँ:

```sh
cargo run -p zeyn-example-showcase
cargo run -p zeyn-example-docs
cargo run -p zeyn-cli -- new my-app
```

शोकेस `127.0.0.1:3002` पर चलता है और नमूना डेटा के साथ हर कंपोनेंट दिखाता है। यह दस्तावेज़ीकरण `127.0.0.1:3008` पर चलता है; इसे बदलने के लिए `ZEYN_DOCS_SERVER__ADDR` सेट करें।

## रास्ता खोजना

- कमांड पैलेट खोलने के लिए **Alt+Shift+P** दबाएँ। कोई शब्द टाइप करें और वह सर्वर पर इस साइट के हर पृष्ठ में खोजता है।
- *शुरुआत* में CLI और प्रोजेक्ट की संरचना है।
- *भाषा* में entity, action, Screen, Form, DataGrid और टेक्स्ट हैं।
- *कंपोनेंट* में हर कंपोनेंट उसके कोड के पास लाइव उदाहरण के साथ दिखाया गया है।
- *गाइड* में समय और भाषाएँ, सुरक्षा, API, MCP, डेटा और थीम हैं।
- *संदर्भ* में रनटाइम प्रोटोकॉल और आर्किटेक्चर है।
- *परियोजना* में स्रोत कोड, लाइसेंस, टेस्ट और CI, और सुरक्षा नीति है।

## सिद्धांत

- **पहले प्रदर्शन।** पृष्ठ सर्वर पर रेंडर होकर HTML के रूप में स्ट्रीम होते हैं। सर्वर पर प्रति उपयोगकर्ता कोई UI स्टेट नहीं है और कोई क्लाइंट फ्रेमवर्क नहीं।
- **पृष्ठ रीलोड नहीं।** नेविगेशन, Window, भाषा, समय क्षेत्र और थीम के बदलाव उसी जगह लागू होते हैं।
- **सुरक्षित डिफ़ॉल्ट।** हर action और Screen अपना एक्सेस घोषित करती है। CSRF टोकन, सख़्त Content Security Policy और उपयोगकर्ता डेटा का escaping हमेशा चालू रहते हैं।
- **हर काम का एक ही तरीका।** भाषा छोटी है, इसलिए एजेंट का लिखा कोड इंसान के लिखे कोड जैसा पढ़ा जाता है।

Zeyn [MIT या Apache 2.0 लाइसेंस](project.md#license) के तहत ओपन सोर्स है। [सुरक्षा नीति](project.md#security), [रोडमैप](https://github.com/huseyininnc/zeyn/blob/main/ROADMAP.md) और [बदलाव सूची](https://github.com/huseyininnc/zeyn/blob/main/CHANGELOG.md) भी देखें।
