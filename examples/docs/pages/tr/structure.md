# Proje yapısı

Her Zeyn uygulaması aynı düzene sahiptir; böylece bir insan ya da ajan her projede şeyleri aynı yerde bulur.

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

## Üreticiler

`zeyn g feature <name>`, çalışan bir liste screen'iyle `models/`, `services/`, `actions/` ve `screens/` klasörlerini oluşturur ve feature'ı servisiyle birlikte `src/app.rs` içine kaydeder. `zeyn g screen <feature> <name>` bir screen ekler ve onu kaydeder. Bir screen parçası paylaşıldığında ya da bir screen dosyası büyüdüğünde `components/` klasörünü elle ekleyin; bir feature'ın ihtiyaç duymadığı klasörleri `mod` satırlarıyla birlikte silin. Ayrıntılar için [CLI and projects](cli.md) sayfasına bakın.

## Boyut sınırları

Küçük dosyalar ve fonksiyonlar kodun incelenmesini kolaylaştırır. `Cargo.toml` içinde `[package.metadata.zeyn]` altındaki `max_file_lines` (varsayılan 300 boş olmayan satır), `build.rs` ile `cargo build` işlemini durdurur ve sınırı aşan dosyaları listeler. Clippy'nin `too_many_lines` kuralı (60 satır, `clippy.toml` içinde `too-many-lines-threshold`) aynısını fonksiyonlar için yapar. İki sınır da bu dosyalarda yükseltilebilir, ama bir ajan bunun yerine kodu bölmelidir: dosya başına bir screen, component ya da bir grup action, ve bir screen'in büyük parçaları kendi fonksiyonlarında.

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
