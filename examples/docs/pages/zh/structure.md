# 项目结构

每个 Zeyn 应用都有相同的布局，因此无论是人还是智能体，在每个项目中都能在同样的位置找到东西。

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

## 生成器

`zeyn g feature <name>` 创建 `models/`、`services/`、`actions/` 和 `screens/`，带有一个可用的列表 screen，并把 feature 及其 service 注册到 `src/app.rs`。`zeyn g screen <feature> <name>` 添加一个 screen 并注册它。当 screen 的某个部分被共享或 screen 文件变长时，手动添加 `components/`；删除 feature 不需要的文件夹及其 `mod` 行。详情见 [CLI and projects](cli.md)。

## 大小限制

小文件和小函数让代码易于审查。`Cargo.toml` 中 `[package.metadata.zeyn]` 下的 `max_file_lines`（默认 300 个非空行）会让 `build.rs` 中止 `cargo build`，并列出超出限制的文件。Clippy 的 `too_many_lines`（60 行，`clippy.toml` 中的 `too-many-lines-threshold`）对函数做同样的检查。两个限制都可以在这些文件中调高，但 agent 应该拆分代码：每个文件一个 screen、component 或一组 action，screen 的大块部分放进各自的函数。

## 应用

`src/app.rs` 构建 `App`。它注册服务、feature，以及适用于每个界面的设置：

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

`App::serve(addr)` 检查应用并启动服务器。`App::router()` 返回 `axum` 路由器，测试无需打开端口即可使用它。

## Feature

一个 feature 将路由前缀、guard、界面、API、命令和菜单组合在一起：

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

## 服务

服务是一种克隆开销很小的值，action 和界面将其作为参数接收。用 `App::service` 注册一次即可；当某个界面需要一个无人注册的服务时，应用会拒绝启动。

```rust
#[derive(Clone, Default)]
pub struct NoteStore(Arc<RwLock<Vec<Note>>>);

impl Service for NoteStore {}
```

## 配置

`Config::load` 按顺序读取各个来源；后面的来源优先。`config.get("key")` 返回一个值，`config.secret("key")` 返回绝不会被打印的 `Secret`。

```rust
let config = Config::load(vec![Box::new(JsonFileSource("config/app.json".into())), Box::new(EnvSource("APP"))]).await?;
let addr = config.get("server.addr").unwrap_or("127.0.0.1:3000").to_string();
```

## 测试

`zeyn::testing::TestApp` 在内存中运行应用：与正式提供服务的应用使用相同的路由、检查和访问规则，但不打开端口。它相当于 Angular 的 `TestBed` 或 Flutter 的 widget tester：构建应用、以某个用户登录、打开 screen、提交表单、翻页 grid 或调用 action，然后检查状态、HTML 或 JSON。CSRF 令牌由它代为发送。`zeyn new` 生成的 `tests/app.rs` 就是这样写的。

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

| 调用 | 作用 |
|---|---|
| `TestApp::new(app)` | 使用应用自身登录方式的应用。`App::check` 发现问题时，会带着这些问题 panic |
| `TestApp::signed_in(app, identity)` | 每个请求都以 `identity` 登录，用来验证每个权限允许做什么 |
| `.header(name, value)` | 每个请求都带上的请求头，例如 `accept-language` |
| `get(path)` | 页面、文件或端点 |
| `submit(screen, params, node, values)` | 像浏览器一样提交表单：成功返回带 `effects()` 的 `200`，规则错误返回按字段列出 `errors` 的 `422`，没有权限返回 `403` |
| `grid(screen, params, node, query)` | 用 JSON 形式的 `DataQuery` 获取 grid 的一页：HTML 形式的 `rows` 和 `total` |
| `api(feature, action, input)` | 调用通过 `Feature::api` 发布的 action |
| `send(request)` | 其他任何请求 |

每个调用都返回一个 `TestResponse`，包含 `status`、`headers`、`body`、`json()` 和 `effects()`。

screen 不需要服务器也能运行：无参数的 `#[screen]` 有 `run()`，`screen.html(&META)` 渲染它，`screen.schema(&META)` 描述它，`App::check_page(&screen)` 运行页面检查。测试用户能做到的事（状态、效果、grid 中的一行），而不是精确的 HTML，并在每次修改后运行 `cargo test`。
