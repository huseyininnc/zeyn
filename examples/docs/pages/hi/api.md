# API और OpenAPI

कोई भी action JSON एंडपॉइंट भी हो सकती है। वही इनपुट टाइप, वही नियम और वही अनुमति लागू होती है, इसलिए API स्क्रीन से अलग नहीं हो सकती।

## action प्रकाशित करना

```rust
Feature::new("stock")
    .api(search_products)
    .api(save_product)
    .api(restock_product)
```

हर action `POST /api/v1/{feature}/{action}` बन जाती है, जैसे `POST /api/v1/stock/save_product`। बॉडी JSON के रूप में action का इनपुट है और उत्तर JSON के रूप में उसका आउटपुट।

```sh
curl -X POST http://127.0.0.1:3002/api/v1/stock/search_products \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"sort":[{"field":"code","desc":false}]}'
```

`App::api_prefix("/api/v2")` प्रीफ़िक्स बदलता है।

## कॉलर के लिए नियम

- कॉल में `Authorization` हेडर चाहिए। ऐप का ऑथेंटिकेटर उसे पढ़ता है; केवल ब्राउज़र सत्र कुकी अस्वीकार की जाती है, इसलिए API को CSRF टोकन की ज़रूरत नहीं।
- एक्सेस, गार्ड, इंटरसेप्टर, ऑडिट और दर सीमाएँ ठीक वैसे ही लागू होती हैं जैसे स्क्रीन पर।
- `Valid<M>` लेने वाले इनपुट हर `#[rule]` चलाते हैं। फ़ील्ड त्रुटियाँ हर फ़ील्ड के संदेश के साथ `422` के रूप में लौटती हैं।
- सूचियाँ `DataQuery` लेती हैं और `Page` लौटाती हैं; सर्वर पृष्ठ का आकार सीमित रखता है।

## त्रुटियाँ

त्रुटियाँ RFC 9457 problem details (`application/problem+json`) हैं:

```json
{
  "type": "urn:zeyn:problem:validation",
  "title": "Unprocessable Entity",
  "status": 422,
  "detail": "Some fields are not valid.",
  "errors": [{ "field": "code", "rule": "unique", "message": "This code is already used." }]
}
```

| स्टेटस | `type` का अंत | कब |
|---|---|---|
| 403 | `forbidden` | `Authorization` हेडर नहीं है, या अनुमति नहीं है |
| 404 | `not_found` | अज्ञात रिकॉर्ड, या ऐसा इनपुट जो पार्स नहीं होता |
| 409 | `conflict` | पंक्ति संस्करण बदल गया |
| 422 | `validation` | फ़ील्ड त्रुटियाँ |
| 429 | `forbidden` | बहुत अधिक अनुरोध; `Retry-After` देखें |
| 504 | `timeout` | कॉल ने सर्वर की सीमा से अधिक समय लिया |

## OpenAPI

```rust
App::new(t!("app.name")).api_docs(ApiDocs::open())
```

`api_docs` के साथ ऐप `{api_prefix}/openapi.json` (OpenAPI 3.1; इसे Postman, क्लाइंट जनरेटर या किसी भी OpenAPI व्यूअर में इम्पोर्ट करें) और `{api_prefix}/docs` पर पढ़ने योग्य संदर्भ प्रकाशित करता है। स्कीमा मॉडल से आते हैं: प्रकार, आवश्यक फ़ील्ड, लंबाइयाँ, सीमाएँ और choice मान।

दस्तावेज़ हर एंडपॉइंट और उसकी ज़रूरी अनुमति सूचीबद्ध करते हैं, इसलिए यह डिफ़ॉल्ट रूप से बंद है। चुनें कि इसे कौन पढ़ सकता है:

| एक्सेस | कौन |
|---|---|
| `ApiDocs::open()` | कोई भी |
| `ApiDocs::basic(credentials)` | HTTP basic ऑथेंटिकेशन, जैसे `StaticCredentials::new("docs", password)` |
| `ApiDocs::permission("api.docs.read")` | अनुमति वाले साइन इन उपयोगकर्ता |

## अन्य origin पर ब्राउज़र ऐप

`App::cors(["https://app.example.com"])` इन ठीक-ठीक origin पर चलने वाले ब्राउज़र ऐप को `Authorization` हेडर के साथ API कॉल करने देता है। कुकी origin के पार कभी नहीं भेजी जातीं।
