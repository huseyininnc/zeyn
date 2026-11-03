# Estructura del proyecto

Toda aplicación Zeyn tiene la misma disposición, de modo que una persona o un agente encuentra las cosas en el mismo lugar en cada proyecto.

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

## La aplicación

`src/app.rs` construye el `App`. Registra los servicios, las features y los ajustes que se aplican a cada pantalla:

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

`App::serve(addr)` comprueba la aplicación e inicia el servidor. `App::router()` devuelve el router de `axum`, que las pruebas usan sin abrir un puerto.

## Una feature

Una feature agrupa un prefijo de ruta, guards, pantallas, APIs, comandos y un menú:

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

## Servicios

Un servicio es un valor barato de clonar que las acciones y las pantallas reciben como argumentos. Regístralo una vez con `App::service`; la aplicación se niega a arrancar cuando una pantalla necesita un servicio que nadie registró.

```rust
#[derive(Clone, Default)]
pub struct NoteStore(Arc<RwLock<Vec<Note>>>);

impl Service for NoteStore {}
```

## Configuración

`Config::load` lee las fuentes en orden; las posteriores prevalecen. `config.get("key")` devuelve un valor y `config.secret("key")` devuelve un `Secret` que nunca se imprime.

```rust
let config = Config::load(vec![Box::new(JsonFileSource("config/app.json".into())), Box::new(EnvSource("APP"))]).await?;
let addr = config.get("server.addr").unwrap_or("127.0.0.1:3000").to_string();
```

## Pruebas

Las pruebas construyen el router y lo llaman en memoria:

```rust
#[tokio::test]
async fn notes_open() {
    let router = app(Config::default()).router().unwrap();
    let res = router.oneshot(Request::get("/notes").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```
