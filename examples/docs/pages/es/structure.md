# Estructura del proyecto

Toda aplicación Zeyn tiene la misma disposición, de modo que una persona o un agente encuentra las cosas en el mismo lugar en cada proyecto.

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

## Generadores

`zeyn g feature <name>` crea `models/`, `services/`, `actions/` y `screens/` con una pantalla de lista que funciona y registra la feature y su servicio en `src/app.rs`. `zeyn g screen <feature> <name>` añade una pantalla y la registra. Añade `components/` a mano cuando una parte de una pantalla se comparte o un archivo de pantalla crece, y borra las carpetas que una feature no necesita, con su línea `mod`. Consulta [CLI and projects](cli.md) para más detalles.

## Límites de tamaño

Los archivos y funciones pequeños hacen que el código sea fácil de revisar. `max_file_lines` en `[package.metadata.zeyn]` de `Cargo.toml` (por defecto 300 líneas no vacías) hace que `build.rs` detenga `cargo build` y liste los archivos que superan el límite. `too_many_lines` de Clippy (60 líneas, `too-many-lines-threshold` en `clippy.toml`) hace lo mismo con las funciones. Ambos límites se pueden subir en esos archivos, pero un agente debe dividir el código: una pantalla, un componente o un grupo de acciones por archivo, y las partes grandes de una pantalla en sus propias funciones.

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
