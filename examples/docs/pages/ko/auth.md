# 인증

누가 호출하는지, 그리고 앱이 그것을 얼마나 확신하는지를 다룹니다. Zeyn에는 네 가지 진입 방법이 있습니다.

| 방법 | 용도 | Cargo 기능 |
| --- | --- | --- |
| `ForwardAuth` | 앱 앞단의 싱글 사인온 프록시가 헤더를 설정합니다 | 기본 |
| `JwtAuth` | `Authorization: Bearer <JWT>`를 사용하는 API, MCP, 서비스 호출 | `oidc` |
| `OidcAuth` | 브라우저 사용자가 OpenID Connect 제공자에서 로그인합니다 | `oidc` |
| `Totp` | 로그인에 2단계 인증이 없는 앱을 위한 두 번째 요소 | `totp` |
| `App::accounts` | 앱이 자체 사용자를 관리합니다. 비밀번호, 2단계 인증, 패스키 (`docs/accounts.md`) | `accounts` |

`RequireMfa`(항상 사용 가능)는 Feature, Screen, action에 두 번째 요소를 요구합니다. 기본 빌드에는 `oidc`와 `totp`가 없으므로 이를 사용하지 않는 앱은 아무 비용도 치르지 않습니다.

```toml
zeyn = { version = "0.0.9", features = ["oidc"] }
```

테스트한 제공자는 Keycloak과 Authentik입니다. OpenID Connect 디스커버리, PKCE를 사용하는 인가 코드 흐름, JWKS를 따르는 모든 제공자가 동작해야 합니다.

## API와 MCP를 위한 Bearer 토큰

```rust
use zeyn::{ClaimMap, JwtAuth};

App::new("Stock").auth(
    JwtAuth::new("https://sso.example.com/realms/main", "stock-api")
        .claims(
            ClaimMap::new()
                .roles("realm_access.roles")
                .role("stock-admins", ["stock.product.read", "stock.product.write"]),
        ),
)
```

- 첫 번째 인수는 토큰의 `iss`에 나타나는 그대로의 발급자입니다. 서명 키는 `<issuer>/.well-known/openid-configuration`에 지정된 JWKS에서 가져오며, 발급자에 디스커버리 문서가 없으면 `jwks_url`이 주소를 지정합니다.
- 두 번째 인수는 토큰이 `aud`에 명시해야 하는 대상(audience)입니다. Keycloak에서는 API용 토큰을 요청하는 클라이언트에 "Audience" 매퍼를 추가하십시오.
- `algorithms([...])`가 더 지정하지 않는 한 `RS256`만 허용됩니다(`RS384`, `RS512`, `PS256`, `PS384`, `PS512`, `ES256`, `ES384`, `EdDSA`). `none`과 모든 `HS*` 알고리즘은 항상 거부되며, 이를 허용할 방법은 없습니다. 키는 자신의 타입에 해당하는 알고리즘 계열에만 사용되므로, 공개 키로 HMAC "서명"을 한 토큰은 거부됩니다.
- `exp`는 필수입니다. `exp`, `nbf`, `iat`에는 1분의 여유가 주어집니다(`leeway(...)`, 최대 5분).
- 키는 한 시간 동안 캐시됩니다(`key_ttl`). 알 수 없는 `kid`를 가진 토큰은 키를 다시 가져오며, 30초에 한 번 이하로 제한되므로(`min_key_refresh`) 지어낸 키 id를 가진 토큰이 제공자를 범람시킬 수 없습니다. 키는 최대 16개까지 보관되고 256 KB를 넘는 응답은 거부됩니다. 가져오기에 실패하면 이전 키를 유지합니다.
- 거부된 토큰은 그냥 로그인되지 않은 것으로 처리되며, 이유는 debug 수준으로만 기록됩니다.

### 테넌트

모든 인증기는 `Identity`의 같은 두 필드를 채웁니다. `tenant`(사용자 자신의 테넌트)와 `tenants`(사용자가 작업할 수 있는 그 밖의 모든 테넌트)입니다. `Identity::member_of(id)`는 둘 중 어느 쪽이든 true이며, 슈퍼유저(`DevAuth`)는 모든 테넌트에 속합니다.

| 인증기 | 테넌트의 출처 |
| --- | --- |
| `JwtAuth`, `OidcAuth` | `ClaimMap::tenant`로 지정한 클레임, 문자열 또는 목록 |
| `accounts` | 사용자의 `tenant` 유형 클레임(`Claim::new("tenant", "acme")`), 추가된 순서대로 |
| `ForwardAuth` | `ForwardAuth::tenant`로 지정한 헤더, 테넌트 하나 |
| 직접 만든 `Authenticator` | `identity.tenant`와 `identity.tenants`를 설정합니다 |

`App::tenancy(resolver)`를 사용하면 요청의 테넌트는 요청(호스트 또는 헤더)에서 가져옵니다. 로그인한 사용자는 그 테넌트에 속해야 하며, 그렇지 않으면 요청이 로그아웃 상태로 실행되므로 공개 Screen은 여전히 동작하고 그 밖의 모든 것은 익명 사용자처럼 응답합니다. 테넌트가 전혀 없는 로그인한 사용자는 어느 테넌트에도 속하지 않습니다. 리졸버가 테넌트를 찾지 못하면 사용자 자신의 테넌트가 적용됩니다.

### 클레임

`ClaimMap`은 클레임을 `Identity`로 바꿉니다.

| 메서드 | 기본값 | 비고 |
| --- | --- | --- |
| `user("email")` | `preferred_username`, 없으면 `sub` | 이것이 없는 토큰은 거부됩니다 |
| `tenant("tenant")` | 없음 | 사용자의 테넌트. 목록(배열 또는 공백으로 구분)은 사용자가 속한 모든 테넌트를 지정하며 첫 번째가 사용자 자신의 것입니다. 아래 "테넌트"를 참고하십시오 |
| `roles("realm_access.roles")` | 없음 | 점으로 구분한 경로. Keycloak 렐름 역할, 또는 Authentik의 `groups` |
| `role("clerks", [...])` | 없음 | 역할 보유자 또는 그룹 구성원의 권한 |
| `permissions("scope")` | 없음 | 값이 권한인 클레임. 목록 또는 공백으로 구분한 문자열 |
| `time_zone("zoneinfo")` | `zoneinfo` | 사용자의 시간대 |
| `mfa_acr(["2"])` | 없음 | 두 번째 요소를 뜻하는 `acr` 값. 아래를 참고하십시오 |

`OidcAuth`는 ID 토큰의 클레임을 읽습니다. Keycloak은 클라이언트의 "realm roles" 매퍼에서 "Add to ID token"을 켜지 않는 한 렐름 역할을 액세스 토큰에만 넣습니다. 이를 켜십시오(또는 `id.token.claim`이 있는 `oidc-usermodel-realm-role-mapper`를 추가). Authentik은 `profile` 스코프와 함께 `groups`를 ID 토큰에 넣습니다.

`auth_time`(없으면 `iat`)은 `reauth` action이 읽는 `Identity::authenticated_at`이 됩니다.

## 브라우저 로그인

```rust
use zeyn::{ClaimMap, OidcAuth};

let oidc = OidcAuth::new(
    "https://sso.example.com/realms/main",
    "stock-web",
    "https://stock.example.com",
    config.secret("auth.session_key")?,
)
.client_secret(config.secret("auth.client_secret")?)
.claims(ClaimMap::new().roles("realm_access.roles").role("stock-admins", ["stock.product.write"]));

App::new("Stock").oidc(oidc)
```

`App::oidc`는 이를 인증기, `login_url`, 유휴 로그아웃 주소, 재인증 주소로 만듭니다. 경로 세 개를 추가합니다.

| 경로 | 기능 |
| --- | --- |
| `GET /_zeyn/auth/login?return_to=/path` | 로그인을 시작하고 제공자로 리다이렉트합니다 |
| `GET /_zeyn/auth/callback` | 제공자가 되돌려 보내는 리다이렉트 |
| `GET /_zeyn/auth/logout` | 여기와 제공자에서 세션을 종료합니다 |

제공자에서 `<public url>/_zeyn/auth/callback`을 리다이렉트 URI로, `<public url>/`을 로그아웃 후 리다이렉트 URI로 등록하십시오. 공개 URL은 설정으로 지정하며 요청의 `Host` 헤더에서 가져오지 않습니다. 기본 경로나 공개 경로가 있으면 경로는 그 아래로 이동합니다.

### 흐름

1. 로그아웃 상태의 사용자가 권한이 필요한 페이지를 열면 `/_zeyn/auth/login`으로 이동합니다.
2. 서버는 새로운 `state`, `nonce`, PKCE 검증자(각각 무작위 32바이트)를 만들고, `return_to` 경로와 함께 10분 동안 유지되는 쿠키에 봉인한 뒤 `code_challenge_method=S256`으로 제공자의 인가 엔드포인트로 리다이렉트합니다.
3. 제공자는 코드와 `state`와 함께 사용자를 돌려보냅니다. 서버는 쿠키를 열고, `state`를 일정한 시간으로 비교하고, 쿠키를 삭제하며, PKCE 검증자로 코드를 교환합니다(클라이언트 시크릿이 설정되어 있으면 HTTP 기본 인증으로 전달).
4. ID 토큰의 서명을 제공자의 JWKS로 검사한 다음, `iss`, `aud`, 대상이 여럿이면 `azp`, `exp`, `nbf`, `iat`, `nonce`를 검사합니다.
5. 세션 쿠키가 설정되고 사용자는 `return_to`로 돌아갑니다. 여기에는 로컬 경로만 허용되며, 그 밖의 것은 앱의 시작 페이지로 갑니다.

### 세션 쿠키

서버에는 세션 스토어가 없습니다. 세션은 쿠키 하나 `zeyn_session`에 있습니다.

- AES-256-GCM으로 암호화되고 인증됩니다. 키는 세션 키(`Config::secret`, 최소 32바이트이며 더 짧으면 앱이 시작할 때 멈춥니다)에서 파생됩니다. 쿠키 이름이 함께 묶여 있으므로 한 쿠키용으로 봉인된 값은 다른 쿠키로 열리지 않습니다. 바이트가 하나라도 바뀌었거나 다른 키로 봉인된 쿠키는 그냥 로그인되지 않은 것으로 처리됩니다.
- 사용자, 테넌트, 시간대, `ClaimMap`이 매핑한 역할, 직접 권한, 로그인 시각, 두 번째 요소 사용 여부, 세션 종료 시각, 그리고 리프레시 토큰이 있으면 리프레시 토큰을 담습니다. ID 토큰이나 액세스 토큰은 담지 않습니다.
- `HttpOnly`, `SameSite=Lax`(제공자가 돌려보내는 리다이렉트는 교차 사이트 탐색입니다)이며, 공개 URL이 `https://`이면(또는 `App::secure_cookies(true)`이면) `Secure`입니다.
- 3800바이트보다 커질 세션은 로그인할 때 로그 메시지와 함께 거부되며 절대 잘리지 않습니다. 매핑하는 역할을 줄이거나 리프레시 토큰을 끄십시오.
- `session_lifetime`(설정하지 않으면 여덟 시간) 동안 유지됩니다. 세션 키를 바꾸면 모든 사람이 로그아웃됩니다.

CSRF 보호는 바뀌지 않습니다. 어떤 쿠키가 사용자를 로그인시키든 런타임의 모든 `POST`에는 `x-zeyn-csrf` 헤더의 `zeyn_csrf` 토큰이 필요합니다.

### 리프레시 토큰

`refresh_tokens(true)`는 리프레시 토큰을 쿠키에 보관합니다. 토큰이 만료되면 다음 요청이 실행되기 전에 이를 갱신하고 갱신된 쿠키를 돌려보냅니다. 갱신이 거부되면(제공자에서 사용자가 비활성화되었거나 로그아웃됨) 세션이 종료됩니다. 리프레시 토큰이 없으면 제공자에서 무슨 일이 일어나든 세션은 `session_lifetime` 동안 유지됩니다. 갱신은 앱 전체에 걸친 하나의 레이어로 실행되므로 선택적 기능의 경로를 포함한 모든 경로가 대상입니다.

제공자가 리프레시 토큰을 순환시키면서 이전 것을 즉시 폐기하면, 같은 순간에 갱신하는 두 요청이 사용자를 로그아웃시킬 수 있습니다. 순환을 끄거나 짧은 시간 동안 재사용을 허용하십시오.

### 로그아웃

`/_zeyn/auth/logout`은 세션 쿠키를 삭제하고 `client_id`와 `post_logout_redirect_uri`와 함께 사용자를 제공자의 `end_session_endpoint`로 보냅니다. 그 엔드포인트가 없으면 사용자는 앱의 시작 페이지로 갑니다.

### 같은 앱의 API 및 MCP 클라이언트

`OidcAuth::accept_bearer("stock-api")`는 같은 발급자가 그 대상에 대해 발급한 bearer 액세스 토큰도 같은 클레임 매핑으로 받아들입니다. `Authorization: Bearer` 헤더가 있는 요청은 그 토큰만으로 판단합니다.

## 두 번째 요소와 단계 상승

`Identity::mfa`는 사용자가 두 번째 요소를 증명했을 때 true입니다.

- `OidcAuth`와 `JwtAuth`에서는 `amr`이 `mfa`, `otp`, `hwk`, `swk`, `sc`, `sms`, 생체 인식 또는 다른 두 번째 요소(RFC 8176)를 명시하거나, `acr`이 `ClaimMap::mfa_acr` 중 하나일 때,
- `Totp`에서는 코드를 검증한 후입니다.

`RequireMfa`가 이를 요구합니다.

```rust
Feature::new("payroll").guard(RequireMfa::new())

App::new("Back office").interceptor(RequireMfa::new().screen("bank_accounts").action("approve_payment"))
```

Feature 가드로 사용하면 Feature 전체를 대상으로 합니다. 인터셉터로 사용하면 나열된 Screen과 action을, 두 목록이 모두 비어 있으면 모든 호출을 대상으로 합니다.

요소가 없으면 페이지 로드는 `return_to`와 함께 단계 상승 주소로 리다이렉트됩니다. 다른 호출은 코드 `mfa_required`와 함께 `401`로 응답하며(API에서는 problem 타입 `urn:zeyn:problem:mfa_required`), 런타임은 `reauth_required`처럼 새 탭에서 단계 상승 페이지를 열고 사용자가 돌아오면 요청을 반복합니다.

`OidcAuth`에서 단계 상승 주소는 `/_zeyn/auth/login?mfa=1`입니다. `prompt=login`, `max_age=0`과 함께 `acr_values`(`mfa_acr` 값)를 보냅니다. 새 ID 토큰에도 두 번째 요소가 나타나지 않으면 콜백은 반복하지 않고 이를 거부합니다.

- **Keycloak**: 브라우저 흐름에 인증 수준을 부여하고(수준 2의 조건부 OTP 또는 WebAuthn 단계) `mfa_acr(["2"])`를 사용하십시오. Keycloak은 `acr`에 도달한 수준으로 응답하거나, 클라이언트가 이름을 수준에 매핑한 경우(`acr.loa.map`) 그 이름으로 응답하므로, 그 경우에는 그 이름을 나열하십시오.
- **Authentik**: Authentik은 흐름이 해당 단계를 사용했을 때 `amr`로 사용자가 어떻게 로그인했는지 보고합니다. 애플리케이션에 인증기 검증 단계가 있는 흐름을 사용하십시오.

`OidcAuth`에서 `reauth` action은 `/_zeyn/auth/login?reauth=1`로 이동하며, 이는 `prompt=login`과 `max_age=0`을 보내므로 새 `auth_time`이 최신 상태가 됩니다.

## 내장 TOTP

로그인에 두 번째 요소가 없는 앱(MFA가 없는 프록시, 앱 전용 인증기)을 위한 기능입니다.

```rust
use zeyn::{Totp, TotpStore};

App::new("Back office")
    .auth(ForwardAuth::new("x-forwarded-user"))
    .totp(Totp::new(TotpTable::new(pool), config.secret("auth.totp_key")?).issuer("Back office"))
```

- `/_zeyn/auth/totp`는 두 번째 요소가 없는 사용자에게 등록 화면을 보여 줍니다. 서버에서 SVG로 그린 QR 코드(스크립트도 이미지 요청도 없음), 직접 입력할 키, 코드 필드가 있습니다. 시크릿은 코드가 앱이 이를 가지고 있음을 증명할 때까지 10분 동안 봉인된 쿠키에서 대기합니다.
- 등록 후 사용자는 복구 코드 열 개를 한 번만 봅니다. 스토어는 키가 있는 해시(TOTP 키에서 파생된 키를 사용하는 HMAC-SHA256)만 보관하므로, 스토어의 사본만으로는 오프라인에서 추측할 수 없습니다. 각 코드는 한 번만 동작합니다.
- 저장된 시크릿은 TOTP 키로 봉인되고 사용자에 묶여 있으므로, 스토어만으로는 코드를 만들 수 없고 레코드를 사용자 사이에서 바꿔치기할 수도 없습니다.
- 등록된 사용자는 그곳에서 코드 또는 복구 코드로 검증합니다. 코드는 한 번만 허용되며(마지막으로 사용한 시간 단계가 저장됨) 양쪽으로 한 단계의 시계 오차가 허용됩니다.
- 시도는 사용자별로 제한되며(`attempts`가 달리 지정하지 않으면 5분에 다섯 번) 앱의 속도 제한 스토어를 통하므로, 여러 서버가 `RedisStore`로 횟수를 공유합니다.
- 검증된 코드는 `lifetime`(설정하지 않으면 열두 시간) 동안 봉인된 `zeyn_mfa` 쿠키를 설정하며, 이는 ID에 `mfa`를 표시하고 `reauth` action에 대해 새로운 로그인으로 간주됩니다.
- 등록된 요소는 세션만으로 교체할 수 없습니다. 재설정은 스토어에서 하는 관리자 작업입니다.
- 폼은 `zeyn_csrf` 토큰을 폼 필드로 하여 같은 출처로 전송됩니다.

`TotpStore`는 `load(user)`와 `save(user, record)`라는 두 개의 비동기 메서드이며, `MemoryTotpStore`는 테스트용입니다. TOTP 키를 안정적으로 유지하십시오. 다른 키로 봉인된 레코드는 더 이상 열리지 않습니다.

## 테스트

`crates/zeyn/tests/oidc.rs`는 테스트 실행을 위해 만든 키로 서명하는 테스트 안의 작은 발급자를 상대로 모든 흐름을 실행하며, 만료된 토큰, 잘못된 대상과 발급자, `alg=none`, 공개 키를 키로 쓴 HMAC 토큰, 변경된 클레임, 변조된 쿠키, 재사용되었거나 일치하지 않는 `state`, 잘못된 `nonce`가 거부되는지 검사합니다. `crates/zeyn/tests/totp.rs`는 등록, 검증, 재전송, 복구 코드, 속도 제한을 다룹니다.

개발 모드의 Keycloak이 있으면 같은 흐름을 그것을 상대로도 실행합니다.

```
docker run --rm -p 8080:8080 -e KC_BOOTSTRAP_ADMIN_USERNAME=admin -e KC_BOOTSTRAP_ADMIN_PASSWORD=admin quay.io/keycloak/keycloak start-dev
ZEYN_TEST_OIDC=http://127.0.0.1:8080 cargo test -p zeyn --features oidc --test oidc
```

테스트는 자체 클라이언트와 사용자가 있는 `zeyn-test` 렐름을 만들고 마지막에 삭제합니다. `ZEYN_TEST_OIDC`가 없으면 건너뜁니다.
