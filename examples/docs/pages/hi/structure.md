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

टेस्ट राउटर बनाते हैं और उसे मेमोरी में कॉल करते हैं:

```rust
#[tokio::test]
async fn notes_open() {
    let router = app(Config::default()).router().unwrap();
    let res = router.oneshot(Request::get("/notes").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```
