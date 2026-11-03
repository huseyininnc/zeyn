# 데스크톱 앱

Zeyn 앱은 하나의 데스크톱 실행 파일로 배포할 수 있습니다. `zeyn-desktop` 크레이트는 프로세스 안에서 앱 자체의 서버를 시작하고 시스템 WebView를 통해 네이티브 창에 표시합니다.

| 플랫폼 | WebView | 상태 |
|----------|---------|--------|
| Windows 10 및 11 | WebView2 (Edge) | 빌드하고 직접 확인함 |
| macOS | WKWebView | `wry`를 통해 컴파일됨, 직접 확인하지 않음 |
| Linux | WebKitGTK 4.1 | `libwebkit2gtk-4.1-dev`와 `libgtk-3-dev`가 설치되어 있으면 `wry`를 통해 컴파일됨, 직접 확인하지 않음 |

아무것도 컴퓨터 밖으로 나가지 않습니다. 앱이 스스로 원격 서비스를 호출하지 않는 한 오프라인에서도 동작합니다.

## 사용하기

일반 서버 빌드가 WebView를 컴파일하지 않도록 기능 뒤에 크레이트를 추가합니다.

```toml
[features]
desktop = ["dep:zeyn-desktop"]

[dependencies]
zeyn-desktop = { version = "0.0.2", optional = true }
```

`App::serve` 대신 창을 시작합니다.

```rust
#[cfg(feature = "desktop")]
fn main() -> std::io::Result<()> {
    zeyn_desktop::Desktop::new(app()?)
        .id("my-app")
        .title("My App")
        .size(1280, 800)
        .run()
}
```

- `id`는 사용자별 데이터 폴더의 이름을 정하며 id당 하나의 인스턴스를 유지합니다. 기본값은 실행 파일 이름입니다.
- `title`은 페이지가 `document.title`을 설정할 때까지 표시되며, 그 후에는 창 제목이 페이지를 따릅니다.
- `size`는 논리 픽셀 단위의 첫 크기입니다. 이후 실행은 마지막 크기와 위치를 재사용합니다.

`run`은 창이 닫힐 때까지 블록하고, 서버를 중지하고, 프로세스를 끝냅니다. `ZEYN_DESCRIBE=1`이면 앱 설명을 출력하고 반환하므로 `zeyn check`와 `zeyn routes`가 계속 동작합니다.

Windows에서 릴리스 빌드의 콘솔 창을 숨기려면 `main.rs`의 맨 위에 다음을 넣으십시오.

```rust
#![cfg_attr(all(feature = "desktop", not(debug_assertions)), windows_subsystem = "windows")]
```

`examples/adisyon`은 판매 시점 관리 앱으로 된 완전한 예제입니다.

```
cargo run -p zeyn-example-adisyon --features desktop
```

## 실행 파일 빌드

```
cd my-app
zeyn build-desktop                    # same as --features desktop
zeyn build-desktop --package my-app --features desktop,postgres
```

링크 타임 최적화, 코드 생성 단위 하나, 심볼 제거와 함께 `cargo build --release`를 실행한 다음 모든 실행 파일의 경로와 크기를 출력합니다. 속도는 `opt-level = 3`으로 유지됩니다.

### 아이콘, 버전 정보, 매니페스트 (Windows)

Windows는 이를 exe 안의 리소스에서 읽습니다. 데스크톱 빌드에서만 이를 포함하는 빌드 스크립트를 추가하십시오.

```toml
[features]
desktop = ["dep:zeyn-desktop", "dep:winresource"]

[build-dependencies]
winresource = { version = "0.1", optional = true, default-features = false }
```

`build.rs`:

```rust
fn main() {
    #[cfg(feature = "desktop")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app.ico")
            .set("ProductName", "My App")
            .set("FileDescription", "My App")
            .set_manifest_file("assets/app.exe.manifest");
        res.compile().unwrap();
    }
}
```

아이콘의 리소스 id는 1이어야 하며(`set_icon`의 기본값), 창과 작업 표시줄이 이를 사용합니다. 버전은 `package.version`에서 가져옵니다. 모니터별 DPI 인식, UTF-8, 공용 컨트롤 6이 있는 매니페스트는 `examples/adisyon/build.rs`와 `examples/adisyon/assets`를 참고하십시오. MSVC 툴체인은 Windows SDK에서 `rc.exe`를 찾으며, GNU 툴체인은 `windres`를 사용합니다.

## WebView2 런타임

Windows 11과 최신 Windows 10에는 WebView2 런타임이 미리 설치되어 있고 스스로 업데이트됩니다(evergreen 런타임). exe는 이를 포함하지 않으므로 작게 유지됩니다.

오래되었거나 제한된 컴퓨터를 위해서는 다음과 같이 합니다.

- Microsoft의 evergreen 부트스트래퍼(`MicrosoftEdgeWebview2Setup.exe`, 약 2 MB)를 설치 프로그램과 함께 배포하고 런타임이 없을 때 `/silent /install`로 실행하십시오. 런타임을 한 번 다운로드합니다.
- 인터넷이 없는 컴퓨터에는 evergreen 독립 실행형 설치 프로그램(크고 오프라인용)이나 exe 옆에 둔 고정 버전 런타임을 사용하십시오. 고정 버전은 스스로 업데이트되지 않으므로 보안 업데이트는 직접 책임져야 합니다.

런타임이 없으면 창을 열 수 없고 `run`은 오류를 반환합니다.

## 보안 모델

서버는 앱 자체의 창에서만 접근할 수 있습니다.

- **루프백 전용.** 리스너는 시스템이 고른 포트의 `127.0.0.1`에 바인드됩니다. `0.0.0.0`이나 LAN 주소에는 절대 바인드되지 않습니다.
- **실행 토큰.** 실행할 때마다 무작위 256비트 토큰을 만듭니다. 창은 먼저 `/_desktop/start?t=<token>`을 열며, 이는 토큰을 `HttpOnly`, `SameSite=Strict` 쿠키로 설정하고 `/`로 리다이렉트합니다. 다른 모든 요청은 그 쿠키 또는 `x-zeyn-desktop` 헤더에 토큰을 실어야 하며, 그렇지 않으면 `403`을 받습니다. 정적 에셋도 보호됩니다. 토큰은 메모리에만 있으며 일정한 시간으로 비교됩니다.
- **Host와 Origin.** `Host` 헤더는 정확히 `127.0.0.1:<port>`여야 하며, 이것이 DNS 리바인딩(이름이 127.0.0.1로 해석되는 사이트가 자신의 이름을 호스트로 보냄)을 막습니다. `Origin` 헤더가 있으면 정확히 `http://127.0.0.1:<port>`여야 합니다. 둘 다 시작과 포커스 엔드포인트를 포함한 모든 경로에 적용됩니다.
- **다른 로컬 프로그램**은 포트에 연결할 수 있지만 토큰을 모릅니다. 다른 사용자 계정은 사용자별 데이터 폴더의 인스턴스 파일을 읽을 수 없습니다.
- **탐색.** 창은 자신의 출처, `blob:` URL, `about:blank`만 표시합니다. `http`, `https`, `mailto` 링크는 시스템 기본 핸들러에서 열리며, 그 밖의 모든 스킴(`file:`, 사용자 지정 프로토콜 핸들러, `javascript:`)은 버려집니다. `window.open`은 두 번째 WebView를 절대 만들지 않습니다.
- **개발자 도구**는 디버그 빌드에서 켜지고 릴리스 빌드에서 꺼집니다.
- **다운로드**는 앱의 출처에서만 허용되며 사용자의 다운로드 폴더로 가고, 파일을 덮어쓰지 않습니다(`orders.csv`, `orders (1).csv`, ...). 연속된 여러 다운로드는 확인 없이 허용됩니다. 다른 권한 요청(카메라, 마이크, 클립보드)은 WebView의 일반 확인을 받습니다.

가드가 다루지 않는 것:

- 같은 사용자로 실행되는 프로그램은 프로세스 메모리를 읽거나 디버거를 연결할 수 있습니다. 어떤 로컬 앱도 이를 막을 수 없습니다.
- WebView2는 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 환경 변수를 따르므로, 사용자의 환경 변수를 설정할 수 있는 사람은 원격 디버깅을 켤 수 있습니다. 같은 사용자이므로 위와 같은 한계입니다.
- 앱 자체의 인증은 창 안에서도 여전히 적용됩니다. 데스크톱 빌드의 `DevAuth`는 컴퓨터 앞에 앉은 누구에게나 전체 접근 권한을 줍니다.

## 동작

- **단일 인스턴스.** 첫 번째 실행은 `instance`(포트와 별도의 포커스 키)를 데이터 폴더에 씁니다. 두 번째 실행은 키를 `/_desktop/focus`로 보내고, 첫 번째 창이 앞으로 나오며 두 번째 프로세스는 약 150 ms 안에 종료됩니다. 두 번의 실행이 동시에 시작되면 둘 다 열릴 수 있습니다.
- **창 상태.** 크기, 위치, 최대화 플래그는 닫을 때 `window.json`에 저장됩니다. 더 이상 어떤 모니터에도 없는 저장된 위치는 무시됩니다.
- **종료.** 창을 닫으면 WebView를 해제하고, 서버를 정상적으로 중지하고(최대 2초), 프로세스를 끝냅니다. WebView 프로세스도 함께 끝납니다.
- **데이터 폴더.** Windows에서는 `%LOCALAPPDATA%\<id>`, macOS에서는 `~/Library/Application Support/<id>`, Linux에서는 `~/.local/share/<id>`입니다. WebView 프로필(캐시, 로컬 저장소)은 그 안의 `webview`에 있으므로 exe는 Program Files 같은 읽기 전용 폴더에서도 동작합니다.
- **트레이 아이콘 없음.** 내장되어 있지 않습니다.

## 크기와 메모리

`examples/adisyon`, 릴리스 빌드, WebView2 런타임 156으로 Windows 11에서 측정한 결과입니다.

| | |
|---|---|
| `zeyn build-desktop` exe | 6.72 MB (일반 릴리스 프로필에서는 7.25 MB) |
| 앱 프로세스, 유휴 | 전용 4.3 MB, 작업 집합 25.6 MB |
| 앱 프로세스와 WebView2 프로세스, 유휴 | 프로세스 7개, 전용 약 180 MB, 작업 집합 370 MB |
| `--disable-gpu`와 함께 | 전용 약 114 MB, 작업 집합 330 MB |
| 두 번째 실행이 종료될 때까지 | 약 130 ms |

WebView가 대부분을 차지합니다. GPU 프로세스만 약 88 MB를 차지합니다. 메모리가 적은 컴퓨터의 단순한 Screen에서는 exe를 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu`와 함께 시작하면(예: 바로 가기나 실행 스크립트에서) 소프트웨어 렌더링을 대가로 약 65 MB를 절약할 수 있습니다.
