# 엔터프라이즈 기능

업무용 앱을 위한 횡단 기능입니다. 데이터 감사, 개인 데이터 보호, 안전한 배포, 상태 점검, 세션 안전, CSV와 XLSX 가져오기, 백그라운드 작업, 승인 워크플로가 있습니다. 모든 것이 서버에서 검사되며, 브라우저 런타임은 사용하기 편하게 만들 뿐입니다. KVKK와 GDPR을 염두에 둔 개인정보 보호법이지만, 여기 있는 어떤 것도 특정 국가나 산업에 한정되지 않습니다.

## 필드 수준 데이터 감사

`AuditLog`는 폼 제출마다, 그리고 `audit`으로 표시된 action을 호출할 때마다 `AuditEntry` 하나를 기록합니다. 저장의 경우 항목에는 레코드와 변경된 모든 필드도 기록됩니다.

```json
{"kind":"form","screen":"customer","action":"save_customer","ok":true,"user":"ayse",
 "model":"Customer","record":"2","changes":[{"field":"name","old":"Customer 2","new":"Renamed"}]}
```

- 폼은 이를 스스로 처리합니다. 이전 값은 서버에서 Screen이 불러온 레코드에서, 새 값은 검증된 `Valid<M>`에서 가져옵니다. 클라이언트가 보낸 어떤 것도 이전 값으로 사용되지 않으며, 폼이 클라이언트에서 받지 않는 필드는 변경된 것으로 나타나지 않습니다.
- API 저장은 `Valid<M>` 입력에서 모델과 키를 기록합니다. 데이터베이스에 무엇이 있는지는 action만 알고 있으므로 차이는 action이 직접 보고합니다.

  ```rust
  #[action(permission = "crm.customer.write", audit)]
  pub async fn save_customer(c: Valid<Customer>, store: Customers, ctx: Ctx) -> Result<Customer> {
      let current = store.get(c.id)?;
      ctx.record_changes(&current, &c);
      store.save(c.into_inner())
  }
  ```

  폼 저장 action에서 같은 일을 해도 괜찮습니다. action의 보고가 폼의 것을 대체합니다.
- 값은 언어에 독립적인 일반 문자열입니다(`1234.56 TRY`, `2024-05-01`, choice 값). 행 버전은 제외됩니다. 마스킹된 필드는 마스킹된 채로 유지됩니다(아래 참고).
- 실패한 저장도 변경 사항 없이 레코드를 기록합니다.

## 필드 마스킹

개인 데이터를 담는 텍스트 필드는 `#[mask]`로 표시합니다. 여기서는 주민 번호가 `•••• 8901`로, 이메일이 `p••••@example.com`으로, 메모가 `••••`로 표시됩니다.

```rust
#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    pub name: String,
    #[mask(last4)]
    pub national_id: String,
    #[mask(email, permission = "hr.contact.unmask")]
    pub email: Option<String>,
    #[mask]
    pub notes: Option<String>,
}
```

- 마스킹은 서버에서 일어나며, 그리드 셀, 폼, API 출력, 에이전트 도구, 따라서 셀로 만들어지는 CSV 내보내기에도 적용됩니다. 원본 값은 권한 없이는 브라우저나 API 클라이언트에 도달하지 않습니다.
- 마스킹 해제 권한은 속성이 따로 지정하지 않으면 스네이크 케이스의 `<model>.<field>.unmask`입니다(`person.national_id.unmask`).
- 권한이 있는 사용자도 먼저 마스킹된 텍스트와 공개 버튼을 봅니다. 공개하면 `POST /_zeyn/reveal`을 호출하며, 이는 권한을 다시 검사하고 감사 기록(종류 `reveal`, 필드는 `revealed`)을 남깁니다. 거부된 시도도 감사됩니다.
- 권한이 있는 API 클라이언트는 원본 값을 받으며, 그런 각 필드는 호출의 감사 기록의 `revealed`에 나열됩니다.
- 폼은 마스킹된 필드를 읽기 전용 텍스트로 표시합니다. 저장된 값은 사용자가 이를 공개할 수 있고 공개한 후에 바꾼 경우가 아니면 저장할 때 유지됩니다. 비어 있는 마스킹 필드는 일반 입력이므로, 데이터 입력 담당자는 나중에 다시 읽을 수 없어도 한 번 채울 수 있습니다.
- 공개할 수 없는 사용자에게는 마스킹된 필드에 대한 필터와 정렬이 무시되므로, 검색으로 값을 추측할 수 없습니다. 이는 그리드의 모든 읽기(페이지, 내보내기, 모든 행에 대한 일괄 action, 저장된 뷰, 에이전트 도구)와 `DataQuery`를 받는 API action에 적용됩니다. 마스킹된 열에 `.filter()`를 추가하지 마십시오.
- 페이징 커서는 마스킹된 필드의 값을 절대 담지 않습니다. 마스킹된 필드로 정렬된 목록은 키가 아니라 위치로 페이징합니다.
- 그리드는 마스킹된 필드로 그룹화하지 않습니다. 이에 대한 `group_by`는 무시되고 기록됩니다.
- 아일랜드와 그 밖의 데이터 소스의 출력은 API 출력처럼 모든 깊이에서 마스킹됩니다. 그 안의 모든 모델의 마스킹된 필드는 마스킹된 텍스트로 나오며, 민감한 모델의 키는 읽기로 기록됩니다.
- 그리드 공개는 키에 대한 필터와 함께 그리드의 action을 통해 행을 조회하므로, action은 키 필드에 대한 필터를 지켜야 합니다.
- `#[mask]`는 `String`과 `Option<String>`에서만 컴파일됩니다.

## 민감한 레코드 읽기

누가 변경했는지뿐 아니라 누가 어떤 개인 데이터를 언제 보았는지도 기록됩니다.

- `#[entity(table = "cases", sensitive)]`(또는 `#[entity]` 아래의 `#[sensitive]`)는 모델을 표시합니다. 사용자에게 그런 레코드가 표시될 때마다 호출의 감사 기록이 이를 `reads`에 나열합니다(`{"model":"Case","id":"2"}`). 그리드 페이지의 행, 폼, 에이전트 도구 결과, API 출력이 그렇습니다. 읽기는 데이터가 렌더링되는 곳에서 기록되므로 레이아웃 안의 컴포넌트도 포함됩니다.
- `#[screen(..., sensitive)]`는 하나의 모델에 묶이지 않은 개인 데이터를 표시하는 Screen에 대해, Screen을 열 때마다 `model`이 Screen 이름이고 `id`가 그 매개변수인 읽기로 기록합니다.
- `#[screen(..., reason)]`은 데이터를 표시하기 전에 사용자에게 사유도 묻습니다. 서버는 사유가 주어질 때까지 페이지, 그 조각, 그리드, 폼, 공개 호출에 코드 `reason_required`와 함께 `428`로 응답합니다(전체 페이지 로드에서는 사유 폼이 있는 페이지). 런타임은 대화 상자에서 묻고, `POST /_zeyn/reason {screen, reason}`(3자에서 300자)을 보낸 다음 요청을 반복합니다. 사유는 `HttpOnly`, `SameSite=Strict` 쿠키에 15분 동안 보관되고, 주어진 Screen만 열며, 그 Screen의 모든 감사 기록의 `reason`에 기록됩니다. 에이전트와 API 클라이언트는 사유를 줄 수 없으므로 그런 Screen을 열 수 없습니다.
- 민감한 모델에 대한 그리드의 내보내기(CSV, XLSX, PDF)는 `export`가 설정된 파일당 감사 기록 하나를 남깁니다. 모델, 형식, 행 수, 요청된 정렬과 필터가 들어갑니다. 모든 행을 `reads`에 나열하지 않으므로 큰 내보내기도 저렴하게 유지되며, 다른 모델의 내보내기는 추가 비용이 없습니다. `sensitive` 또는 `reason` Screen에서 내보내면 Screen 읽기도 기록됩니다.
- Screen의 지연 부분(`POST /_zeyn/node`, 지연 탭에서 사용)은 Screen을 여는 것과 똑같이 인터셉터를 실행하고 읽기를 기록합니다.
- `ctx.record_read(&record)`는 애플리케이션 코드에서 읽기를 기록합니다.
- 읽기 기록은 민감한 레코드가 표시되자마자 페이지 로드를 포함한 모든 호출에 대해 `AuditLog`가 기록합니다. 감사 인터셉터가 없으면 아무것도 기록되지 않습니다. 개인 데이터를 보관하는 앱은 항상 하나를 등록해야 합니다.

## 승인 워크플로

워크플로는 타입이 있는 작은 상태 머신입니다. 상태는 `#[choice]` enum이며, 각 이동은 시작 상태, 끝 상태, 필요한 권한을 지정합니다.

```rust
#[choice]
pub enum LeaveState { Draft, Submitted, Approved, Rejected }

pub fn leave_flow() -> Workflow<LeaveState> {
    Workflow::new()
        .transition("submit", LeaveState::Draft, LeaveState::Submitted, "people.leave.request")
        .transition("approve", LeaveState::Submitted, LeaveState::Approved, "people.leave.approve")
        .transition("reject", LeaveState::Submitted, LeaveState::Rejected, "people.leave.approve")
}

#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(req: TransitionRequest<LeaveRequestId>, store: LeaveStore, ctx: Ctx) -> Result<LeaveState> {
    let mut leave = store.get(req.id)?;
    leave.state = leave_flow().apply(&ctx, leave.state, &req.transition, req.comment.as_deref())?;
    store.save(leave)
}
```

- `apply`는 스토어에서 읽은 상태로 서버에서 실행됩니다. 알 수 없는 이동은 `404`, 더 이상 시작 상태에 있지 않은 레코드는 `409`(누군가 먼저 이동시킴), 권한이 없으면 `403`, 1000자를 넘는 댓글은 `422`입니다.
- 각 이동은 `transition`의 감사 기록에 기록됩니다. 이름, from, to, 댓글이 들어가며, 사용자와 시각은 기록 자체에 있습니다.
- `Transitions::of(flow, state, id, action)`은 현재 상태, 선택적 댓글, 그 상태에서 사용자가 할 수 있는 이동마다 버튼 하나를 보여 줍니다(`Workflow::allowed`). 레코드 키는 서버의 Screen에서 오므로 클라이언트가 다른 레코드를 이동시킬 수 없습니다. 에이전트는 이를 폼 도구로 봅니다.
- 버튼 레이블은 카탈로그의 `workflow.<name>`에서 가져오며, `.label(...)`로 직접 설정할 수 있습니다.
- `.confirm(name, message)`는 전환 `name`이 실행되기 전에 `ActionButton::confirm` 대화 상자로 묻고, `.tone(name, Tone::Danger)`는 그 버튼을 빨간색으로 그립니다. 서버는 어떤 경우든 전환과 권한을 확인합니다.
- `Workflow::check`는 잘못된 권한과 같은 상태에서 두 번 선언된 이동을 보고합니다. 테스트에서 호출하십시오.
- 쇼케이스의 휴가 신청이 이를 사용합니다. 초안으로 저장하고, 제출한 다음, 승인하거나 거부합니다.

## 안전한 배포

- 모든 응답에는 `x-zeyn-build`가, 모든 페이지에는 `<meta name="zeyn-build">`가 붙습니다. id는 설정되어 있으면 `ZEYN_BUILD_ID`(문자, 숫자, `.`, `_`, `-`, 최대 64자)이고, 아니면 실행 파일의 해시입니다. `zeyn::server::build_id()`가 로그용으로 이를 반환합니다.
- 런타임이 다른 빌드의 응답을 보면 정중한 알림을 표시합니다. 새 버전이 있으니 준비되면 새로 고치라는 내용입니다. 스스로 새로 고치는 일은 없습니다.
- 사용자가 폼에 입력한 내용은 Screen, 매개변수, 폼별로 `sessionStorage`에 보관되고, 새로 고침 후 메모와 버리기 버튼과 함께 복원됩니다. 마스킹, 비밀번호, 파일, 숨김 입력은 보관되지 않습니다. 초안은 입력된 시점의 행 버전을 유지하므로, 그사이에 누군가 바꾼 레코드의 초안을 저장하면 충돌로 거부됩니다. 저장에 성공하면 초안이 지워지며, 하루가 지난 초안은 버려집니다. `sessionStorage`는 그 브라우저 탭에서만 유지됩니다.

## 상태 점검

- `GET /_zeyn/health`는 프로세스가 실행되는 동안 `200 ok`로 응답합니다. 라이브니스에 사용하십시오.
- `GET /_zeyn/ready`는 `App::readiness(name, check)`로 추가한 모든 점검을 동시에, 각각 5초 제한으로 실행하고 `200 {"status":"ready"}` 또는 `503 {"status":"not_ready","failed":["database"]}`로 응답합니다. 점검 이름만 표시되며 오류는 절대 표시되지 않습니다.
- 둘 다 `base_path` 아래에 있으며(`/billing/_zeyn/health`), `Cache-Control: no-store`로 응답하고 로그인이 필요 없습니다.
- 준비 상태 결과는 1초 동안 재사용되며(`READY_CACHE`), 한 번에 요청 하나만 점검을 실행하므로 프로브가 쏟아져도 데이터베이스에 부하를 줄 수 없습니다.
- health는 속도 제한을 받지 않으므로 바쁜 프로브가 스스로 차단되어 정상적인 앱을 재시작시키는 일이 없습니다. 준비 상태는 `probe_ips` 주소에서 온 것이 아니면 속도 제한에 포함됩니다.
- IP 규칙은 여전히 적용됩니다. `deny_ips`는 항상, `allow_only_ips`도 프로브 주소가 `App::probe_ips([...])`에 나열되어 있지 않으면 적용되며, `App::probe_ips`는 이 두 엔드포인트만 그 주소에 엽니다.

## 개발 지연

`App::dev_latency(Duration::from_millis(400))`는 모든 호출(페이지, 조각, 그리드, 폼, API, MCP, 런타임 호출. 정적 에셋이나 health 프로브는 제외)을 지연시켜 개발자가 느린 네트워크의 사용자처럼 로딩 상태와 더블 클릭을 볼 수 있게 합니다. 디버그 빌드에서만 동작합니다. 릴리스 빌드에서는 이 메서드가 아무 일도 하지 않고 지연 코드는 컴파일에서 제외되므로, 실수로 남겨 두어도 운영 환경을 느리게 할 수 없습니다. 켜져 있는 동안 시작할 때 경고가 출력됩니다.

## 세션 안전

로그인은 싱글 사인온 프록시, 내장 OpenID Connect 로그인, bearer 토큰(`docs/auth.md` 참고)에서 오므로, 이 기능들은 인증기가 사용자가 누구인지, 그리고 재인증을 위해 언제 로그인했는지(`Identity::authenticated_at`, `ForwardAuth::auth_time("x-auth-time")`은 Unix 초 또는 RFC 3339를 읽으며, `DevAuth`는 모든 요청을 방금 로그인한 것으로 보고합니다)만 알려 주면 됩니다.

- `App::idle_timeout(Duration)`은 종료 `idle_warning` 전(기본 1분)에 런타임이 사용자에게 카운트다운과 "로그인 유지" 버튼이 있는 알림 대화 상자로 경고하게 하며, 이 버튼은 `POST /_zeyn/keepalive`를 호출하여 앞단의 프록시가 활동을 볼 수 있게 합니다. 앱의 어느 탭에서든 입력이 있으면 활동으로 계산됩니다. 종료 시점에는 사용자가 `logout_url`로, 없으면 `return_to`와 함께 `login_url`로, 그것도 없으면 로그아웃된 화면으로 이동합니다. 폼 초안은 유지됩니다. 유휴 제한 시간은 잊힌 화면만 닫으며, 실제 세션을 끝내는 것은 ID 제공자의 몫입니다.
- `#[action(permission = "...", reauth)]`는 `App::reauth_within`(기본 5분) 안의 로그인이 필요합니다. 그렇지 않으면 클라이언트가 무엇을 보내든 서버는 코드 `reauth_required`와 함께 `401`로 응답합니다(API에서는 problem 타입 `urn:zeyn:problem:reauth_required`). 런타임은 사용자에게 새 탭에서 다시 로그인하도록 요청하고(`reauth_url`, 없으면 `login_url`) 돌아오면 요청을 반복하므로 입력한 내용은 사라지지 않습니다. 로그인 시각이 없는 ID는 통과하지 못합니다.

## CSV 가져오기

```rust
#[action(permission = "people.leave.import", audit)]
pub async fn import_leaves(rows: Vec<Valid<LeaveRequest>>, store: LeaveStore) -> Result<usize> { ... }

Screen::new("Import").body(Import::of(import_leaves).max_rows(1000))
```

- 사용자가 CSV 파일을 선택합니다. 런타임은 이를 UTF-8로 읽고, 유효한 UTF-8이 아니면 Windows-1254(터키어 스프레드시트)로 읽은 다음, 텍스트를 JSON 본문으로 `POST /_zeyn/import`에 보냅니다(multipart 불필요, 최대 8 MB). 서버는 요약, 처음 열 행의 미리 보기, 행별 오류, 다운로드할 수 있는 오류 보고서로 응답합니다.
- 파싱: 바이트 순서 표시는 제거되고, `,`, `;`, 탭은 헤더 줄에서 감지되며, 구분자와 줄 바꿈을 포함한 `"` 인용을 지원하고, `\r\n` 또는 `\n`을 지원합니다.
- 제한은 파일을 읽은 후가 아니라 읽는 동안 검사됩니다. 읽기는 행 제한에서 멈추고, 512개를 넘는 셀의 헤더는 거부되며, 데이터 행은 헤더보다 최대 한 셀만 더 유지합니다(따라서 추가 값도 보고됩니다). XLSX 파일은 헤더의 열 수만큼만 열을 읽으며, 헤더는 모델이 가져올 수 있는 열 수만큼만 열을 지정할 수 있고, 16 MB를 넘는 공유 문자열 부분은 아무것도 압축 해제하기 전에 거부됩니다.
- 열은 앱의 어떤 언어로든 필드 이름이나 레이블로 대소문자와 터키어의 점 있는 i와 점 없는 i를 무시하고 매칭됩니다. 알 수 없는 열, 읽기 전용 열, 키 열, 행 버전 열은 거부됩니다.
- 모든 행은 모델의 폼과 똑같이 읽히고 검증됩니다. 그 언어의 사용자가 입력하는 방식의 값(`1.234,56`, `05.12.2026`), 필드 규칙, 모델 자체의 `validate`가 적용됩니다.
- 가져오기는 유효한 행만 action에 보냅니다. `.all_or_nothing()`은 모든 행이 유효하지 않으면 아무것도 가져오지 않습니다. 가져오기 시 서버는 파일을 다시 읽고 검증하며, 미리 보기의 어떤 것도 신뢰하지 않습니다. 두 단계 모두 action의 권한이 필요하며 가져오기는 감사됩니다.
- `.max_rows(n)`은 행을 제한합니다(기본 5000, 50000을 넘지 않음). 오류 보고서는 스프레드시트 수식을 무력화합니다. 마스킹된 필드는 미리 보기에서 마스킹되어 표시됩니다.

### XLSX 가져오기

`xlsx` Cargo 기능을 사용하면 같은 컴포넌트가 Excel 통합 문서(`.xlsx`)도 받습니다. 런타임은 파일을 base64로 보내고(같은 JSON 본문에서 `"csv"` 대신 `"xlsx"`), 서버는 `calamine`으로 첫 번째 워크시트를 읽어 CSV와 같은 행 단위 검증, 미리 보기, 오류 보고서로 처리합니다.

- 셀은 타입을 유지합니다. 사용자의 언어와 상관없이 숫자는 숫자로, 날짜 셀은 날짜로 읽히므로 터키어 세션에서도 `1234.5`는 여전히 1.234,50입니다. 날짜 셀의 벽시계 시각은 폼이 읽는 것처럼 사용자의 시간대로 읽힙니다. choice는 값 또는 레이블로 매칭됩니다.
- 제한: 파일은 최대 8 MB이고, 아카이브는 최대 1000개 부분에서 최대 64 MB로 압축 해제될 수 있으며, 아무것도 압축 해제하기 전에 아카이브가 선언한 크기로 검사합니다. 셀은 하나씩 읽고 읽기는 행 제한에서 멈추므로, 백만 행의 시트도 제한 이상의 비용이 들지 않습니다. 통합 문서가 아니거나, 비밀번호로 보호되었거나, 잘린 파일은 명확한 오류를 받습니다.
- 첫 번째 워크시트만 읽습니다. 수식은 캐시된 값으로 계산됩니다.

## 백그라운드 작업

```rust
App::new("Billing")
    .job_concurrency(2)
    .job("send_reminders", Schedule::every(Duration::from_secs(3600)).retry(3, Duration::from_secs(30)), send_reminders)
    .job("rebuild_index", Schedule::manual(), rebuild_index)
    .jobs_permission("ops.jobs.read")
    .serve_until("0.0.0.0:3000", shutdown_signal())
```

- 작업은 `async fn(Ctx) -> Result<()>`입니다. 그 컨텍스트에는 앱의 서비스, 설정, 기본 언어가 있으며, 권한이 없는 ID `job:<name>`이 있습니다.
- `Schedule::every(d)`는 간격마다 실행되고, `Schedule::manual()`은 트리거될 때만 실행됩니다. action은 주입 가능한 `Jobs` 서비스로 작업을 트리거합니다. `jobs.trigger("rebuild_index")`. 실행 중에 트리거된 작업은 그 후에 한 번 더 실행되며, 작업은 동시에 두 번 실행되지 않습니다.
- `.retry(times, first_delay)`는 실패한 실행을 다시 시도하며, 대기 시간을 최대 10분까지 두 배로 늘립니다.
- 최대 `job_concurrency`개의 작업(기본 4개)이 동시에 실행됩니다.
- 상태(상태, 실행 횟수, 실패 횟수, 마지막 시작과 종료, 마지막 오류 코드)는 코드에서는 `jobs.status()`, `jobs_permission`이 있는 사용자에게는 `GET /_zeyn/jobs`입니다. 오류의 세부 정보는 서버 로그에만 남습니다.
- `App::serve_until(addr, signal)`은 신호가 완료되면 요청 수신을 중단하고, 실행 중인 요청이 끝나게 하며, 재시도 대기를 취소하고, 실행 중인 작업에 30초를 줍니다.
- 스토어가 없으면 작업은 메모리에 있습니다. 프로세스가 멈출 때 대기 중이거나 실행 중인 작업은 사라지며, 서버가 여러 대이면 각각 자신의 일정을 실행합니다.

### 내구성 있는 작업

```rust
App::new("Billing")
    .job_store(SqlJobStore::new(db))
    .queued_job("send_invoice", Schedule::manual().retry(5, Duration::from_secs(10)), send_invoice)
    .job("close_day", Schedule::every(Duration::from_secs(86_400)), close_day)

async fn send_invoice(ctx: Ctx, invoice_id: String) -> Result<()> { ... }
jobs.enqueue("send_invoice", "42").await?;
```

- `JobStore`는 실행을 보관합니다. `enqueue`, `schedule`, 리스(워커 id와 종료 시각)가 있는 `claim`, `heartbeat`, `complete`, `fail`이 있습니다. `MemoryJobStore`(최대 10,000개의 실행, 가장 오래된 실패한 실행부터 버려짐)가 기본값이고, `SqlJobStore`(기능 `postgres`, `oracle`)는 실행을 `zeyn_jobs`에 보관합니다. `create_job_tables(dialect, prefix)`가 마이그레이션용 구문을 제공합니다.
- 스토어를 설정하면 `Schedule::every` 작업은 다음 실행을 스토어에 보관하므로, 재시작해도 이동하지 않고 각 간격마다 서버 하나만 실행합니다. `trigger`도 그곳에 실행을 큐에 넣습니다.
- `queued_job` 작업은 각 실행의 페이로드를 받습니다(최대 `MAX_JOB_PAYLOAD`, 4000바이트). 일반 작업도 큐에 넣을 수 있지만 페이로드는 무시합니다. `enqueue_after`는 실행을 지연시킵니다.
- 가져가기(claim)는 이 서버가 아는 작업의 기한이 된 실행을 오래된 것부터 가져가 `JOB_LEASE`(60초) 동안 보유하며, 실행이 진행되는 동안 20초마다 갱신됩니다. `complete`와 `fail`은 워커가 여전히 그 실행을 보유하는 동안에만 실행을 변경합니다. 워커가 멈춘 실행은 리스가 끝나면 다시 가져가며, 잃어버린 시도도 횟수에 포함되므로 서버를 죽이는 실행은 결국 실패로 끝납니다.
- 실패한 시도는 시도가 남아 있는 동안 백오프(두 배씩 늘어나며 최대 `MAX_BACKOFF_MS`, 10분)만큼 기다리고, 그 후 오류 코드와 함께 `failed`로 남습니다. 일정은 다음 간격으로 넘어갑니다.
- 워커는 `JOB_POLL`(1초)마다, 그리고 같은 서버에서 `enqueue` 직후 바로 기한이 된 실행을 찾습니다. 각 claim은 빈 작업 슬롯 수만큼만 실행을 가져가므로 메모리가 큐와 함께 늘어나지 않습니다.
- PostgreSQL은 하나의 `UPDATE ... WHERE id IN (SELECT ... FOR UPDATE SKIP LOCKED) RETURNING`으로 가져갑니다. Oracle 11g 이상은 트랜잭션에서 `ROWNUM`과 `FOR UPDATE SKIP LOCKED`로 id를 선택하고, 갱신한 다음, 커밋 전에 다시 읽습니다. Oracle은 잠금 select를 정렬할 수 없으므로 그곳에서는 기한이 된 실행이 정해진 순서 없이 선택됩니다. 시각은 `BIGINT` 또는 `NUMBER(19)` 열의 Unix 밀리초입니다.
- 스토어가 있으면 같은 작업의 예약된 실행과 트리거된 실행이 겹칠 수 있습니다. 동시에 두 번 실행되면 안 되는 작업은 스스로 보호해야 합니다(예: 행 잠금).
