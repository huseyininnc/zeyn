# プロジェクト構造

すべての Zeyn アプリは同じ構成を持つため、人もエージェントも、どのプロジェクトでも同じ場所にものを見つけられます。

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

## アプリ

`src/app.rs` が `App` を構築します。サービス、Feature、そしてすべての画面に適用される設定を登録します。

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

`App::serve(addr)` はアプリを検査してサーバーを起動します。`App::router()` は `axum` のルーターを返し、テストはポートを開かずにこれを使います。

## Feature

Feature は、ルートのプレフィックス、ガード、画面、API、コマンド、メニューをまとめます。

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

## サービス

サービスは、アクションや画面が引数として受け取る、複製の軽い値です。`App::service` で一度だけ登録します。画面が必要としているサービスを誰も登録していない場合、アプリは起動を拒否します。

```rust
#[derive(Clone, Default)]
pub struct NoteStore(Arc<RwLock<Vec<Note>>>);

impl Service for NoteStore {}
```

## 設定

`Config::load` はソースを順に読み込み、後のソースが優先されます。`config.get("key")` は値を返し、`config.secret("key")` は決して出力されない `Secret` を返します。

```rust
let config = Config::load(vec![Box::new(JsonFileSource("config/app.json".into())), Box::new(EnvSource("APP"))]).await?;
let addr = config.get("server.addr").unwrap_or("127.0.0.1:3000").to_string();
```

## テスト

テストはルーターを構築し、メモリ上で呼び出します。

```rust
#[tokio::test]
async fn notes_open() {
    let router = app(Config::default()).router().unwrap();
    let res = router.oneshot(Request::get("/notes").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```
