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

`zeyn::testing::TestApp`은 앱을 메모리에서 실행합니다. 서비스 중인 앱과 같은 라우터, 같은 검사, 같은 접근 규칙을 쓰며 포트를 열지 않습니다. Angular의 `TestBed`나 Flutter의 위젯 테스터 역할을 합니다. 앱을 만들고, 사용자로 로그인하고, Screen을 열고, 폼을 제출하고, 그리드 페이지를 불러오거나 Action을 호출한 뒤 상태, HTML 또는 JSON을 확인합니다. CSRF 토큰은 대신 보내 줍니다. `zeyn new`는 이 방식으로 `tests/app.rs`를 작성합니다.

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

| 호출 | 하는 일 |
|---|---|
| `TestApp::new(app)` | 앱 자체의 로그인 방식을 그대로 쓰는 앱. `App::check`가 문제를 찾으면 그 문제와 함께 panic합니다 |
| `TestApp::signed_in(app, identity)` | 모든 요청을 `identity`로 로그인시켜 각 권한이 무엇을 허용하는지 확인합니다 |
| `.header(name, value)` | 모든 요청에 붙는 헤더. 예: `accept-language` |
| `get(path)` | 페이지, 파일 또는 엔드포인트 |
| `submit(screen, params, node, values)` | 브라우저처럼 폼을 제출합니다. 성공하면 `effects()`와 함께 `200`, 규칙 위반이면 필드별 `errors`와 함께 `422`, 권한이 없으면 `403` |
| `grid(screen, params, node, query)` | JSON `DataQuery`로 그리드의 한 페이지를 불러옵니다. HTML `rows`와 `total` |
| `api(feature, action, input)` | `Feature::api`로 공개한 Action을 호출합니다 |
| `send(request)` | 그 밖의 모든 요청 |

모든 호출은 `status`, `headers`, `body`, `json()`, `effects()`를 가진 `TestResponse`를 반환합니다.

Screen은 서버 없이도 동작합니다. 인수 없는 `#[screen]`에는 `run()`이 있고, `screen.html(&META)`로 렌더링하고, `screen.schema(&META)`로 설명을 얻고, `App::check_page(&screen)`로 페이지 검사를 실행합니다. 정확한 HTML보다 사용자가 할 수 있는 일(상태, 효과, 그리드의 행)을 테스트하고, 변경할 때마다 `cargo test`를 실행하십시오.
