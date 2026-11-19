# प्रोजेक्ट संरचना

हर Zeyn ऐप का लेआउट एक जैसा है, इसलिए इंसान या एजेंट को हर प्रोजेक्ट में चीज़ें उसी जगह मिलती हैं।

```
Cargo.toml               dependencies, lints and [package.metadata.zeyn] (max_file_lines)
build.rs                 stops cargo build when a file in src/ is longer than max_file_lines
config/app.json          defaults; APP_* environment variables override them
locales/<lang>.json      every user facing text; all files have the same keys
src/main.rs              loads configuration and starts the app
src/app.rs               the App: services, features, security settings
src/services/<name>.rs   services shared by several features, registered in src/app.rs
src/features/<name>/     one folder per feature
    mod.rs               module declarations and the Feature: route prefix, guards, screens, APIs, menu
    models/              #[entity] and #[model] types, one file each
    services/            services of the feature that read and write data (stores, clients)
    actions/             #[action] functions, one file each
    screens/             #[screen] functions, one file each
    components/          parts of screens (a toolbar, a panel), added by hand when needed
tests/                   tests that drive the app through its router
```

## जनरेटर

`zeyn g feature <name>` एक चालू सूची Screen के साथ `models/`, `services/`, `actions/` और `screens/` बनाता है और feature व उसकी सर्विस को `src/app.rs` में रजिस्टर करता है। `zeyn g screen <feature> <name>` एक Screen जोड़ता है और उसे रजिस्टर करता है। जब Screen का कोई हिस्सा कई जगह इस्तेमाल हो या कोई Screen फ़ाइल लंबी हो जाए, तब `components/` हाथ से जोड़ें, और जिन फ़ोल्डरों की feature को ज़रूरत नहीं, उन्हें उनकी `mod` पंक्ति के साथ हटा दें। विवरण के लिए [CLI and projects](cli.md) देखें।

## आकार की सीमाएँ

छोटी फ़ाइलें और फ़ंक्शन कोड की समीक्षा आसान बनाते हैं। `Cargo.toml` में `[package.metadata.zeyn]` के नीचे `max_file_lines` (डिफ़ॉल्ट 300 भरी हुई पंक्तियाँ) से आगे जाने पर `build.rs` `cargo build` रोक देता है और सीमा से बड़ी फ़ाइलों की सूची देता है। Clippy का `too_many_lines` (60 पंक्तियाँ, `clippy.toml` में `too-many-lines-threshold`) फ़ंक्शनों के लिए यही करता है। दोनों सीमाएँ इन फ़ाइलों में बढ़ाई जा सकती हैं, पर एजेंट को इसकी जगह कोड बाँटना चाहिए: हर फ़ाइल में एक Screen, कंपोनेंट या action का एक समूह, और Screen के बड़े हिस्से अपने फ़ंक्शनों में।

## ऐप

`src/app.rs` `App` बनाता है। यह सेवाएँ, feature और वे सेटिंग पंजीकृत करता है जो हर Screen पर लागू होती हैं:

```rust
pub fn app(config: Config) -> App {
    App::new(t!("app.name"))
        .config(config)
        .i18n(i18n())
        .service(NoteStore::default())
        .auth(ForwardAuth::new("x-forwarded-user").groups("x-forwarded-groups"))
        .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
        .home(features::home::home)
        .feature(features::home::feature())
        .feature(features::notes::feature())
}
```

`App::serve(addr)` ऐप की जाँच करता है और सर्वर शुरू करता है। `App::router()` `axum` राउटर लौटाता है, जिसे टेस्ट बिना पोर्ट खोले इस्तेमाल करते हैं।

## एक feature

feature रूट प्रीफ़िक्स, गार्ड, Screen, API, कमांड और मेनू को समूहित करती है:

```rust
pub fn feature() -> Feature {
    Feature::new("notes")
        .route_prefix("/notes")
        .guard(SignedIn)
        .screen(note_list)
        .screen(note_editor)
        .api(search_notes)
        .menu(|m| m.group(t!("menu.notes"), Icon::Edit).item(t!("notes.title"), note_list))
}
```

## सेवाएँ

सेवा क्लोन करने में सस्ता मान है जिसे action और Screen आर्ग्युमेंट के रूप में पाती हैं। इसे `App::service` से एक बार पंजीकृत करें; जब किसी Screen को ऐसी सेवा चाहिए जो किसी ने पंजीकृत नहीं की, तो ऐप शुरू होने से मना कर देता है।

```rust
#[derive(Clone, Default)]
pub struct NoteStore(Arc<RwLock<Vec<Note>>>);

impl Service for NoteStore {}
```

## कॉन्फ़िगरेशन

`Config::load` स्रोतों को क्रम से पढ़ता है; बाद के स्रोत जीतते हैं। `config.get("key")` मान लौटाता है और `config.secret("key")` `Secret` लौटाता है जो कभी नहीं छपता।

```rust
let config = Config::load(vec![Box::new(JsonFileSource("config/app.json".into())), Box::new(EnvSource("APP"))]).await?;
let addr = config.get("server.addr").unwrap_or("127.0.0.1:3000").to_string();
```

## टेस्ट

`zeyn::testing::TestApp` ऐप को मेमोरी में चलाता है: सर्व किए गए ऐप जैसा ही राउटर, वही जाँचें और वही एक्सेस नियम, बिना कोई पोर्ट खोले। यह Angular के `TestBed` या Flutter के widget tester की भूमिका निभाता है: ऐप बनाइए, किसी यूज़र से साइन इन कीजिए, Screen खोलिए, फ़ॉर्म भेजिए, grid के पेज देखिए या कोई Action चलाइए, फिर स्टेटस, HTML या JSON पढ़िए। CSRF टोकन यह आपके लिए भेजता है। `zeyn new` इसी तरह `tests/app.rs` लिखता है।

```rust
use zeyn::testing::{TestApp, json};
use zeyn::Identity;

#[tokio::test]
async fn a_writer_saves_a_note() {
    let writer = Identity::user("ayse").with_permissions(["notes.note.read", "notes.note.write"]);
    let app = TestApp::signed_in(app(config).unwrap(), writer);

    assert_eq!(app.get("/notes/").await.status, 200);

    let saved = app.submit("note_editor", json!({ "id": null }), "0", json!({ "title": "Call Ada" })).await;
    assert_eq!(saved.status, 200);
    assert!(saved.effects().contains(&"toast".to_string()));

    let rows = app.grid("note_list", json!(null), "0", json!({})).await;
    assert!(rows.json()["rows"].as_str().unwrap().contains("Call Ada"));
}
```

| कॉल | क्या करता है |
|---|---|
| `TestApp::new(app)` | ऐप अपने साइन इन के साथ। `App::check` को समस्याएँ मिलें तो उन्हीं के साथ panic करता है |
| `TestApp::signed_in(app, identity)` | हर अनुरोध `identity` के रूप में साइन इन, यह देखने के लिए कि हर अनुमति क्या करने देती है |
| `.header(name, value)` | हर अनुरोध पर एक हेडर, जैसे `accept-language` |
| `get(path)` | एक पेज, एक फ़ाइल या एक endpoint |
| `submit(screen, params, node, values)` | फ़ॉर्म को ब्राउज़र की तरह भेजता है: `effects()` के साथ `200`, फ़ील्ड के अनुसार `errors` के साथ `422`, अनुमति न हो तो `403` |
| `grid(screen, params, node, query)` | JSON में `DataQuery` के साथ grid का एक पेज: HTML के रूप में `rows` और `total` |
| `api(feature, action, input)` | `Feature::api` से प्रकाशित Action को चलाता है |
| `send(request)` | कोई भी दूसरा अनुरोध |

हर कॉल एक `TestResponse` लौटाता है जिसमें `status`, `headers`, `body`, `json()` और `effects()` होते हैं।

Screen सर्वर के बिना भी चलती है: बिना आर्ग्युमेंट वाले `#[screen]` के पास `run()` होता है, `screen.html(&META)` उसे रेंडर करता है, `screen.schema(&META)` उसका वर्णन करता है और `App::check_page(&screen)` पेज की जाँचें चलाता है। हूबहू HTML के बजाय यह टेस्ट कीजिए कि यूज़र क्या कर सकता है (एक स्टेटस, एक इफ़ेक्ट, grid की एक पंक्ति), और हर बदलाव के बाद `cargo test` चलाइए।
