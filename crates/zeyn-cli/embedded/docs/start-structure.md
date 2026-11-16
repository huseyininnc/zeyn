# Project structure

Every Zeyn app has the same layout, so a person or an agent finds things in the same place in every project.

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

## Generators

`zeyn g feature <name>` creates `models/`, `services/`, `actions/` and `screens/` with a working list screen and registers the feature and its service in `src/app.rs`. `zeyn g screen <feature> <name>` adds one screen and registers it. Add `components/` by hand when a part of a screen is shared or a screen file grows, and delete the folders a feature does not need, with their `mod` line. See [CLI and projects](cli.md) for details.

## Size limits

Small files and functions keep code easy to review. `max_file_lines` under `[package.metadata.zeyn]` in `Cargo.toml` (default 300 non blank lines) makes `build.rs` stop `cargo build` and list the files over the limit. Clippy's `too_many_lines` (60 lines, `too-many-lines-threshold` in `clippy.toml`) does the same for functions. Both limits can be raised in those files, but an agent should split the code instead: one screen, component or group of actions per file, and large parts of a screen in their own functions.

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

`zeyn::testing::TestApp` drives the app in memory: the same router, checks and access rules as a served app, without opening a port. It plays the part of Angular's `TestBed` or Flutter's widget tester: build the app, sign a user in, open a screen, submit a form, page a grid or call an action, then read the status, the HTML or the JSON. It sends the CSRF token for you. `zeyn new` writes `tests/app.rs` this way.

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

| Call | Does |
|---|---|
| `TestApp::new(app)` | The app with its own sign in. Panics with the problems `App::check` finds |
| `TestApp::signed_in(app, identity)` | Every request signed in as `identity`, to try what each permission allows |
| `.header(name, value)` | A header on every request, such as `accept-language` |
| `get(path)` | A page, a file or an endpoint |
| `submit(screen, params, node, values)` | Submits a form as the browser does: `200` with `effects()`, `422` with `errors` by field, `403` without the permission |
| `grid(screen, params, node, query)` | A page of a grid, with a `DataQuery` as JSON: `rows` as HTML and `total` |
| `api(feature, action, input)` | Calls an action published with `Feature::api` |
| `send(request)` | Any other request |

Each call returns a `TestResponse` with `status`, `headers`, `body`, `json()` and `effects()`.

A screen also works without the server: a `#[screen]` without arguments has `run()`, `screen.html(&META)` renders it, `screen.schema(&META)` describes it and `App::check_page(&screen)` runs the page checks. Test what the user can do (a status, an effect, a row in the grid) rather than the exact HTML, and run `cargo test` after every change.
