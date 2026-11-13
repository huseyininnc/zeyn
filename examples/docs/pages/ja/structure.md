# プロジェクト構造

すべての Zeyn アプリは同じ構成を持つため、人もエージェントも、どのプロジェクトでも同じ場所にものを見つけられます。

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

## ジェネレーター

`zeyn g feature <name>` は、動作する一覧画面とともに `models/`、`services/`、`actions/`、`screens/` を作り、feature とそのサービスを `src/app.rs` に登録します。`zeyn g screen <feature> <name>` は画面を 1 つ追加して登録します。画面の一部を共有するときや画面ファイルが長くなったときは `components/` を手で追加し、feature に不要なフォルダーは `mod` 行とともに削除します。詳しくは [CLI and projects](cli.md) を参照してください。

## サイズの上限

小さなファイルと関数はコードをレビューしやすくします。`Cargo.toml` の `[package.metadata.zeyn]` にある `max_file_lines`（既定は空でない行が 300 行）を超えると、`build.rs` が `cargo build` を止め、上限を超えたファイルを一覧にします。Clippy の `too_many_lines`（60 行、`clippy.toml` の `too-many-lines-threshold`）は関数について同じことをします。どちらの上限もそれらのファイルで引き上げられますが、エージェントは代わりにコードを分割すべきです。1 ファイルに画面、コンポーネント、またはアクションのまとまりを 1 つ置き、画面の大きな部分はそれぞれの関数に分けます。

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

`zeyn::testing::TestApp` はアプリをメモリ上で動かします。提供中のアプリと同じルーター、チェック、アクセスルールを使い、ポートは開きません。Angular の `TestBed` や Flutter のウィジェットテスターに当たるもので、アプリを構築し、ユーザーでサインインし、画面を開き、フォームを送信し、グリッドのページを読み込み、アクションを呼び出してから、ステータス、HTML、JSON を確認します。CSRF トークンは自動で送ります。`zeyn new` はこの形で `tests/app.rs` を書きます。

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

| 呼び出し | 内容 |
|---|---|
| `TestApp::new(app)` | アプリ自身のサインイン方法のままのアプリ。`App::check` が問題を見つけると、その問題とともに panic します |
| `TestApp::signed_in(app, identity)` | すべてのリクエストを `identity` としてサインインさせ、各権限で何ができるかを試します |
| `.header(name, value)` | すべてのリクエストに付けるヘッダー。たとえば `accept-language` |
| `get(path)` | ページ、ファイル、エンドポイント |
| `submit(screen, params, node, values)` | ブラウザーと同じようにフォームを送信します。成功すると `effects()` 付きの `200`、ルール違反ではフィールドごとの `errors` 付きの `422`、権限がなければ `403` |
| `grid(screen, params, node, query)` | JSON の `DataQuery` でグリッドの 1 ページを取得します。HTML の `rows` と `total` |
| `api(feature, action, input)` | `Feature::api` で公開したアクションを呼び出します |
| `send(request)` | その他のリクエスト |

どの呼び出しも、`status`、`headers`、`body`、`json()`、`effects()` を持つ `TestResponse` を返します。

画面はサーバーなしでも動きます。引数のない `#[screen]` には `run()` があり、`screen.html(&META)` で描画し、`screen.schema(&META)` で記述を得て、`App::check_page(&screen)` でページのチェックを実行します。HTML そのものではなく、ユーザーにできること（ステータス、エフェクト、グリッドの行）をテストし、変更のたびに `cargo test` を実行してください。
