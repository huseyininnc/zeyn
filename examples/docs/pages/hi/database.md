# डेटा और डेटाबेस

Zeyn डेटाबेस को ऑब्जेक्ट मैपर के पीछे नहीं छिपाता। डेटा उन सेवाओं (स्टोर) में रहता है जिन्हें ऐप पंजीकृत करता है; action उन्हें आर्ग्युमेंट के रूप में पाती हैं। स्टोर पंक्तियाँ मेमोरी में रख सकता है, डेटाबेस को कॉल कर सकता है, या किसी दूसरी सेवा को कॉल कर सकता है। Screen, ग्रिड और API केवल action देखते हैं।

## एक स्टोर

```rust
#[derive(Clone)]
pub struct ProductStore {
    db: Pool,
}

impl Service for ProductStore {}

impl ProductStore {
    pub async fn get(&self, id: ProductId) -> Result<Product> {
        self.db.product(id).await.map_err(|e| Error::Internal(e.to_string()))?.ok_or(Error::NotFound)
    }
}
```

इसे `App::service(store)` से एक बार पंजीकृत करें। जब किसी Screen या API को ऐसा स्टोर चाहिए जो पंजीकृत नहीं है, तो ऐप शुरू होने से मना कर देता है और बताता है कि कौन सा।

## सूचियाँ: DataQuery और Page

सूची action `DataQuery` लेती है और `Page<M>` लौटाती है:

| फ़ील्ड | अर्थ |
|---|---|
| `page` | क्रमांकित पृष्ठों के लिए `Offset { offset, limit }`, या अनंत स्क्रॉल के लिए `After { cursor, limit }` |
| `sort` | फ़ील्ड और दिशाएँ, क्रम से |
| `filters` | फ़ील्ड, ऑपरेटर और मान, कॉलम के प्रकार के अनुसार |
| `with_total` | क्या क्लाइंट को कुल गिनती चाहिए |
| `aggregates` | हर फ़िल्टर की गई पंक्ति पर गणना किए जाने वाले कॉलम योग |
| `time_zone` | `DateTime` कॉलम पर तारीख़ फ़िल्टर का क्षेत्र, सर्वर तय करता है |
| `group` | किसी फ़ील्ड से पंक्तियों को समूहित करें, ग्रिड तय करता है |

`DataQuery::apply(vec)` यह सब मेमोरी में करता है। SQL डेटाबेस के लिए `zeyn::sql` इस्तेमाल करें: `Db::find` उसी क्वेरी को PostgreSQL या Oracle के लिए `WHERE`, `ORDER BY`, `LIMIT` और `GROUP BY` में बदल देता है। केवल entity के अपने फ़ील्ड फ़िल्टर या क्रमबद्ध किए जा सकते हैं, हर मान पैरामीटर के रूप में बाँधा जाता है, और पृष्ठ का आकार सर्वर सीमित रखता है, इसलिए क्लाइंट इनपुट कभी SQL टेक्स्ट तक नहीं पहुँचता। SQL हाथ से केवल उन क्वेरी के लिए लिखें जिन्हें `Db::find` व्यक्त नहीं कर सकता, और तब भी क्लाइंट से आया कोई फ़ील्ड नाम या मान उसमें कभी न चिपकाएँ।

```toml
zeyn = { version = "0.0.6", features = ["postgres"] }   # or ["oracle"]
```

```rust
let db = Db::from_config(&config).await?;            // reads the secret database.url
App::new(t!("app.name")).config(config).service(db)

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, db: Db) -> Result<Page<Product>> {
    db.find(&q).await
}
```

[डेटाबेस संदर्भ](/reference/database) में कनेक्शन सेटिंग, Oracle, माइग्रेशन, ट्रांज़ैक्शन, स्ट्रीम और हर क्वेरी जिस SQL में बदलती है वह है।

`Page` पंक्तियाँ, माँगने पर कुल, अगले बैच का कर्सर और योग रखता है:

```rust
#[action(permission = "sales.order.read")]
pub async fn search_orders(q: DataQuery, store: OrderStore) -> Result<Page<Order>> {
    store.search(&q).await
}
```

## पंक्ति संस्करण

`#[version] pub version: i64` फ़ील्ड आशावादी लॉकिंग चालू करता है। फ़ॉर्म वह संस्करण भेजते हैं जो उन्होंने लोड किया; स्टोर उसकी तुलना करता है और बीच में किसी के सहेजने पर `Error::Conflict` लौटाता है। फिर फ़ॉर्म सहकर्मी का बदलाव ओवरराइट करने के बजाय उपयोगकर्ता को बताता है।

```rust
if current.version != product.version {
    return Err(Error::Conflict);
}
product.version += 1;
```

## स्टोर में सत्यापन

जिन नियमों को डेटा चाहिए, जैसे `unique`, उन्हें स्टोर में जाँचा जाता है और फ़ील्ड त्रुटियों के रूप में लौटाया जाता है:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## टेनेंट

`App::tenancy(resolver)` के साथ हर अनुरोध एक टेनेंट (`ctx.tenant()`) रखता है, होस्ट नाम (`SubdomainTenant`) या हेडर (`HeaderTenant`) से, या जब अनुरोध कोई नाम न दे तब उपयोगकर्ता के अपने टेनेंट से। साइन इन उपयोगकर्ता को अनुरोध के टेनेंट का सदस्य होना चाहिए (`Identity::member_of`), वरना अनुरोध साइन आउट की तरह चलता है। स्टोर इसे स्कीमा चुनने या पंक्तियाँ फ़िल्टर करने के लिए इस्तेमाल करता है; `TenantRequired` बिना टेनेंट वाले अनुरोध अस्वीकार करता है।

## सहेजी गई उपयोगकर्ता स्थिति

ग्रिड व्यू (कॉलम, क्रम, फ़िल्टर), डॉक लेआउट और चुनी गई थीम `App::datagrid_views(store)` से सर्वर पर प्रति उपयोगकर्ता रखी जा सकती हैं। `MemoryViewStore` डेवलपमेंट के लिए उन्हें मेमोरी में रखता है; प्रोडक्शन के लिए ऐप अपने डेटाबेस के साथ `DataGridViewStore` लागू करता है।

## बैकग्राउंड काम

इम्पोर्ट, एक्सपोर्ट और अन्य लंबे काम प्रगति के साथ बैकग्राउंड जॉब (`Jobs`) के रूप में चलते हैं, ताकि अनुरोध को उनका इंतज़ार न करना पड़े। देखें [ऑडिट, मास्किंग और जॉब](enterprise.md)।

जॉब तब तक मेमोरी में रहते हैं जब तक ऐप उन्हें स्टोर न दे। `SqlJobStore` के साथ कतार में लगे रन और हर शेड्यूल का अगला रन डेटाबेस में रखे जाते हैं: वे रीस्टार्ट में बचे रहते हैं, कई सर्वर एक कतार साझा करते हैं, और एक लीज़ सुनिश्चित करती है कि दो वर्कर एक ही रन कभी न चलाएँ।

```rust
let jobs = SqlJobStore::new(db.clone());
jobs.create_tables().await?;

App::new("Billing")
    .job_store(jobs)
    .queued_job("send_invoice", Schedule::manual().retry(5, Duration::from_secs(10)), send_invoice)
    .job("close_day", Schedule::every(Duration::from_secs(86_400)), close_day)

async fn send_invoice(ctx: Ctx, invoice_id: String) -> Result<()> { ... }

#[action(permission = "billing.invoice.send")]
pub async fn send(id: InvoiceId, jobs: Jobs) -> Result<()> {
    jobs.enqueue("send_invoice", id.0.to_string()).await?;
    Ok(())
}
```

- `jobs.enqueue(name, payload)` एक रन कतार में लगाता है; `enqueue_after` उसे देर से चलाता है। पेलोड अधिकतम 4000 बाइट का टेक्स्ट है, जैसे आईडी या छोटा JSON।
- वर्कर एक मिनट की लीज़ के साथ देय रन को दावे में लेता है और रन के चलते रहने तक उसे नवीनीकृत करता है। यदि सर्वर रुक जाए, तो लीज़ खत्म होने के बाद रन फिर दावे में लिया जाता है; वह प्रयास गिना जाता है।
- असफल प्रयास बैकऑफ़ का इंतज़ार करता है, जो दस मिनट तक दोगुना होता है, जब तक पुनः प्रयास खत्म न हो जाएँ। फिर रन अपने त्रुटि कोड के साथ `failed` के रूप में तालिका में रहता है। शेड्यूल कभी हमेशा के लिए असफल नहीं होता: वह अपने अगले अंतराल पर चला जाता है।
- PostgreSQL `FOR UPDATE SKIP LOCKED` से दावा करता है, Oracle (11g और बाद के) आईडी के select से उसी तरह। तालिका `zeyn_jobs` है; `create_job_tables(dialect, prefix)` माइग्रेशन के लिए स्टेटमेंट देता है।
- मेमोरी वाला स्टोर (`MemoryJobStore`) अधिकतम 10,000 रन रखता है।
