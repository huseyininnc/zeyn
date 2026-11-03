# डेटाबेस

`zeyn::sql` entity को SQL डेटाबेस में रखता है। यह उन्हें उस `DataQuery` से सूचीबद्ध करता है जो ग्रिड भेजता है (फ़िल्टर, क्रम, पृष्ठ, योग, समूह) और पंक्ति संस्करण जाँच के साथ सहेजता है। PostgreSQL `postgres` Cargo feature के पीछे है और Oracle `oracle` के पीछे, इसलिए बिना डेटाबेस वाला ऐप इसका कुछ कंपाइल नहीं करता।

```toml
zeyn = { version = "0.0.5", features = ["postgres"] }   # or ["oracle"], or both
```

## सेटअप

एक `Db` को सेवा के रूप में पंजीकृत करें और उसे action में लें:

```rust
let db = Db::from_config(&config).await?;
App::new("Stock").config(config).service(db)

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, db: Db) -> Result<Page<Product>> {
    db.find(&q).await
}

#[action(permission = "stock.product.write")]
pub async fn save_product(p: Valid<Product>, db: Db) -> Result<Product> {
    if p.id.0 == 0 { db.insert(&*p).await } else { db.update(&*p).await }
}
```

`Db::from_config` `database.url` को सीक्रेट के रूप में और पूल सीमाएँ `database.*` से पढ़ता है:

| कुंजी | डिफ़ॉल्ट | अर्थ |
|---|---|---|
| `database.url` | आवश्यक | `postgres://user:password@host:5432/db` या `oracle://user:password@host:1521/service` |
| `database.max_connections` | 10 | पूल का आकार; अनुरोध खाली कनेक्शन का इंतज़ार करते हैं |
| `database.min_connections` | 0 | निष्क्रिय होने पर खुले रखे कनेक्शन |
| `database.acquire_timeout_ms` | 5000 | कनेक्शन के लिए सबसे लंबा इंतज़ार |
| `database.idle_timeout_s` | 600 | न्यूनतम से ऊपर के निष्क्रिय कनेक्शन इसके बाद बंद होते हैं |
| `database.statement_timeout_ms` | 30000 | स्टेटमेंट सबसे लंबा कितना चल सकता है; 0 सर्वर का डिफ़ॉल्ट रखता है |
| `database.count_cap` | 1000000 | पृष्ठ के लिए गिना जाने वाला सबसे बड़ा कुल |

`Db::connect(url, PoolLimits { .. })` बिना `Config` के यही करता है। URL स्कीम ड्राइवर चुनती है। Oracle पासवर्ड में `@` या `:` जैसे वर्ण percent एन्कोड करके लिखे जाते हैं (`%40`)।

PostgreSQL कनेक्शन lazily खुलते हैं, इसलिए डेटाबेस बंद होने पर भी ऐप शुरू होता है और पहली क्वेरी समस्या बताती है। Oracle कनेक्ट होते समय सर्वर से उसका संस्करण पूछता है, जब तक URL न बताए कि कौन सा SQL लिखना है (`?compat=11` या `?compat=12`); `compat` सेट होने पर यह डेटाबेस के बिना भी शुरू होता है।

`inventory` उदाहरण अपने उत्पाद मेमोरी में रखता है, या उस डेटाबेस में जिसे `ZEYN_DATABASE_URL` नाम दे (इसे `--features postgres` या `--features oracle` से बनाएँ):

```
docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55432:5432 postgres:17-alpine
ZEYN_DATABASE_URL=postgres://postgres:zeyn@127.0.0.1:55432/postgres cargo run -p zeyn-example-inventory --features postgres
```

## Oracle Instant Client

Oracle ड्राइवर ODPI-C पर `oracle` crate है। यह Oracle सॉफ़्टवेयर के बिना कंपाइल होता है, लेकिन रन टाइम पर Oracle Client लाइब्रेरी (`oci.dll`, `libclntsh.so`) लोड करता है। Oracle Instant Client इस्तेमाल करने के लिए मुफ़्त है:

1. Oracle के Instant Client पृष्ठ से अपने प्लेटफ़ॉर्म के लिए Instant Client Basic Light डाउनलोड करें। Windows x64 पर ज़िप `instantclient-basiclite-windows.x64-19.*.zip` लें। संस्करण 19 Oracle Database 11.2 से 23ai तक कनेक्ट होता है; नए क्लाइंट अब 11g से कनेक्ट नहीं होते।
2. इसे अपने अलग फ़ोल्डर में खोलें, उदाहरण के लिए `C:\oracle\instantclient_19`।
3. उस फ़ोल्डर को `PATH` में सबसे पहले रखें, या `ZEYN_ORACLE_CLIENT_DIR` को उस पर सेट करें। वेरिएबल `PATH` पर जीतता है, जो तब मदद करता है जब कोई अन्य Oracle होम इंस्टॉल हो। Linux पर RPM इंस्टॉल करें या `LD_LIBRARY_PATH` सेट करें; Basic Light को `libaio` भी चाहिए।

लाइब्रेरी के बिना पहला कनेक्शन `DPI-1047: Cannot locate a 64-bit Oracle Client library` के साथ विफल होता है।

## बोलियाँ

SQL बिल्डर साझा है। `Dialect` केवल वह देता है जो अलग है, और `Postgres` तथा `Oracle { compat }` उसे लागू करते हैं:

| | PostgreSQL | Oracle 12c और बाद के (`V12`) | Oracle 11g (`V11`) |
|---|---|---|---|
| पैरामीटर | `$1` | `:1` | `:1` |
| आइडेंटिफ़ायर | `"code"`, 63 अक्षर तक | `"CODE"`, 30 अक्षर तक | `"CODE"`, 30 अक्षर तक |
| पृष्ठ | `LIMIT $n OFFSET $m` | `OFFSET :m ROWS FETCH NEXT :n ROWS ONLY` | क्रमबद्ध क्वेरी पर `ROWNUM` विंडो |
| नई कुंजियाँ | identity, `RETURNING` | identity, `RETURNING id INTO :n` | `<table>_seq.NEXTVAL` |
| लिखने के बाद वापस पढ़ना | पंक्ति `RETURNING` | कुंजी से पढ़ना | कुंजी से पढ़ना |
| हाँ/नहीं | `BOOLEAN` | `NUMBER(1)` | `NUMBER(1)` |
| खाली टेक्स्ट | एक मान | NULL | NULL |

`compat` EF Core के `UseOracleSQLCompatibility("11")` के विचार का पालन करता है: जब URL न बताए तो सर्वर संस्करण इसे चुनता है। आइडेंटिफ़ायर हर Oracle संस्करण पर 30 अक्षरों के भीतर रहते हैं क्योंकि 12.1 की वही सीमा है जो 11g की; उससे लंबा entity या फ़ील्ड नाम त्रुटि है जो उसे नाम देती है।

Oracle खाली स्ट्रिंग को NULL के रूप में सहेजता है। Zeyn आवश्यक `String` फ़ील्ड में NULL को `""` के रूप में पढ़ता है, टेक्स्ट कॉलम nullable लिखता है, और टेक्स्ट को nullable कॉलम की तरह क्रमबद्ध और पृष्ठबद्ध करता है, इसलिए कर्सर सटीक रहते हैं। `Some("")` रखने वाला `Option<String>` वापस `None` पढ़ा जाता है।

Oracle का ड्राइवर blocking है। हर कॉल Tokio के blocking थ्रेड पर सीमित पूल से लिए कनेक्शन के साथ चलती है, इसलिए यह async रनटाइम को कभी नहीं रोकती; एक समय में अधिकतम `max_connections` कॉल चलती हैं और बाकी `acquire_timeout_ms` तक इंतज़ार करती हैं।

## तालिकाएँ

कॉलमों के नाम फ़ील्ड के नाम पर हैं। प्रकार:

| फ़ील्ड | PostgreSQL | Oracle |
|---|---|---|
| `#[key]` entity आईडी | `BIGINT GENERATED BY DEFAULT AS IDENTITY` | `NUMBER(19)` identity (12c+) या सीक्वेंस के साथ (11g) |
| `String` | `VARCHAR(max_len)` या `TEXT` | `VARCHAR2(max_len CHAR)` या `VARCHAR2(4000)`, nullable |
| `i32`, `i64` | `BIGINT` (`INTEGER` कॉलम भी काम करता है) | `NUMBER(19)` |
| `bool` | `BOOLEAN` | `NUMBER(1)` |
| `Money` | सबसे छोटी इकाई में `BIGINT` राशि, साथ में `<field>_currency VARCHAR(3)` | `NUMBER(19)`, साथ में `<field>_currency` |
| `Decimal` | फ़ील्ड के नियमों से `NUMERIC(precision, scale)`, उनके बिना `NUMERIC(18, 2)` | `NUMBER(precision, scale)` |
| `Date` | `DATE` | `DATE` |
| `Time` | `TIME` | `HH:MM:SS` रखने वाला `VARCHAR2(8)` |
| `DateTime` | `TIMESTAMPTZ(3)` | UTC रखने वाला `TIMESTAMP(3)` |
| `LocalDateTime` | `TIMESTAMP` | `TIMESTAMP(0)` |
| `#[choice]` enum | वेरिएंट का नाम रखने वाला `VARCHAR` | `VARCHAR2` |
| कस्टम फ़ील्ड टाइप | JSON रखने वाला `TEXT` | JSON रखने वाला `CLOB` |
| `Option<T>` | वही कॉलम, nullable | वही कॉलम, nullable |

करेंसी `TRY`, `USD` और `EUR` के रूप में सहेजी जाती हैं। तारीख़ें और समय ISO टेक्स्ट के रूप में चलते हैं और SQL उन्हें निश्चित प्रारूपों से बदलता है (`TO_DATE(:1, 'YYYY-MM-DD')`, `to_char(...)`), इसलिए `NLS_DATE_FORMAT` या PostgreSQL समय क्षेत्र जैसी सत्र सेटिंग परिणाम नहीं बदलतीं। Decimal दशमलव विभाजक के बिना सटीक टेक्स्ट के रूप में बाँधे जाते हैं (`12345E-2`, `CAST(... AS NUMERIC)` और `TO_NUMBER(...)` द्वारा पढ़े गए) और `CAST(... AS TEXT)` या `TO_CHAR(..., 'TM9', 'NLS_NUMERIC_CHARACTERS=''.,''')` से वापस पढ़े जाते हैं, इसलिए `NLS_NUMERIC_CHARACTERS` भी उन्हें नहीं बदलता।

`create_table::<E>(dialect)` (या `db.create_table::<E>()`) स्कैफ़ोल्डिंग और टेस्ट के लिए `CREATE TABLE` लिखता है, हर `#[rule(unique)]` फ़ील्ड के लिए `<table>_<field>_key` नाम की constraint के साथ और, 11g पर, कुंजी सीक्वेंस के साथ। प्रोडक्शन स्कीमा माइग्रेशन में होने चाहिए। PostgreSQL पर:

```rust
db.migrate(&sqlx::migrate!("./migrations")).await?;
db.migrate_dir("./migrations").await?;
```

पहला माइग्रेशन को कंपाइल समय पर एम्बेड करता है, दूसरा फ़ोल्डर रन टाइम पर पढ़ता है। `sqlx::migrate!` को ऐप की अपनी `Cargo.toml` में `macros` और `migrate` feature वाला `sqlx` चाहिए; `migrate_dir` को और कुछ नहीं चाहिए। Oracle स्कीमा टीम के अपने टूल से माइग्रेट होते हैं; `create_table` शुरुआती स्क्रिप्ट देता है।

## पढ़ना

`db.find::<E>(&query)` एक कनेक्शन पर अधिकतम तीन स्टेटमेंट चलाता है: पृष्ठ, गिनती और योग। फ़िल्टर उसी कोड से पढ़े जाते हैं जिससे `DataQuery::apply`, इसलिए सूची मेमोरी के डेटा और SQL में एक जैसा व्यवहार करती है:

| फ़िल्टर | SQL |
|---|---|
| `contains`, `starts_with` | `LOWER(col) LIKE $1 ESCAPE '\'`, टेक्स्ट में `%` और `_` escape किए गए |
| `equals` | `LOWER(col) = $1` |
| संख्याओं और मुद्रा पर `from`, `to` | `col >= $1`, `col <= $2` |
| तारीख़ों और समय पर `from`, `to` | `col >= CAST($1 AS date)`; क्षण पर तारीख़ सीमा उपयोगकर्ता के समय क्षेत्र में पूरा दिन कवर करती है |
| `any_of` | `col IN ($1, $2, ...)` |
| `is` | `col = $1` |
| `blank` | `col IS NULL`, `col IS NOT NULL` |

मुद्रा और हाँ/नहीं कॉलम पर `contains` SQL में अनदेखा किया जाता है। टेक्स्ट डेटाबेस collation से क्रमबद्ध होता है (Oracle: `NLS_SORT`)।

हर क्रम कुंजी पर खत्म होता है, इसलिए क्रम पूर्ण है। NULL ऊपर जाते समय पहले आते हैं और नीचे जाते समय आखिर में, जैसे मेमोरी में; `NULLS` क्लॉज़ केवल nullable कॉलम के लिए लिखा जाता है, इसलिए सादे इंडेक्स अन्य क्रमों की सेवा करते हैं।

`get(key)` एक पंक्ति पढ़ता है और जब कोई न हो तो `Error::NotFound` से विफल होता है। कोई भी तालिका या व्यू entity से पढ़ा जा सकता है, उदाहरण के लिए Oracle का `user_sequences`।

## कर्सर

क्रमांकित पृष्ठ ऑफ़सेट इस्तेमाल करते हैं। अनंत और वर्चुअल स्क्रॉल keyset कर्सर इस्तेमाल करते हैं, जो सूची में गहराई पर धीमे नहीं होते और जब दूसरे सम्मिलित हों तो पंक्तियाँ नहीं छोड़ते या दोहराते:

```sql
WHERE "code" <= $1 AND (("code" < $2) OR ("code" = $3 AND "id" > $4))
ORDER BY "code" DESC, "id" ASC LIMIT $5
```

पृष्ठ माँगे से एक पंक्ति अधिक लाता है ताकि जाना जा सके कि और आगे हैं या नहीं। `Page::next` तब एक कर्सर रखता है: base64url JSON जिसमें क्रम फ़ील्ड, उनकी दिशाएँ और हर क्रम कुंजी का आखिरी पंक्ति का मान (कुंजी सबसे आखिर में) होता है। कर्सर क्लाइंट के लिए अपारदर्शी है लेकिन गुप्त नहीं, और सर्वर उस पर कभी भरोसा नहीं करता। इसे आकार सीमा के साथ डिकोड किया जाता है, उसका क्रम क्वेरी के क्रम से अलग हो तो अस्वीकार किया जाता है, और हर मान बाँधे जाने से पहले उसके कॉलम प्रकार से जाँचा जाता है। खराब कर्सर सत्यापन त्रुटि है, क्वेरी नहीं। यह केवल बताता है कि कहाँ से जारी रखना है; फ़िल्टर और अनुमतियाँ फिर लागू होती हैं।

समूहित पृष्ठ अपने कर्सर के लिए ऑफ़सेट इस्तेमाल करते हैं।

## योग और गिनती

`with_total` अधिकतम `database.count_cap` पंक्तियाँ गिनता है (सीमित क्वेरी पर `COUNT(*)`), इसलिए विशाल तालिका पर कुल सस्ता रहता है; जब गिनती सीमा तक पहुँचती है, तो `Page::total` सीमा है।

कॉलम योग हर उस पंक्ति पर एक एग्रीगेट स्टेटमेंट हैं जो फ़िल्टर से मेल खाती है। वे मेमोरी के नियमों का पालन करते हैं: खाली योग 0 है, खाली औसत खाली है, मुद्रा औसत शून्य से दूर की ओर आधे पर पूर्णांकित होते हैं, और एक से अधिक करेंसी वाली मुद्रा का कोई योग नहीं।

`DataQuery::group` गिनती और माँगे गए प्रति समूह योग के साथ `GROUP BY` में बदलता है।

## लिखना

- `insert(&row)` नई कुंजी (0) डेटाबेस पर छोड़ता है, पंक्ति संस्करण 1 सेट करता है और सहेजी गई पंक्ति लौटाता है।
- `update(&row)` `UPDATE ... SET version = version + 1 WHERE id = $1 AND version = $2` चलाता है। शून्य पंक्तियों का अर्थ है किसी और ने पहले सहेजा (या पंक्ति हटाई): `Error::Conflict`।
- `delete::<E>(key)` जब कुछ नहीं हटा तब `Error::NotFound` से विफल होता है।
- `#[readonly]` फ़ील्ड मॉडल से कभी लिखे नहीं जाते, इसलिए गढ़ा हुआ फ़ॉर्म उन्हें नहीं बदल सकता। सर्वर कोड उन्हें अपने SQL से बदलता है: `db.execute("UPDATE products SET stock = stock - $1 WHERE id = $2", args)`। `execute` स्ट्रिंग लिटरल लेता है, इसलिए मान केवल पैरामीटर के रूप में आ सकते हैं।

यूनिक उल्लंघन (PostgreSQL `23505`, Oracle `ORA-00001`) फ़ील्ड पर सत्यापन त्रुटि बन जाता है: वह `#[rule(unique)]` फ़ील्ड जिसका नाम constraint के नाम में है, या अकेला यूनिक फ़ील्ड। फ़ॉर्म इसे इनपुट के नीचे किसी भी अन्य नियम की तरह दिखाता है। constraint का नाम `<table>_<column>_key` रखें।

## ट्रांज़ैक्शन

```rust
let mut tx = db.begin().await?;
let order = tx.insert(&order).await?;
tx.execute("UPDATE products SET stock = stock - $1 WHERE id = $2", vec![qty.into(), id.0.into()]).await?;
tx.commit().await?;
```

`Tx` में एक कनेक्शन पर `Db` जैसी ही कॉल हैं। इसे `commit` के बिना छोड़ना रोलबैक करता है। `Db` पर ही कॉल एक-एक करके कमिट होती हैं।

## स्ट्रीम

`db.stream::<E>(&query)` निर्यात के लिए हर मेल खाती पंक्ति को बिना पृष्ठांकन क्रम से लौटाता है। एक टास्क पंक्तियों को `STREAM_BUFFER` (64) पंक्तियों के चैनल में पढ़ता है और जब पाठक पीछे हो तो इंतज़ार करता है, इसलिए किसी भी परिणाम आकार के लिए मेमोरी सपाट रहती है। स्ट्रीम छोड़ना क्वेरी रोकता है और कनेक्शन मुक्त करता है। `RowStream` `futures_core::Stream` लागू करता है।

## सुरक्षा: SQL इंजेक्शन

- आइडेंटिफ़ायर (तालिका और कॉलम नाम) केवल entity से आते हैं: `E::TABLE` और उसकी `FieldMeta` सूची। क्लाइंट जो नाम फ़िल्टर और क्रम में भेजता है वे उस सूची में खोजे जाते हैं और सूची में न होने पर हटा दिए जाते हैं। हर आइडेंटिफ़ायर जाँचा जाता है (अक्षर, अंक, `_`, अधिकतम बोली की लंबाई) और उद्धृत किया जाता है।
- मान हमेशा बाँधे गए पैरामीटर हैं, `LIKE` पैटर्न, कर्सर मान, सीमाएँ और ऑफ़सेट समेत। कोई मान कभी SQL टेक्स्ट में नहीं लिखा जाता।
- हाथ से लिखा SQL `execute(&'static str, args)` से जाता है: स्टेटमेंट लिटरल होना चाहिए।
- सीमाएँ सर्वर पर सीमित हैं (`MAX_PAGE_SIZE`, `MAX_FILTERS`, `MAX_SORT`, फ़िल्टर टेक्स्ट लंबाई, `any_of` आकार), क्लाइंट कुछ भी माँगे।
- ड्राइवर त्रुटियाँ `Error::Internal` के रूप में लॉग होती हैं और उपयोगकर्ताओं को कभी नहीं दिखाई जातीं। कनेक्शन URL `Secret` के रूप में पढ़ा जाता है और कभी लॉग नहीं होता।

## प्रदर्शन

- स्टेटमेंट प्रति कनेक्शन तैयार और कैश किए जाते हैं (sqlx का स्टेटमेंट कैश, Oracle का 64 का स्टेटमेंट कैश)।
- पंक्तियाँ कुछ सादे सेल टाइप में पढ़ी जाती हैं और serde डीसीरियलाइज़र से मॉडल में ले जाई जाती हैं: कोई मध्यवर्ती मैप नहीं और टेक्स्ट की कोई प्रति नहीं।
- पूल सीमित हैं और हर `find` अपने दो या तीन स्टेटमेंट के लिए एक कनेक्शन इस्तेमाल करता है।
- Oracle प्रति राउंड ट्रिप 128 पंक्तियाँ लाता है। पृष्ठ (अधिकतम 501 पंक्तियाँ) blocking थ्रेड पर पढ़ा जाता है और तुरंत सौंपा जाता है; स्ट्रीम पंक्ति दर पंक्ति जाती हैं।
- स्ट्रीम अधिकतम `STREAM_BUFFER` पंक्तियाँ रखती हैं।

## टेस्ट

SQL बिल्डर के दोनों बोलियों के यूनिट टेस्ट हैं जिन्हें डेटाबेस नहीं चाहिए। इंटीग्रेशन टेस्ट `Db::find` की तुलना उन्हीं पंक्तियों पर `DataQuery::apply` से करते हैं, हर keyset कर्सर पर चलते हैं, और सेव, टकराव, यूनिक त्रुटियाँ, ट्रांज़ैक्शन और स्ट्रीम जाँचते हैं। वे तब चलते हैं जब उनके वेरिएबल में एक या अधिक URL हों (खाली जगह से अलग) और वरना बताते हैं कि छोड़े गए:

```
docker run -d --name zeyn-pg -e POSTGRES_PASSWORD=zeyn -p 55432:5432 postgres:17-alpine
ZEYN_TEST_PG=postgres://postgres:zeyn@127.0.0.1:55432/postgres cargo test -p zeyn --features postgres --test postgres

docker run -d --name zeyn-ora11 -p 15211:1521 -e ORACLE_PASSWORD=zeyn -e APP_USER=zeyn -e APP_USER_PASSWORD=zeyn gvenzl/oracle-xe:11-slim
docker run -d --name zeyn-ora21 -p 15221:1521 -e ORACLE_PASSWORD=zeyn -e APP_USER=zeyn -e APP_USER_PASSWORD=zeyn gvenzl/oracle-xe:21-slim
ZEYN_TEST_ORACLE="oracle://zeyn:zeyn@127.0.0.1:15211/XE oracle://zeyn:zeyn@127.0.0.1:15221/XEPDB1" cargo test -p zeyn --features oracle --test oracle
```

Oracle कंटेनर शुरू होने में कुछ मिनट लेते हैं; `docker logs` में `DATABASE IS READY TO USE!` का इंतज़ार करें।
