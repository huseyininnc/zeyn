# Структура проекта

У каждого приложения Zeyn одинаковая структура, поэтому человек или агент находит нужное в одном и том же месте в любом проекте.

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

## Генераторы

`zeyn g feature <name>` создаёт `models/`, `services/`, `actions/` и `screens/` с рабочим экраном списка и регистрирует фичу и её сервис в `src/app.rs`. `zeyn g screen <feature> <name>` добавляет один экран и регистрирует его. Папку `components/` добавляйте вручную, когда часть экрана используется в нескольких местах или файл экрана разрастается, а ненужные фиче папки удаляйте вместе с их строкой `mod`. Подробности в [CLI and projects](cli.md).

## Ограничения размера

Небольшие файлы и функции упрощают ревью кода. `max_file_lines` в `[package.metadata.zeyn]` в `Cargo.toml` (по умолчанию 300 непустых строк) заставляет `build.rs` остановить `cargo build` и перечислить файлы сверх предела. `too_many_lines` в Clippy (60 строк, `too-many-lines-threshold` в `clippy.toml`) делает то же для функций. Оба предела можно поднять в этих файлах, но агенту следует вместо этого разделить код: один экран, компонент или группа действий на файл, а крупные части экрана в отдельных функциях.

## Приложение

`src/app.rs` создаёт `App`. Он регистрирует сервисы, Feature и настройки, которые действуют на каждый экран:

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

`App::serve(addr)` проверяет приложение и запускает сервер. `App::router()` возвращает роутер `axum`, который тесты используют без открытия порта.

## Feature

Feature объединяет префикс маршрута, guard (защитные проверки), экраны, API, команды и меню:

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

## Сервисы

Сервис это значение, которое дёшево клонируется и которое action и экраны получают как аргументы. Зарегистрируйте его один раз через `App::service`; приложение не запустится, если экрану нужен сервис, который никто не зарегистрировал.

```rust
#[derive(Clone, Default)]
pub struct NoteStore(Arc<RwLock<Vec<Note>>>);

impl Service for NoteStore {}
```

## Конфигурация

`Config::load` читает источники по порядку; более поздние источники имеют приоритет. `config.get("key")` возвращает значение, а `config.secret("key")` возвращает `Secret`, который никогда не выводится.

```rust
let config = Config::load(vec![Box::new(JsonFileSource("config/app.json".into())), Box::new(EnvSource("APP"))]).await?;
let addr = config.get("server.addr").unwrap_or("127.0.0.1:3000").to_string();
```

## Тесты

Тесты создают роутер и вызывают его в памяти:

```rust
#[tokio::test]
async fn notes_open() {
    let router = app(Config::default()).router().unwrap();
    let res = router.oneshot(Request::get("/notes").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```
