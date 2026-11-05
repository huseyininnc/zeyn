# 프로젝트 구조

모든 Zeyn 앱은 같은 레이아웃을 가지므로 사람이나 에이전트가 모든 프로젝트에서 같은 위치에서 원하는 것을 찾습니다.

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

## 생성기

`zeyn g feature <name>`는 동작하는 목록 Screen과 함께 `models/`, `services/`, `actions/`, `screens/`를 만들고 feature와 서비스를 `src/app.rs`에 등록합니다. `zeyn g screen <feature> <name>`은 Screen 하나를 추가하고 등록합니다. Screen의 일부를 여러 곳에서 쓰거나 Screen 파일이 길어지면 `components/`를 직접 추가하고, feature에 필요 없는 폴더는 `mod` 줄과 함께 삭제합니다. 자세한 내용은 [CLI and projects](cli.md)를 참고하세요.

## 크기 제한

작은 파일과 함수는 코드를 검토하기 쉽게 합니다. `Cargo.toml`의 `[package.metadata.zeyn]`에 있는 `max_file_lines`(기본값은 비어 있지 않은 줄 300개)를 넘으면 `build.rs`가 `cargo build`를 멈추고 한도를 넘은 파일을 나열합니다. Clippy의 `too_many_lines`(60줄, `clippy.toml`의 `too-many-lines-threshold`)는 함수에 같은 일을 합니다. 두 한도 모두 이 파일들에서 올릴 수 있지만, 에이전트는 대신 코드를 나눠야 합니다. 파일마다 Screen, 컴포넌트 또는 action 묶음 하나를 두고, Screen의 큰 부분은 각자의 함수로 뺍니다.

## 앱

`src/app.rs`는 `App`을 만듭니다. 서비스, Feature, 모든 Screen에 적용되는 설정을 등록합니다.

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

`App::serve(addr)`는 앱을 검사하고 서버를 시작합니다. `App::router()`는 `axum` 라우터를 반환하며, 테스트에서 포트를 열지 않고 사용합니다.

## Feature

Feature는 경로 접두사, 가드, Screen, API, 명령, 메뉴를 묶습니다.

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

## 서비스

서비스는 복제 비용이 낮은 값이며 action과 Screen이 인수로 받습니다. `App::service`로 한 번 등록합니다. Screen이 아무도 등록하지 않은 서비스를 필요로 하면 앱은 시작을 거부합니다.

```rust
#[derive(Clone, Default)]
pub struct NoteStore(Arc<RwLock<Vec<Note>>>);

impl Service for NoteStore {}
```

## 설정

`Config::load`는 소스를 순서대로 읽으며 나중의 소스가 우선합니다. `config.get("key")`는 값을 반환하고 `config.secret("key")`는 출력되지 않는 `Secret`을 반환합니다.

```rust
let config = Config::load(vec![Box::new(JsonFileSource("config/app.json".into())), Box::new(EnvSource("APP"))]).await?;
let addr = config.get("server.addr").unwrap_or("127.0.0.1:3000").to_string();
```

## 테스트

테스트는 라우터를 만들고 메모리에서 호출합니다.

```rust
#[tokio::test]
async fn notes_open() {
    let router = app(Config::default()).router().unwrap();
    let res = router.oneshot(Request::get("/notes").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}
```
