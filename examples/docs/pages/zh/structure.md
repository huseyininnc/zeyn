# 项目结构

每个 Zeyn 应用都有相同的布局，因此无论是人还是智能体，在每个项目中都能在同样的位置找到东西。

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

测试构建路由器并在内存中调用它：

```rust
#[tokio::test]
async fn notes_open() {
    let router = app(Config::default()).router().unwrap();
    let res = router.oneshot(Request::get("/notes").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```
