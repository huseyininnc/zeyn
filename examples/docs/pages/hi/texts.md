# टेक्स्ट और t!

उपयोगकर्ता को दिखने वाला हर टेक्स्ट `t!` से locale फ़ाइलों से आता है। कंपाइलर जाँचता है कि हर locale फ़ाइल में कुंजी है और आर्ग्युमेंट प्लेसहोल्डर से मेल खाते हैं, इसलिए छूटा हुआ अनुवाद बिल्ड त्रुटि है, खाली लेबल नहीं।

## Locale फ़ाइलें

`locales/<lang>.json` नेस्टेड कुंजियाँ रखती है। सभी फ़ाइलों में समान कुंजियाँ होनी चाहिए।

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

`t!` एक `Text` लौटाता है: आर्ग्युमेंट के साथ एक कुंजी, जो पृष्ठ रेंडर होने पर उपयोगकर्ता की भाषा में हल होती है। टाइपो कंपाइल समय पर विफल होते हैं:

```sh
error: t!: key `stock.prodcts` is missing in locales/tr.json
  = help: add it to every file in locales/
```

## फ़ील्ड और choice के लेबल

फ़ील्ड लेबल कुंजी `<Model>.<field>` इस्तेमाल करते हैं, जैसे `Product.code`। choice लेबल `<Choice>.<Variant>` इस्तेमाल करते हैं। जब कुंजी छूटी हो तो लेबल `#[label("...")]` या नाम के पढ़ने योग्य रूप पर लौटता है। `zeyn check` उन कुंजियों को बताता है जो कुछ भाषाओं में हैं और दूसरों में नहीं।

```json
{
  "Product": { "code": "Code", "name": "Product name" },
  "Unit": { "Piece": "Piece", "Kg": "Kilogram" }
}
```

## भाषाएँ पंजीकृत करना

```rust
let i18n = I18n::new("en")
    .catalog("en", include_str!("../locales/en.json"))
    .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))?;
App::new(t!("app.name")).i18n(i18n)
```

`I18n::new` का पहला आर्ग्युमेंट डिफ़ॉल्ट भाषा है। एक से अधिक कैटलॉग होने पर शेल भाषा चयनकर्ता दिखाता है। फ्रेमवर्क के अपने टेक्स्ट (ग्रिड, फ़ॉर्म, शेल) अंग्रेज़ी और तुर्की में आते हैं।

## Screen के बाहर टेक्स्ट

- `ctx.t(&text)` `Text` को उपयोगकर्ता की भाषा में `String` बनाता है, जैसे ईमेल के लिए।
- `Text::english()` लॉग और टेस्ट के लिए अंग्रेज़ी टेक्स्ट देता है।
- `Text::from("literal")` ऐसा टेक्स्ट है जिसका कभी अनुवाद नहीं होता, जैसे उत्पाद कोड।

भाषा कैसे चुनी जाती है और संख्याएँ, मुद्रा और तारीख़ें कैसे स्वरूपित होती हैं, इसके लिए [समय और भाषाएँ](time-and-languages.md) देखें।
