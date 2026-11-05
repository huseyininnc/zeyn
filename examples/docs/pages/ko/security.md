# 보안

Zeyn의 보안은 보호하는 코드 옆에 선언되고 모든 호출마다 서버에서 검사됩니다. 기본값은 엄격하며, 이를 완화하려면 `app.rs`에 눈에 보이는 줄을 추가해야 합니다.

## 권한

모든 action과 Screen은 `permission = "feature.resource.verb"` 또는 `public`을 선언합니다. 선언이 없으면 컴파일되지 않습니다. 요청의 ID는 자신의 권한을 가지고 있습니다.

```rust
Identity::user("ada").with_permissions(["stock.product.read", "stock.product.write"])
```

사용자가 실행할 수 없는 메뉴 항목, 툴바 버튼, 행 action, 명령은 HTML에서 제외됩니다. 이를 제외하는 것은 편의 기능이며, 서버는 호출할 때마다 권한을 다시 검사합니다.

## 로그인

앱은 비밀번호를 보관하지 않습니다. `Authenticator`가 누가 호출하는지 알아내며, 보통 앱 앞단의 싱글 사인온 프록시에서 알아냅니다.

```rust
App::new(t!("app.name")).auth(
    ForwardAuth::new("x-forwarded-user")
        .groups("x-forwarded-groups")
        .require_header("x-proxy-secret", secret.expose())
        .role("stock-admins", ["stock.product.read", "stock.product.write"]),
)
```

모든 요청이 프록시를 거치는 경우에만 `ForwardAuth`를 사용하십시오. 앱에 직접 접근할 수 있는 사람은 누구나 이 헤더를 보낼 수 있습니다. `require_header`는 프록시가 추가하는 공유 시크릿을 검사합니다. `DevAuth::superuser(name)`은 로컬 개발을 위해 모든 요청에 전체 접근 권한을 주며, 켜져 있으면 서버가 경고를 출력합니다. 릴리스 빌드는 이것이 켜져 있으면 시작을 거부합니다(`App::check`가 이를 보고합니다). 릴리스에서 실행해야 하는 데모는 `App::allow_dev_auth()`를 호출할 수 있습니다. `zeyn new`로 만든 앱은 디버그 빌드에서만 이를 켭니다.

## 가드

가드는 Feature의 어떤 Screen이 열리기 전에, 그리고 그 그리드, 폼, 명령이 호출되기 전에 실행됩니다.

```rust
fn warehouse_open(ctx: &Ctx) -> GuardResult {
    if ctx.identity().permissions.contains("stock.warehouse.read") { GuardResult::Allow } else { GuardResult::Deny(t!("stock.closed")) }
}

Feature::new("stock").guard(SignedIn).guard(warehouse_open)
```

내장 가드는 `SignedIn`, `TenantRequired`, `Licensed`입니다.

## 인터셉터

인터셉터는 접근 권한 검사 이후 모든 Screen, 그리드, 폼, API, 명령 호출의 앞뒤에서 실행됩니다. `before`는 호출을 중단할 수 있고, `after`는 호출이 어떻게 끝났는지 봅니다. `AuditLog(sink)`는 누가 무엇을 했는지 기록하는 내장 인터셉터입니다.

## CSRF, CSP, 헤더

- 모든 페이지는 무작위 `zeyn_csrf` 쿠키(`HttpOnly`, `SameSite=Strict`)를 설정하고 같은 토큰을 meta 태그에 넣습니다. 런타임의 모든 호출은 이를 `x-zeyn-csrf` 헤더로 보내며, 서버는 둘을 일정한 시간으로 비교합니다.
- 세션 없이 열린 공개 Screen은 예외입니다. 쿠키를 설정하지 않고 토큰도 담지 않으므로 캐시할 수 있습니다. 런타임은 첫 호출 전에 `GET /_zeyn/csrf`에서 토큰을 가져오며, 모든 `POST`는 여전히 같은 방식으로 검사됩니다([공개 페이지](public-pages.md) 참고).
- Content Security Policy는 스크립트, 스타일, 이미지를 앱 자체의 출처에서만 허용합니다. 인라인 스크립트는 없으며, 테마는 스타일시트이고 인라인 스타일이 아닙니다.
- `App::csp`는 지정한 호스트에 대해 지시문을 하나씩 정책에서 엽니다. 헤더는 시작할 때 한 번 만들어집니다.

```rust
App::new("Shop").csp(|c| {
    c.img_src(["https://images.example.com"])
        .frame_src(["https://www.youtube-nocookie.com", "https://*.vimeo.com"])
        .media_src(["https://media.example.com:8443"])
        .media_blob()
        .connect_src(["https://api.example.com"])
        .font_src(["https://fonts.example.com"])
})
```

  `https://host`, `https://host:port`, `https://*.host`만 허용됩니다(와일드카드는 서브도메인과 일치하며 호스트 자체와는 일치하지 않습니다). `.media_blob()`은 스트리밍 라이브러리를 위해 `media-src`에 `blob:`을 추가합니다. `App::check`는 `*`, `http:`, `data:`나 `https:` 같은 스킴만 있는 값, `'unsafe-inline'`이나 `'unsafe-eval'` 같은 따옴표 붙은 키워드, 경로, 사용자 이름, 공백, 쉼표, 세미콜론이 있는 값을 거부하며, 각 문제는 대신 무엇을 써야 하는지 알려 줍니다. `script-src`, `object-src`, `base-uri`, `frame-ancestors`는 변경할 수 없습니다. `Embed`와 `Image`는 같은 목록을 읽으므로 브라우저가 차단할 호스트는 렌더링되지 않습니다([표시](display.md) 참고).
- 응답에는 `X-Content-Type-Options: nosniff`, `Referrer-Policy: same-origin`, `X-Frame-Options: DENY`가 붙습니다.
- `App::secure_cookies(true)`는 쿠키를 `Secure`로 표시합니다. HTTPS 뒤에서 켜십시오.

## 이스케이프

HTML 안의 사용자 데이터는 항상 `render::escape`를 거칩니다. 컴포넌트는 렌더링하는 모든 값을 이스케이프하므로 `<script>`라는 이름의 제품은 텍스트로 표시됩니다.

## 제한

```rust
App::new(t!("app.name"))
    .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
    .write_limit(RateLimit::per_user(60, Duration::from_secs(60)))
    .trusted_proxies(["10.0.0.0/8"])
    .allow_only_ips(["10.0.0.0/8", "192.168.0.0/16"])
    .deny_ips(["203.0.113.7"])
```

- `rate_limit`은 정적 에셋과 공개 미디어 소스의 읽기(`Feature::public_media`)를 제외한 모든 요청에 적용됩니다. `write_limit`은 데이터를 변경하는 호출에 더 엄격한 제한을 추가합니다. 폼 제출, 그리드 편집, 행, 일괄, 메뉴 action, 명령, 가져오기, 저장된 그리드 뷰, 도크 레이아웃, 테마, API 호출, MCP 호출, 어시스턴트가 이에 해당합니다. 거부된 요청은 `Retry-After`와 함께 `429`를 받습니다.
- `X-Forwarded-For`는 `trusted_proxies`에서 온 경우에만 신뢰합니다. 헤더의 모든 줄이 순서대로 계산되며, 선택되는 주소는 신뢰된 프록시가 아닌 마지막 주소입니다.
- 공개 보기, 아일랜드 및 데이터 소스 호출도 쓰기로 계산되며, 내보내기와 인쇄된 문서(`GET /_zeyn/grid/export`, `GET /_zeyn/pdf`)도 마찬가지입니다.
- 페이지 크기, 라인 수, 쿼리 길이, 업로드 크기는 클라이언트가 무엇을 보내든 서버에서 제한됩니다.

## 세션

`App::idle_timeout(d)`는 입력 없이 시간이 지나면 먼저 경고 대화 상자를 보여 준 후 사용자를 페이지에서 로그아웃시킵니다. `reauth`로 표시된 action은 최근 로그인이 필요합니다(`App::reauth_within`).

## 데이터 보호

필드 마스킹, 민감한 레코드를 열기 전의 사유, 필드 수준 감사, 읽기 감사는 [감사, 마스킹, 작업](enterprise.md)에서 다룹니다.

## 클라이언트 환경설정

일부 작은 선택은 서버가 아니라 브라우저에 속합니다. 임베드된 콘텐츠를 불러오기 전에 사용자가 확인한 안내, 마지막으로 연 탭이 그 예입니다. Zeyn은 이를 한 곳, 즉 `zeyn.state`(`/_zeyn/state.js`, 이를 쓰는 페이지에서만 로드됨)에 보관하며 서버는 이를 읽지 않으므로 캐시된 공개 페이지는 모두에게 같게 유지됩니다. 테마와 언어처럼 HTML을 바꾸는 환경설정은 쿠키에 남습니다.

키는 종류, 수명, 목적과 함께 상수로 한 번 선언하고 앱에 등록하며 `.remember(KEY)`로 컴포넌트에 바인딩합니다. `Embed::remember`는 consent 키를, `Tabs::remember`는 choice 키를 받습니다. 일반 문자열이나 다른 종류의 키는 컴파일되지 않으며, ASCII 문자, 숫자, `-`, `_`로 이루어진 1자에서 40자를 벗어나는 이름이나 1초에서 400일(`MAX_PREF_LIFETIME`)을 벗어나는 수명은 상수를 만들 때 실패합니다. 앱이 등록하지 않은 키에 바인딩된 컴포넌트는 기억되지 않으므로 개인정보 목록은 항상 완전합니다.

```rust
pub const VIDEO_NOTICE: ClientPref<Consent> =
    ClientPref::consent("video-notice", Duration::from_secs(86_400), Purpose::Necessary);
pub const ORDER_TAB: ClientPref<Choice> =
    ClientPref::choice("order-tab", Duration::from_secs(30 * 86_400), Purpose::Preference);

App::new(t!("app.name")).client_prefs(|p| {
    p.key(VIDEO_NOTICE)
        .key(ORDER_TAB)
        .allowed_when(|headers| consent_cookie(headers) == Some("all"))
})
```

- **레코드.** 각 값은 `zeyn.v1.{key}` 아래의 `{v, exp, val}`입니다. 읽을 때마다 버전, 값(`true` 또는 최대 200자의 텍스트), 종료 시각을 확인하며, 종료 시각은 지나지 않았어야 하고 키의 수명보다 더 먼 미래여서는 안 됩니다. 직접 바꾼 경우처럼 검사에 실패한 레코드는 제거됩니다. 형식의 새 버전은 오래된 레코드를 무시합니다.
- **목적.** `Purpose::Necessary`는 다른 사이트의 콘텐츠를 불러오기 전에 안내를 확인하는 것처럼 사용자가 요청한 것을 위한 것이며, 수명 동안 보관됩니다. `Purpose::Preference`는 인터페이스 선택입니다. ePrivacy 지침(제5조 3항)은 사용자가 요청한 서비스에 꼭 필요한 경우에만 동의 없이 브라우저에 데이터를 저장하도록 허용하며, EU 개인정보 보호 당국(Article 29 Working Party, 의견 04/2012)은 인터페이스 선택을 세션 동안이나 몇 시간 동안만 필수로 간주합니다. 따라서 환경설정 값은 앱의 훅 `allowed_when(|headers| ...)`이 사용자가 동의했다고 알려 줄 때까지 `sessionStorage`에 저장됩니다. 브라우저는 페이지마다 한 번 `GET /_zeyn/prefs`를 요청하고, 응답은 캐시되지 않으며, 훅이 없으면 응답은 아니요입니다.
- **탭과 차단된 저장소.** 다른 탭은 `storage` 이벤트를 통해 변경을 따라갑니다. 저장소가 차단되면(비공개 창, 엄격한 설정) 값은 페이지 동안 메모리에 있습니다. JavaScript가 없으면 컴포넌트는 기본 상태를 보여 줍니다.
- **공개성.** `App::client_prefs_table()`은 개인정보 페이지를 위해 모든 키를 종류, 목적, 초 단위 수명과 함께 나열하며, `ClearPrefs`(또는 `zeyn.prefs.clear()`)는 이를 브라우저에서 모두 제거합니다.

## API와 에이전트

API 호출에는 `Authorization` 헤더가 필요합니다. 브라우저 세션 쿠키만으로는 API나 MCP 엔드포인트를 호출할 수 없으므로 CSRF 토큰이 필요하지 않습니다. 에이전트는 자신이 대행하는 사용자의 권한만 정확히 가집니다. [API와 OpenAPI](api.md)와 [MCP와 AI 에이전트](mcp.md)를 참고하십시오.
