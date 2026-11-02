# Project structure

Every Zeyn app has the same layout, so a person or an agent finds things in the same place in every project.

```
config/app.json          defaults; APP_* environment variables override them
locales/<lang>.json      every user facing text; all files have the same keys
src/main.rs              loads configuration and starts the app
src/app.rs               the App: services, features, security settings
src/features/<name>/     one folder per feature
    mod.rs               the Feature: route prefix, guards, screens, APIs, menu
    models.rs            #[entity] and #[model] types
    store.rs             services that read and write data
    actions.rs           #[action] functions
    screens.rs           #[screen] functions
tests/                   tests that drive the app through its router
```

## The app

`src/app.rs` builds the `App`. It registers services, features and the settings that apply to every screen:

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

`App::serve(addr)` checks the app and starts the server. `App::router()` returns the `axum` router, which tests use without opening a port.

## A feature

A feature groups a route prefix, guards, screens, APIs, commands and a menu:

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

## Services

A service is a cheap to clone value that actions and screens receive as arguments. Register it once with `App::service`; the app refuses to start when a screen needs a service nobody registered.

```rust
#[derive(Clone, Default)]
pub struct NoteStore(Arc<RwLock<Vec<Note>>>);

impl Service for NoteStore {}
```

## Configuration

`Config::load` reads sources in order; later sources win. `config.get("key")` returns a value and `config.secret("key")` returns a `Secret` that never prints.

```rust
let config = Config::load(vec![Box::new(JsonFileSource("config/app.json".into())), Box::new(EnvSource("APP"))]).await?;
let addr = config.get("server.addr").unwrap_or("127.0.0.1:3000").to_string();
```

## Tests

Tests build the router and call it in memory:

```rust
#[tokio::test]
async fn notes_open() {
    let router = app(Config::default()).router().unwrap();
    let res = router.oneshot(Request::get("/notes").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```
