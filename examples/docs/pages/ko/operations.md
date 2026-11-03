# 운영

운영 환경에서 Zeyn 앱을 실행하는 방법입니다. 시간대 데이터, 서명된 라이선스, 여러 서버를 위한 공유 상태, 로그와 트레이스, PDF 문서, 이메일을 다룹니다.

## 시간대

`TimeZone::named`는 모든 IANA 시간대(`Europe/Istanbul`, `America/New_York`, `Australia/Sydney`, 대소문자 무관)와 고정 오프셋(`UTC+03:00`, `+05:30`)을 받습니다.

- 시간대 데이터는 `jiff`를 통해 바이너리에 내장된 IANA 시간대 데이터베이스입니다(약 200 KB). Windows를 포함한 모든 플랫폼에서 응답이 같으며 시스템 zoneinfo는 읽지 않습니다.
- 이력도 반영됩니다. 2016년까지의 튀르키예 서머타임과 그 이후의 영구 `+03`, 2007년 이전의 미국 규칙, 기록된 그 밖의 모든 변경이 있습니다.
- 존재하지 않는 벽시계 시각(시계가 앞으로 이동함)은 `LocalTimeError::Skipped`로 거부됩니다. 두 번 발생하는 시각(시계가 뒤로 이동함)은 더 이른 순간을 뜻합니다.
- 시간대는 이름이 처음 사용될 때 번들에서 읽어 프로세스가 유지되는 동안 보관하며, 각각 몇 킬로바이트입니다. 알 수 없는 이름은 보관되지 않으므로 클라이언트가 캐시를 키울 수 없습니다.
- `TimeZone::names()`는 셸의 시간대 선택기를 위한 일반적인 시간대의 짧은 목록입니다. 목록에 없는 사용자의 시간대는 그 사용자의 선택기에 추가됩니다.
- 새로운 시간대 규칙은 더 새로운 `jiff` 릴리스와 함께 옵니다(`cargo update -p jiff-tzdb`). 국가가 규칙을 바꾸면 다시 빌드하고 배포하십시오.

## 서명된 라이선스

라이선스 파일은 공급자만 가진 ed25519 키로 서명됩니다. 앱은 공개 키를 내장하므로 고객은 라이선스를 읽을 수는 있지만 변경하거나 새로 만들 수 없습니다. `signed-licenses` Cargo 기능으로 켭니다.

```
zeyn license keygen                      # writes zeyn-license.key, prints the public key
zeyn license sign acme.json --out license.json
```

`acme.json`에는 라이선스 필드가 들어 있습니다.

```json
{ "holder": "Acme", "valid_until": "2027-12-31", "modules": ["pos", "stock"], "max_users": 50 }
```

```rust
const LICENSE_KEY: &str = "f414a89e9dae57598d60141df96dd1c9fb655fdcf49f5eaf24e093fda7d332e3";

App::new("Shop")
    .license(SignedLicenseFile::new("license.json", LICENSE_KEY), Duration::from_secs(3600))
    .feature(Feature::new("pos").guard(Licensed::module("pos")).screen(pos))
```

- 파일은 `payload`에 라이선스 텍스트가, `signature`에 서명이 들어 있는 JSON입니다. 서명되는 것은 `zeyn-license-v1\n` 뒤에 페이로드 텍스트가 이어진 것이므로, 서명된 바이트는 저장된 바이트와 정확히 같습니다.
- 개인 키는 서명하는 컴퓨터에 남습니다. `keygen`은 키 파일을 절대 덮어쓰지 않으며, 새 프로젝트의 `.gitignore`에는 `*.key`가 있습니다. `sign`은 `--key`, `ZEYN_LICENSE_KEY` 변수, 또는 `zeyn-license.key`에서 키를 읽으며, `holder`가 없거나 `valid_until`이 `YYYY-MM-DD`가 아닌 라이선스는 거부합니다.
- 변경된 파일, 다른 키로 서명된 파일, 라이선스가 아닌 파일은 거부됩니다. `License::from_signed`는 `LicenseError`(`BadSignature`, `InvalidPublicKey`, `Malformed`)를 반환하고, 서버는 이유를 기록하며, 모든 `Licensed` 가드는 "Your license does not cover this."로 거부합니다. 읽을 수 없는 파일은 실패하는 다른 라이선스 소스처럼 마지막 라이선스를 유지합니다.
- 만료는 가드가 모든 요청에서 사용자의 오늘 날짜와 비교하여 검사하므로, 만료된 라이선스는 재시작 없이 "Your license has expired."와 함께 앱을 잠급니다. 갱신된 파일은 다음 새로 고침 때 읽힙니다.
- ed25519는 변경된 파일과 다른 키로 서명된 파일을 구별할 수 없습니다. 둘 다 일치하지 않는 서명이며 메시지도 그렇게 알려 줍니다.
- `max_users`는 담겨 있고 서명되지만 프레임워크가 강제하지는 않습니다. 무엇을 사용자로 셀지는 앱이 결정합니다.

## 여러 서버: Redis의 공유 상태

`redis` Cargo 기능을 사용하면 `RedisStore`가 로드 밸런서 뒤의 모든 인스턴스에서 같아야 하는 상태를 보관합니다.

```rust
let redis = RedisStore::new(config.require("redis.url")?, "billing").map_err(Error::Internal)?;
App::new("Billing")
    .rate_limit(RateLimit::per_ip(300, Duration::from_secs(60)))
    .rate_store(redis.clone())
    .datagrid_views(redis)
```

서버가 사용자별로 무엇을 어디에 보관하는지는 다음과 같습니다.

| 상태 | 위치 | 공유 |
|---|---|---|
| 속도 제한 카운터 | `RateStore` | `RedisStore` |
| 저장된 그리드 뷰와 도크 레이아웃 | `DataGridViewStore` | `RedisStore` |
| 유휴 제한 시간 | 브라우저(앱의 모든 탭) | 서버에는 없음 |
| 로그인과 재인증 | 싱글 사인온 프록시 | 서버에는 없음 |
| 민감한 Screen의 사유 | `HttpOnly` 쿠키 | 서버에는 없음 |
| 폼 초안 | `sessionStorage` | 서버에는 없음 |
| 라이선스 캐시 | 프로세스별, 소스에서 다시 로드 | 각 서버가 자신의 것을 로드 |
| 백그라운드 작업 | 프로세스별 | 공유되지 않음. `enterprise.md`의 작업 섹션 참고 |

- 속도 제한은 하나의 Lua 스크립트로 된 토큰 버킷이므로, 서로 다른 서버의 동시 요청도 제한을 넘지 않습니다. 시간은 앱 서버가 아니라 Redis에서 가져옵니다. 카운터는 마지막 사용 후 한 구간이 지나면 만료됩니다.
- 뷰는 `zeyn:<prefix>:view:<user length>:<user>:<grid>` 아래의 JSON이며 만료되지 않습니다.
- 연결은 처음 사용할 때 만들어지고 스스로 복구됩니다. 호출은 500 ms 후 타임아웃됩니다. Redis가 중단되면 속도 제한은 요청을 통과시키고, 뷰는 로드도 저장도 되지 않으며(그리드는 자체 레이아웃으로, 도크는 `localStorage`로 대체), 중단과 복구는 각각 한 번씩 기록되고, 실패할 때마다 5초 동안 Redis를 건드리지 않으므로 요청이 각각 타임아웃을 기다리지 않습니다.
- `RateStore::take`는 스토어가 네트워크와 통신할 수 있도록 퓨처를 반환합니다. `MemoryRateStore::take`는 직접 사용을 위해 동기식으로 유지됩니다.
- TLS(`rediss://`)는 내장되어 있지 않습니다.
- `ZEYN_TEST_REDIS`가 설정되면 테스트는 실제 서버를 상대로 실행됩니다.

  ```
  docker run -d --rm --name zeyn-redis -p 6379:6379 redis:7-alpine
  ZEYN_TEST_REDIS=redis://127.0.0.1:6379 cargo test -p zeyn --features redis --test redis
  docker stop zeyn-redis
  ```

## 로그와 트레이스

프레임워크는 `tracing`을 통해 보고합니다. 스스로 출력하는 일은 없습니다.

- `App::serve`와 `App::serve_until`은 앱이 먼저 설치하지 않았다면 구독자를 설치합니다. 이벤트는 `log.level`(기본 `info`, `warn,zeyn=debug` 같은 모든 `tracing` 대상 필터)로 필터링되어 텍스트 줄로 표준 오류에 출력됩니다. JSON 줄이나 다른 싱크를 원하는 앱은 서비스하기 전에 자체 구독자를 설치하며, Zeyn은 이를 건드리지 않습니다.
- `info`에서는 출력이 예전 그대로입니다. 수신 주소, 시작 경고(누락된 번역, `DevAuth`, `dev_latency`), 실패하는 라이선스 소스, 내부 오류입니다. `debug`에서는 모든 호출이 종류, Screen, action, 사용자 id, 테넌트 id, 결과, 오류 코드, 밀리초가 있는 `call finished` 줄을 추가합니다.
- 트레이싱 백엔드를 위한 스팬:

  | 스팬 | 수준 | 필드 |
  |---|---|---|
  | `request` | info | 메서드, 경로(쿼리 문자열 제외), 상태, ms, 그리고 알려지면 kind, screen, action, user, tenant, ok, error |
  | `run` | info | 애플리케이션 코드: action, 데이터 소스(그리드 가져오기, 행 조회, 뷰 스토어), Screen 빌더 |
  | `interceptor` | debug | 이름(타입 이름 또는 `Interceptor::name`), 단계(`before` 또는 `after`) |

- `otel` Cargo 기능을 사용하면 `Config`에 `otel.endpoint`가 설정된 경우 스팬이 OTLP/HTTP로 내보내집니다(`http://collector:4318`, 스팬은 `/v1/traces`로 전송). `otel.service_name`은 서비스 이름을 지정하며 기본값은 앱의 이름입니다. 스팬은 백그라운드 스레드에서 배치로 처리되고 `serve_until`이 멈출 때 플러시됩니다.
- 내보내기가 없으면 표준 오류는 이벤트만 받으므로 스팬이 만들어지지 않고 스팬 비용은 캐시된 검사 한 번뿐입니다. 다른 서버에서 `App::router`로 마운트된 앱과 테스트도 같습니다.
- 기록되지 않는 것: 필드 값(마스킹 여부와 상관없이), 폼과 API 입력, 쿼리 문자열, 쿠키, 헤더, 시크릿. 테스트는 trace 수준 구독자 아래에서 그리드, 폼 저장, API 호출, 공개를 실행하고 마스킹된 값이 나타나지 않는지 검사합니다. 두 가지는 앱의 몫으로 남습니다. 경로는 기록되므로 경로에 개인 데이터를 넣지 마십시오. 그리고 `Error::Internal` 메시지는 작성된 그대로 기록됩니다.

## PDF 문서

`pdf` Cargo 기능은 [Typst](https://typst.app) 템플릿으로 PDF 문서를 렌더링합니다. Typst가 크기 때문에 기본적으로 꺼져 있습니다(아래 수치 참고).

```rust
const INVOICE: PdfTemplate = PdfTemplate::new("invoice", include_str!("../templates/invoice.typ"));

#[action(permission = "sales.order.read")]
pub async fn invoice_data(p: OrderParams, store: Orders, ctx: Ctx) -> Result<InvoiceData> { ... }

Screen::window("Order").body(PdfLink::new("Print invoice", INVOICE, invoice_data, params))

App::new("Shop").service(Pdf::from_config(&config)?)
```

- `PdfLink`는 Screen에 다운로드 버튼을 둡니다. `GET /_zeyn/pdf`는 접근 검사와 함께 Screen을 열고, 사용자를 위해 action을 실행하며(그 권한과 인터셉터가 적용되고 호출 종류는 `Document`), action이 반환하는 것으로 템플릿을 렌더링합니다. action을 실행할 수 없는 사용자에게는 버튼이 보이지 않습니다.
- `exportable()`이 있는 그리드는 툴바의 `Export::Pdf`(`GET /_zeyn/grid/export?format=pdf`)로 목록 보고서도 인쇄합니다. 사용자의 열, 레이블, 필터, 정렬이 반영되고, 숫자는 오른쪽 정렬되며, 헤더 행이 반복되고 페이지 번호가 있습니다. 보고서는 최대 2,000행(`MAX_PDF_ROWS`, 또는 더 작으면 그리드의 내보내기 제한)을 담으며 행이 빠졌을 때 그렇다고 알려 줍니다.
- **데이터는 Typst 코드가 되지 않습니다.** action의 출력은 JSON으로 직렬화되고 템플릿은 이를 `data`로 읽습니다(렌더러가 템플릿 앞에 `#let data = json("/data.json")`을 바인드합니다). `#panic()`, `]`, `$x$` 같은 값은 텍스트로 출력되며 테스트가 이를 검사합니다. 템플릿은 그 밖의 어떤 것도 읽을 수 없습니다. 파일도 패키지도 안 됩니다.
- 렌더링은 블로킹 스레드에서 실행됩니다. 최대 `pdf.max_concurrent`개의 문서(1에서 64, 기본 두 개이며 CPU가 하나면 하나)가 동시에 렌더링되고 나머지는 슬롯을 기다립니다. Typst의 메모이제이션 캐시는 문서마다 비워지므로 메모리가 다시 내려갑니다.

| 키 | 의미 |
|---|---|
| `pdf.fonts` | 추가 `.ttf`, `.otf`, `.ttc`, `.otc` 글꼴의 디렉터리이며 시작할 때 한 번 읽습니다 |
| `pdf.max_concurrent` | 동시에 렌더링하는 문서 수 |

글꼴: Libertinus Serif(일반, 굵게, 기울임, 굵은 기울임. SIL Open Font License)가 내장되어 터키어를 지원하며, DejaVu Sans Mono는 `₺`처럼 Libertinus에 없는 기호를 위한 대체 글꼴로 내장되어 있습니다. 이들의 라이선스는 `crates/zeyn/assets/fonts`에 있습니다. 수학 글꼴은 없으며, 템플릿이 수식을 설정한다면 `pdf.fonts`로 추가하십시오.

Windows(릴리스 빌드, x86_64)에서 측정한 결과입니다.

| | 결과 |
|---|---|
| 바이너리 크기 | `pdf` 없이 5.5 MB, 있으면 약 46 MB (Typst가 약 39 MB, 글꼴이 1.6 MB를 추가) |
| 한 페이지 문서 | 4에서 7 ms, 렌더링 중 힙 1 MB 미만, 파일 10 KB |
| 1,000행(25페이지)의 목록 보고서 | 0.54에서 0.74초, 렌더링 중 최대 힙 51 MB, 이후 유지되는 것 0, 파일 0.87 MB |
| 시작 후 첫 문서 | Typst가 지연 생성하는 자체 테이블 때문에 약 42 MB의 힙이 남습니다 |
| 글꼴 | 글꼴 색인에 힙 0.25 MB |

디버그 빌드는 약 10배 느립니다(쇼케이스에서 2,000행 보고서가 71초 걸렸습니다).

## 이메일

`email` Cargo 기능은 [lettre](https://lettre.rs)로 알림을 보냅니다.

```rust
let mailer = Mailer::from_config(&config)?;
App::new("People").mailer(mailer)

#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(r: TransitionRequest<LeaveRequestId>, store: Leaves, mailer: Mailer, ctx: Ctx) -> Result<LeaveState> {
    let state = leave_flow().apply(&ctx, current, &r.transition, r.comment.as_deref())?;
    mailer.send(&ctx, Mail::transition(&ctx, &manager_email, &employee)?)?;
    Ok(state)
}
```

- `Mailer::send`는 메시지를 렌더링하여 메모리 큐(최대 10,000개 메시지)에 넣고 반환합니다. `zeyn.mail` 백그라운드 작업이 큐를 즉시 보내고, 실패한 메시지를 백오프와 함께 다시 시도하며(`Mailer::retry`가 달리 지정하지 않으면 10초부터 다섯 번), 큐를 매분 확인합니다. 메시지는 여덟 번 실패하면 로그에 오류와 함께 버려집니다. 모든 작업과 마찬가지로 프로세스가 멈출 때 보내지 않은 큐는 사라지며, 내구성 있는 전달에는 데이터 계층의 데이터베이스 기반 큐가 필요합니다.
- 메시지는 i18n 카탈로그에서 가져옵니다. `<template>.subject`, `<template>.text`, 선택적으로 `<template>.html`이며 `{name}` 인수를 사용합니다. 수신자의 언어는 `Mail::locale`이 설정하지 않으면 발신자의 언어입니다. 모든 메시지에는 일반 텍스트 부분과 HTML 부분이 있습니다. HTML 부분에서는 모든 인수가 이스케이프되며, `.html` 항목이 없으면 HTML은 이스케이프된 텍스트입니다. 제목에서는 줄 바꿈이 제거되고 주소는 파싱되므로 데이터가 헤더나 수신자를 추가할 수 없습니다.
- `Mail::transition`은 호출이 방금 `Workflow::apply`로 수행한 이동(레코드, from, to, 누가, 댓글)을 내장된 `zeyn.mail.transition` 템플릿으로 작성합니다. 쇼케이스는 휴가 신청이 이동하면 관리자에게 알립니다.

| 키 | 의미 |
|---|---|
| `mail.from` | `Northwind <no-reply@example.com>` 같은 발신자 (필수) |
| `mail.transport` | `smtp`, `file`(메시지당 `.eml` 파일, 개발과 테스트용), 또는 `log`(수신자 수와 크기가 있는 로그 줄이며 주소나 내용은 절대 없음, 기본값) |
| `mail.smtp.host`, `mail.smtp.port` | SMTP 서버 |
| `mail.smtp.tls` | `starttls`(기본), `tls`(암시적 TLS, 보통 465 포트), 또는 `none`(로컬 릴레이 전용) |
| `mail.smtp.user`, `mail.smtp.password` | 자격 증명. 비밀번호는 환경 변수 같은 시크릿 소스에서 읽으십시오 |
| `mail.file.dir` | `file` 전송의 디렉터리 |

TLS는 Mozilla 루트 인증서(`webpki-roots`)와 함께 rustls를 사용합니다.
