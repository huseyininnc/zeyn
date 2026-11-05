# بنية المشروع

لكل تطبيق Zeyn التخطيط نفسه، فيجد الشخص أو الوكيل الأشياء في المكان نفسه في كل مشروع.

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

## التطبيق

يبني `src/app.rs` الكائن `App`. يسجّل الخدمات والميزات والإعدادات التي تنطبق على كل شاشة:

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

يفحص `App::serve(addr)` التطبيق ويبدأ الخادم. ويعيد `App::router()` موجّه `axum`، الذي تستخدمه الاختبارات دون فتح منفذ.

## الميزة

تجمع الميزة بادئة مسار وحراساً وشاشات وواجهات API وأوامر وقائمة:

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

## الخدمات

الخدمة (service) قيمة رخيصة النسخ تستقبلها الإجراءات والشاشات كوسائط. سجّلها مرة واحدة بـ `App::service`؛ ويرفض التطبيق البدء عندما تحتاج شاشة إلى خدمة لم يسجلها أحد.

```rust
#[derive(Clone, Default)]
pub struct NoteStore(Arc<RwLock<Vec<Note>>>);

impl Service for NoteStore {}
```

## الإعدادات

يقرأ `Config::load` المصادر بالترتيب؛ وتفوز المصادر اللاحقة. يعيد `config.get("key")` قيمة، ويعيد `config.secret("key")` كائن `Secret` لا يُطبع أبداً.

```rust
let config = Config::load(vec![Box::new(JsonFileSource("config/app.json".into())), Box::new(EnvSource("APP"))]).await?;
let addr = config.get("server.addr").unwrap_or("127.0.0.1:3000").to_string();
```

## الاختبارات

تبني الاختبارات الموجّه وتستدعيه في الذاكرة:

```rust
#[tokio::test]
async fn notes_open() {
    let router = app(Config::default()).router().unwrap();
    let res = router.oneshot(Request::get("/notes").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```
