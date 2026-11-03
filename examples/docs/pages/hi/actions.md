# Actions

action सर्वर कोड है जिसे कोई Screen, ग्रिड, फ़ॉर्म, कमांड, JSON API या एजेंट कॉल कर सकता है। `#[action]` घोषित करता है कि इसे कौन कॉल कर सकता है; फ्रेमवर्क कोड चलने से पहले यह जाँचता है।

## action घोषित करना

```rust
#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

- पहला आर्ग्युमेंट इनपुट है। यह क्लाइंट से आता है, इसलिए फ्रेमवर्क इसे अविश्वसनीय JSON की तरह पढ़ता है।
- बाकी हर आर्ग्युमेंट इंजेक्ट होता है: `Ctx` या कोई पंजीकृत `Service`।
- रिटर्न टाइप `Result<T>` है।

## एक्सेस

हर action ठीक इनमें से एक घोषित करती है:

| विकल्प | अर्थ |
|---|---|
| `permission = "feature.resource.verb"` | आइडेंटिटी को यह अनुमति चाहिए |
| `public` | कोई भी, उनमें वे उपयोगकर्ता भी जिन्होंने साइन इन नहीं किया |

बिना एक्सेस वाली action कंपाइल नहीं होती। अनुमति के नाम छोटे अक्षरों में और बिंदु से जुड़े होते हैं, जैसे `sales.order.write`।

अन्य विकल्प:

| विकल्प | अर्थ |
|---|---|
| `audit` | हर कॉल के लिए ऑडिट रिकॉर्ड लिखता है |
| `reauth` | उपयोगकर्ता ने हाल में साइन इन किया होना चाहिए (`App::reauth_within`) |

## त्रुटियाँ

| त्रुटि | स्टेटस | उपयोग |
|---|---|---|
| `Error::NotFound` | 404 | रिकॉर्ड मौजूद नहीं है या उपयोगकर्ता को उसके होने का पता नहीं होना चाहिए |
| `Error::Forbidden` | 403 | एक्सेस अस्वीकृत |
| `Error::Denied(t!("..."))` | 403 | उपयोगकर्ता के पढ़ने लायक कारण के साथ अस्वीकृत |
| `Error::Conflict` | 409 | फ़ॉर्म लोड होने के बाद पंक्ति संस्करण बदल गया |
| `Error::Validation(errors)` | 422 | फ़ील्ड त्रुटियाँ, फ़ील्ड के पास दिखाई जाती हैं |
| `Error::Internal("detail")` | 500 | विवरण केवल सर्वर लॉग में जाता है |

कोड से फ़ील्ड त्रुटि:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## सूचियाँ

सूचियाँ `DataQuery` लेती हैं और `Page<M>` लौटाती हैं। सूची action कभी हर पंक्ति नहीं लौटाती; ग्रिड पृष्ठ माँगता है, और क्लाइंट कुछ भी भेजे, सर्वर पृष्ठ का आकार सीमित रखता है।

## कॉन्टेक्स्ट

`Ctx` अनुरोध है: `ctx.identity()`, `ctx.locale()`, `ctx.time_zone()`, `ctx.now()`, `ctx.today()`, `ctx.tenant()`, `ctx.license()`, `ctx.config()`, `ctx.theme()` और `ctx.t(&text)`। सिस्टम घड़ी के बजाय `ctx.now()` और `ctx.today()` इस्तेमाल करें, ताकि टेस्ट `App::fixed_now` से समय तय कर सकें।

## action कहाँ इस्तेमाल होती हैं

- `DataGrid::of(action)` पंक्तियाँ लोड करती है; `.edit_with(action)`, `.row_action(...)` और `.bulk_action(...)` उन्हें बदलती हैं।
- `Form::submit(label, action)` फ़ॉर्म सहेजती है।
- `ActionButton::new(label, action, input)` बटन से action चलाती है।
- `Command::action(...)` कमांड पैलेट या शॉर्टकट से एक चलाती है।
- `Feature::api(action)` उसे JSON के रूप में प्रकाशित करती है (देखें [API और OpenAPI](api.md))।
- `Feature::search(action)` कमांड पैलेट में सर्वर खोज जोड़ती है।

हर रास्ता वही जाँचें चलाता है: ब्राउज़र कॉल के लिए CSRF, feature के गार्ड, action का एक्सेस, फिर इंटरसेप्टर।
