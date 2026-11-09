# MCP와 AI 에이전트

Zeyn은 두 가지 에이전트를 위해 만들어졌습니다. Zeyn 앱을 작성하는 에이전트와, 사용자를 대신해 실행 중인 앱을 사용하는 에이전트입니다.

## 앱을 작성하는 에이전트

에이전트가 작업하는 프로젝트에는 자체 규칙과 도구가 들어 있습니다.

- `AGENTS.md`와 `CLAUDE.md`는 명령, 프로젝트 구조, 언어의 규칙, 프레임워크 색인을 나열합니다.
- `.mcp.json`은 `zeyn mcp`를 등록합니다. stdio 기반 MCP 서버이며 다음 도구를 제공합니다.

| 도구 | 기능 |
|---|---|
| `zeyn_docs` | 주제별 프레임워크 레퍼런스: `rules`, `language`, `protocol` |
| `zeyn_describe` | 앱의 Feature, Screen, 경로, 접근 권한, API, 문제, 누락된 번역 |
| `zeyn_check` | 컴파일하고 문제를 보고합니다. `fix: true`는 먼저 이름이 바뀐 API를 다시 씁니다 |
| `zeyn_test` | `cargo test`를 실행하고 출력의 끝부분을 반환합니다 |
| `zeyn_new_feature` | Feature 폴더를 만들고 등록합니다 |
| `zeyn_new_screen` | Feature에 화면 파일을 추가하고 등록합니다 |

컴파일러가 주된 방어선입니다. 컴파일러가 강제할 수 있는 모든 규칙에는 `compile_fail` 테스트와 에이전트가 행동할 수 있도록 작성된 `#[diagnostic::on_unimplemented]` 메시지가 있으며, 예는 다음과 같습니다.

```sh
error[E0277]: `min()` and `max()` cannot be used on a `bool` column
  = note: `min()` and `max()` work on i32, i64, Money, Date, Time, DateTime, LocalDateTime and their Option forms
```

`ZEYN_DESCRIBE=1 cargo run`은 `zeyn_describe`가 반환하는 것과 같은 설명을 출력하고 종료합니다.

## 앱을 사용하는 에이전트

```rust
App::new(t!("app.name")).mcp(Mcp::read_only())
```

`agents`를 사용하면 앱의 Screen이 도구가 됩니다.

- 그리드마다 검색 도구 하나,
- 폼마다 저장 도구 하나. 폼의 필드와 규칙이 스키마가 됩니다(`Mcp::read_write()`에서만).

에이전트는 두 가지 방법으로 이 도구에 접근합니다.

- **브라우저에서** 브라우저가 지원하면 WebMCP(`document.modelContext.registerTool`)를 통해 접근합니다. 현재 Screen의 도구는 Screen이 열릴 때 등록되고 닫힐 때 제거됩니다.
- **서버에서** `POST /_zeyn/mcp`(JSON-RPC)로 접근합니다. `Authorization` 헤더가 필요하므로 브라우저 세션만으로는 이를 호출할 수 없습니다.

## 사람과 같은 규칙

- 에이전트는 자신이 대행하는 사용자의 권한만 정확히 가집니다. 사용자가 열 수 없는 Screen과 action의 도구는 목록에 나타나지 않습니다.
- 호출은 일반 호출의 가드, 인터셉터, 검증, 속도 제한을 거칩니다.
- 감사 기록은 에이전트가 한 호출을 표시하므로 검토자가 구분할 수 있습니다.
- `Mcp::read_only()`는 에이전트가 검색은 할 수 있지만 저장은 못 하게 합니다. 에이전트가 데이터를 변경해야 하는 앱에서만 `read_write()`를 선택하십시오.
