# Proje yapısı

Her Zeyn uygulaması aynı düzene sahiptir; böylece bir insan ya da ajan her projede şeyleri aynı yerde bulur.

```
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

## Uygulama

`src/app.rs` `App` değerini oluşturur. Servisleri, Feature'ları ve her ekrana uygulanan ayarları kaydeder:

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

`App::serve(addr)` uygulamayı denetler ve sunucuyu başlatır. `App::router()` testlerin port açmadan kullandığı `axum` router'ını döndürür.

## Bir Feature

Bir Feature; rota öneki, guard'lar, ekranlar, API'ler, komutlar ve bir menüyü bir araya getirir:

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

## Servisler

Servis, action'ların ve ekranların argüman olarak aldığı, kopyalaması ucuz bir değerdir. `App::service` ile bir kez kaydedin; bir ekran kimsenin kaydetmediği bir servise ihtiyaç duyarsa uygulama başlamayı reddeder.

```rust
#[derive(Clone, Default)]
pub struct NoteStore(Arc<RwLock<Vec<Note>>>);

impl Service for NoteStore {}
```

## Yapılandırma

`Config::load` kaynakları sırayla okur; sonraki kaynaklar kazanır. `config.get("key")` bir değer döndürür, `config.secret("key")` ise asla yazdırılmayan bir `Secret` döndürür.

```rust
let config = Config::load(vec![Box::new(JsonFileSource("config/app.json".into())), Box::new(EnvSource("APP"))]).await?;
let addr = config.get("server.addr").unwrap_or("127.0.0.1:3000").to_string();
```

## Testler

Testler router'ı oluşturur ve bellekte çağırır:

```rust
#[tokio::test]
async fn notes_open() {
    let router = app(Config::default()).router().unwrap();
    let res = router.oneshot(Request::get("/notes").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```
