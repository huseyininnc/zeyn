# CLI와 프로젝트

`zeyn` 명령은 프로젝트를 만들고, Feature를 추가하고, 앱을 검사하고, 에이전트를 위한 MCP 서버를 실행합니다.

## 명령

| 명령 | 기능 |
|---|---|
| `zeyn new <name>` | 표준 Feature 구조의 앱을 만듭니다 |
| `zeyn new <name> --lib` | 여러 앱이 공유하는 모델, Feature, 텍스트 라이브러리를 만듭니다 |
| `zeyn g feature <name>` | `models`, `services`, `actions`, `screens` 폴더와 동작하는 목록 Screen이 있는 `src/features/<name>`을 추가합니다 |
| `zeyn g screen <feature> <name>` | feature에 `screens/<name>.rs`를 추가하고 등록한 뒤 제목을 모든 로케일 파일에 추가합니다 |
| `zeyn routes` | Feature, Screen, 경로, API, 접근 권한을 나열합니다 |
| `zeyn check` | 컴파일하고 구조적 문제와 누락된 번역을 보고합니다 |
| `zeyn check --fix` | `src`, `tests`, `examples`에서 이름이 바뀐 Zeyn API를 다시 쓴 다음 검사합니다 |
| `zeyn check --json` | 안정된 코드, 이유, 해결 방법과 함께 보고서를 JSON으로 출력합니다 |
| `zeyn mcp` | AI 에이전트를 위한 MCP 서버를 stdio로 실행합니다 |

crates.io에서 설치합니다.

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

이 저장소 안에서는 Cargo로 CLI를 실행합니다.

```sh
cargo run -p zeyn-cli -- new my-app
cd my-app
cargo run
```

## 새 앱

`zeyn new`는 빌드되고, 테스트를 통과하며, 에이전트에게 주어진 규칙을 따르는 프로젝트를 만듭니다.

- `AGENTS.md`와 `CLAUDE.md`: 프로젝트 규칙과 에이전트를 위한 프레임워크 색인입니다.
- `.mcp.json`: `zeyn` MCP 서버를 등록하여 에이전트가 `zeyn_check`와 `zeyn_docs`를 호출할 수 있게 합니다.
- `config/app.json`: 기본값입니다. 환경 변수 `APP_*`가 이를 덮어씁니다. 예를 들어 `server.addr`에는 `APP_SERVER__ADDR`를 사용합니다.
- `locales/en.json`과 `locales/tr.json`: 사용자에게 보이는 모든 텍스트입니다.
- `src/features/`: Feature당 폴더 하나이며, 샘플 `notes` Feature가 있습니다.
- `rustfmt.toml`, `clippy.toml`, `deny.toml`, `rust-toolchain.toml`, `Cargo.toml`의 `[lints]`: CI가 강제하는 표준입니다.
  `rustfmt.toml`은 `chain_width = 40`을 설정하므로 코드가 위에서 아래로 읽힙니다. 이보다 긴 호출 체인은
  한 줄에 호출 하나씩, 평균적으로 한 줄에 점 하나 또는 둘이 되도록 나뉩니다. 에이전트와 사람은 긴 한 줄보다
  목록을 더 빨리 검토하며, diff에는 바뀐 호출 하나만 나타납니다. 이 설정을 유지하십시오.

  ```rust
  DataGrid::of(search_items)
      .paging(Paging::pages(25))
      .column(Item::code)
      .column(Item::qty)
  ```

  `DataGrid::of(search_items).paging(Paging::pages(25)).column(Item::code).column(Item::qty)`처럼 쓰지 않습니다.
- `build.rs`와 `Cargo.toml`의 `[package.metadata.zeyn]`에 있는 `max_file_lines = 300`: `src/`의 파일에 비어 있지 않은 줄이 더 많으면 `cargo build`가 멈춥니다. clippy `too_many_lines`(60, `clippy.toml`)는 함수에 같은 일을 합니다. [Project structure](structure.md)를 참고하세요.
- `Dockerfile`과 `.dockerignore`: 작은 컨테이너 이미지입니다.
- 일반적인 CI 서버용 파이프라인이며, 모두 같은 검사(`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check`)를 실행하고 `Cargo.toml`과 일치하지 않는 태그 `vX.Y.Z`를 거부합니다.
  - GitHub Actions용 `.github/workflows/ci.yml`, 그리고 `.github/dependabot.yml`(Cargo와 action의 주간 업데이트)과 pull request 체크리스트
  - GitLab용 `.gitlab-ci.yml`
  - Jenkins용 `Jenkinsfile`(Docker 에이전트, `cargo llvm-cov`로 커버리지)
- `sonar-project.properties`: SonarQube 분석입니다(Clippy 이슈와 `target/lcov.info`의 LCOV 커버리지). GitHub 워크플로는 저장소 변수 `SONAR_HOST_URL`과 시크릿 `SONAR_TOKEN`이 설정되어 있으면 이를 실행하고, Jenkins는 작업이 `SONAR_ENABLED=true`를 설정하면 실행합니다.
- `SECURITY.md`와 `CHANGELOG.md`(Keep a Changelog).
- git이 설치되어 있고(Windows, macOS, Linux) 폴더가 이미 저장소 안에 있지 않으면 `main` 브랜치의 로컬 git 저장소를 만듭니다. 아무것도 커밋하지 않으므로 첫 커밋에는 사용자 본인의 이름과 이메일이 들어갑니다. `--no-git`을 쓰면 건너뜁니다.

각 폴더에 대해서는 [프로젝트 구조](structure.md)를 참고하십시오.

## 생성기

`zeyn g feature`는 모델, 서비스, action, Screen마다 파일이 하나인 feature를 만듭니다.

```sh
zeyn g feature orders            # src/features/orders/: models/, services/, actions/, screens/
zeyn g screen orders detail      # src/features/orders/screens/detail.rs
```

짧은 형태 `zeyn g f`와 `zeyn g s`도 같은 일을 합니다. 새 feature는 바로 동작합니다. 모델, 메모리 데이터 서비스, 검색 action, 메뉴 항목이 있는 목록 Screen이 들어 있습니다. 필요 없는 폴더는 feature에서 삭제합니다(폴더와 feature의 `mod.rs`에 있는 `mod` 줄). `components/`는 생성되지 않습니다. Screen의 일부를 여러 곳에서 쓰거나 Screen 파일이 길어지면 직접 추가하고, 파일마다 `Node`를 반환하는 함수 하나를 둡니다(`components/<name>.rs`). Screen은 `<feature>.<name>.view` 권한을 받고 `locales/`의 모든 파일에 제목이 들어갑니다. 영어가 아닌 언어에는 영어 문장과 번역하라는 안내가 들어갑니다. 이 구조는 강제가 아닙니다. feature에 하나의 `screens.rs`가 있으면 `zeyn g screen`은 먼저 내용을 그대로 `screens/mod.rs`로 옮기고, `screens/`가 삭제되었으면 다시 만듭니다. 생성기는 파일을 덮어쓰지 않으며 snake case가 아닌 이름은 거부합니다.

## 앱 검사하기

`zeyn check`는 컴파일러를 실행한 다음 앱 자체의 검사(`App::check`)를 실행합니다. 중복된 경로, 두 번 등록된 Screen, Screen에 필요하지만 아무도 등록하지 않은 서비스, 사용할 수 없는 단축키, WCAG AA 대비에 못 미치는 테마 토큰, 어떤 언어에서 누락된 번역 키를 검사합니다. 문제가 있으면 오류로 종료하므로 CI에서 실행할 수 있습니다.

검사를 실패시키지 않는 경고도 출력합니다. 어떤 feature도 등록하지 않은 `#[screen]`이나 `#[action]`, 그리고 여러 항목이 있으면서 약 150줄을 넘은 하나의 `screens.rs`나 `actions.rs`입니다.

Zeyn 릴리스에서 API 이름이 바뀌면 `zeyn check --fix`는 예를 들어 `Grid`를 `DataGrid`로, `GridQuery`를 `DataQuery`로 이전 이름을 다시 쓴 다음 다시 검사합니다.

```sh
zeyn check --fix
```

모든 문제에는 안정된 코드가 있습니다. 앱, 기능, 경로, 서비스는 `Z01xx`, 보안은 `Z02xx`, 공개 페이지는 `Z03xx`, 메뉴, 명령, 단축키는 `Z04xx`, 데이터는 `Z05xx`, 페이지 검사는 `Z06xx`, 경고는 `W01xx`입니다. 텍스트 출력은 각 문제를 이유(`why`), 해결 방법(`help`), 이 문서로 가는 링크, 코드와 함께 보여 줍니다. `zeyn check --json`은 같은 보고서를 에이전트와 CI를 위한 하나의 JSON 문서로 출력하며, `ok`가 false이면 오류로 끝납니다.

```sh
zeyn check --json
```

```json
{
  "ok": false,
  "errors": [
    {
      "code": "Z0103",
      "summary": "screen `order_list` needs service `OrderStore`, which is not registered",
      "why": "the screen takes the service as an argument, so every request to it would fail",
      "fix": "add `.service(...)` to the App",
      "docs": "https://docs.zeynsoft.com/start/structure#services"
    }
  ],
  "warnings": [
    { "code": "W0105", "message": "translation `orders.title` is missing in `tr`" }
  ]
}
```

## Docker

생성된 `Dockerfile`은 두 단계로 빌드하며 distroless 이미지에서 루트 권한이 없는 사용자로 실행됩니다. 바이너리는 `<app> health`로 스스로 상태를 확인하며, `HEALTHCHECK`가 이를 사용합니다.

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

생성된 앱의 이미지는 약 43 MB이고, 유휴 상태의 앱은 RAM을 몇 MB만 사용합니다.

## MCP 서버

`zeyn mcp`는 stdio로 MCP를 사용합니다. 도구는 `zeyn_docs`(주제별 프레임워크 레퍼런스), `zeyn_describe`(현재 디렉터리에 있는 앱의 Feature, Screen, 경로, 문제), `zeyn_check`(`fix: true`를 주면 `check --fix`를 실행), `zeyn_test`, `zeyn_new_feature`, `zeyn_new_screen`입니다. [MCP와 AI 에이전트](mcp.md)를 참고하십시오.
