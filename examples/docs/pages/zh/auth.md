# 身份认证

谁在调用，以及应用对此有多大把握。Zeyn 有四种接入方式：

| 方式 | 适用场景 | Cargo feature |
| --- | --- | --- |
| `ForwardAuth` | 应用前面的单点登录代理设置请求头 | 默认 |
| `JwtAuth` | 使用 `Authorization: Bearer <JWT>` 的 API、MCP 和服务调用 | `oidc` |
| `OidcAuth` | 浏览器用户在 OpenID Connect 提供方处登录 | `oidc` |
| `Totp` | 为自身登录没有第二因素的应用提供第二因素 | `totp` |
| `App::accounts` | 应用自行保存用户：密码、双因素、通行密钥（`docs/accounts.md`） | `accounts` |

`RequireMfa`（始终可用）在某个 feature、界面或 action 上要求第二因素。
默认构建不包含 `oidc` 和 `totp`，因此不使用它们的应用无需为此付出任何代价。

```toml
zeyn = { version = "0.0.10", features = ["oidc"] }
```

已测试的提供方是 Keycloak 和 Authentik。任何遵循 OpenID Connect discovery、带 PKCE 的授权码流程以及 JWKS 的提供方，都应该可以使用。

## 用于 API 和 MCP 的 Bearer 令牌

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

- 第一个参数是签发者，必须与令牌中的 `iss` 完全一致。签名密钥来自 `<issuer>/.well-known/openid-configuration` 中指明的 JWKS；当签发者没有 discovery 文档时，用 `jwks_url` 设置地址。
- 第二个参数是令牌必须在 `aud` 中写明的受众。在 Keycloak 中，请为请求该 API 令牌的客户端添加一个 "Audience" 映射器。
- 除非 `algorithms([...])` 另行指定（`RS384`、`RS512`、`PS256`、`PS384`、`PS512`、`ES256`、`ES384`、`EdDSA`），否则只接受 `RS256`。`none` 和所有 `HS*` 算法始终被拒绝：无法允许它们。密钥只用于与其类型相对应的算法族，因此用公钥以 HMAC “签名”的令牌会被拒绝。
- `exp` 是必需的。`exp`、`nbf` 和 `iat` 允许一分钟的时间偏差（`leeway(...)`，最多五分钟）。
- 密钥缓存一小时（`key_ttl`）。如果令牌指向未知的 `kid`，会重新获取密钥，但最多每 30 秒一次（`min_key_refresh`），因此伪造密钥 id 的令牌无法冲垮提供方。最多保留 16 个密钥，超过 256 KB 的响应会被拒绝。获取失败时保留旧密钥。
- 被拒绝的令牌只是不登录；原因只在 debug 级别记录日志。

### 租户

每个认证器都会填充 `Identity` 的同样两个字段：`tenant`（用户自己的租户）和 `tenants`（用户可以在其中工作的所有其他租户）。对于二者之一，`Identity::member_of(id)` 都返回 true；超级用户（`DevAuth`）属于每一个租户。

| 认证器 | 租户的来源 |
| --- | --- |
| `JwtAuth`、`OidcAuth` | 用 `ClaimMap::tenant` 指定的 claim，字符串或列表 |
| `accounts` | 用户的 `tenant` 类型 claim（`Claim::new("tenant", "acme")`），按添加顺序 |
| `ForwardAuth` | 用 `ForwardAuth::tenant` 指定的请求头，一个租户 |
| 你自己的 `Authenticator` | 设置 `identity.tenant` 和 `identity.tenants` |

使用 `App::tenancy(resolver)` 时，请求的租户来自请求本身（主机或请求头）。已登录的用户必须属于该租户；否则请求将以未登录状态运行，因此公开界面仍然可用，其余一切都按匿名用户的方式响应。没有任何租户的已登录用户不属于任何租户。当解析器找不到租户时，使用用户自己的租户。

### Claims

`ClaimMap` 把 claims 转换为 `Identity`：

| 方法 | 默认值 | 说明 |
| --- | --- | --- |
| `user("email")` | `preferred_username`，否则 `sub` | 没有它的令牌会被拒绝 |
| `tenant("tenant")` | 无 | 用户的租户；列表（数组或以空格分隔）列出用户所属的每个租户，第一个是用户自己的。见下方“租户” |
| `roles("realm_access.roles")` | 无 | 点分路径；Keycloak 的 realm 角色，或 Authentik 中的 `groups` |
| `role("clerks", [...])` | 无 | 角色持有者或组成员的权限 |
| `permissions("scope")` | 无 | 其值即为权限的 claim；列表或以空格分隔的字符串 |
| `time_zone("zoneinfo")` | `zoneinfo` | 用户的时区 |
| `mfa_acr(["2"])` | 无 | 表示第二因素的 `acr` 值；见下文 |

`OidcAuth` 读取 ID 令牌的 claims。除非客户端的 “realm roles” 映射器开启了 “Add to ID token”，否则 Keycloak 只把 realm 角色放在访问令牌中；请开启它（或添加一个带 `id.token.claim` 的 `oidc-usermodel-realm-role-mapper`）。Authentik 在使用 `profile` scope 时会把 `groups` 放进 ID 令牌。

`auth_time`（否则 `iat`）成为 `Identity::authenticated_at`，`reauth` action 会读取它。

## 浏览器登录

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

`App::oidc` 使其成为认证器、`login_url`、空闲登出地址和重新认证地址。它添加三条路由：

| 路由 | 作用 |
| --- | --- |
| `GET /_zeyn/auth/login?return_to=/path` | 发起登录并重定向到提供方 |
| `GET /_zeyn/auth/callback` | 提供方的回跳 |
| `GET /_zeyn/auth/logout` | 在本应用和提供方两处结束会话 |

请在提供方处把 `<public url>/_zeyn/auth/callback` 注册为重定向 URI，把 `<public url>/` 注册为登出后重定向 URI。公开 URL 是配置得到的，绝不取自请求的 `Host` 头。设置了 base 路径或公开路径时，这些路由会移到该路径之下。

### 流程

1. 未登录的用户打开需要权限的页面，被带到 `/_zeyn/auth/login`。
2. 服务器生成新的 `state`、`nonce` 和 PKCE verifier（各 32 个随机字节），连同 `return_to` 路径一起封入一个有效期十分钟的 cookie，并以 `code_challenge_method=S256` 重定向到提供方的授权端点。
3. 提供方带着 code 和 `state` 把用户送回。服务器打开 cookie，以常量时间比较 `state`，删除该 cookie，并用 PKCE verifier 兑换 code（设置了客户端密钥时，通过 HTTP basic 认证发送）。
4. 先用提供方的 JWKS 校验 ID 令牌的签名，然后校验 `iss`、`aud`、存在多个受众时的 `azp`、`exp`、`nbf`、`iat` 以及 `nonce`。
5. 设置会话 cookie，用户回到 `return_to`。这里只接受本地路径；其他一律去往应用的起始页。

### 会话 cookie

服务器上没有会话存储。会话保存在一个 cookie 中，即 `zeyn_session`：

- 使用 AES-256-GCM 加密并认证。密钥由会话密钥派生（`Config::secret`，至少 32 字节；更短的会在启动时使应用停止）。cookie 名称被绑定进去，因此为某个 cookie 封装的值永远不会被当作另一个 cookie 打开。被改动一个字节的，或用另一把密钥封装的 cookie，只会被视为未登录。
- 它包含用户、租户、时区、`ClaimMap` 映射出的角色、直接权限、登录时间、是否使用过第二因素、会话结束时间，以及在使用刷新令牌时的刷新令牌。不包含 ID 令牌或访问令牌。
- `HttpOnly`、`SameSite=Lax`（提供方的回跳是跨站导航），并且在公开 URL 为 `https://`（或 `App::secure_cookies(true)`）时带 `Secure`。
- 登录时，大于 3800 字节的会话会被拒绝并记录日志，绝不会被截断。请映射更少的角色，或关闭刷新令牌。
- 它持续 `session_lifetime`（未设置时为八小时）。更换会话密钥会让所有人登出。

CSRF 保护不变：无论由哪个 cookie 让用户登录，运行时的每个 `POST` 仍然需要 `x-zeyn-csrf` 头中的 `zeyn_csrf` 令牌。

### 刷新令牌

`refresh_tokens(true)` 把刷新令牌保存在 cookie 中。令牌过期时，下一个请求会在运行前刷新它们，并把更新后的 cookie 送回。刷新被拒绝（用户已被禁用或已在提供方处登出）时会结束会话。没有刷新令牌时，无论提供方处发生什么，会话都持续 `session_lifetime`。刷新作为覆盖整个应用的一个层运行，因此每条路由都在其覆盖范围内，可选 feature 的路由也不例外。

如果提供方轮换刷新令牌并立即撤销旧令牌，则同一时刻刷新的两个请求可能让用户登出。请关闭轮换，或允许短时间内重用。

### 登出

`/_zeyn/auth/logout` 删除会话 cookie，并带着 `client_id` 和 `post_logout_redirect_uri` 把用户送到提供方的 `end_session_endpoint`。没有该端点时，用户会回到应用的起始页。

### 同一应用的 API 和 MCP 客户端

`OidcAuth::accept_bearer("stock-api")` 还接受来自同一签发者、面向该受众的 bearer 访问令牌，claim 映射相同。带有 `Authorization: Bearer` 头的请求只根据它的令牌来判断。

## 第二因素与升级验证（step up）

当用户证明了第二因素时，`Identity::mfa` 为 true：

- 对于 `OidcAuth` 和 `JwtAuth`：当 `amr` 包含 `mfa`、`otp`、`hwk`、`swk`、`sc`、`sms`、生物特征或其他第二因素（RFC 8176），或者 `acr` 是 `ClaimMap::mfa_acr` 之一；
- 对于 `Totp`：在验证通过一个验证码之后。

`RequireMfa` 要求它：

```rust
Feature::new("payroll").guard(RequireMfa::new())

App::new("Back office").interceptor(RequireMfa::new().screen("bank_accounts").action("approve_payment"))
```

作为 feature guard 时，它覆盖整个 feature。作为拦截器时，它覆盖所列出的界面和 action；当两个列表都为空时，覆盖每一次调用。

缺少该因素时，页面加载会带着 `return_to` 被重定向到升级验证地址。其他调用返回 `401`，code 为 `mfa_required`（在 API 上问题类型为 `urn:zeyn:problem:mfa_required`）；运行时会在新标签页中打开升级验证页面，并在用户回来时重复该请求，与 `reauth_required` 的处理方式相同。

对于 `OidcAuth`，升级验证地址是 `/_zeyn/auth/login?mfa=1`。它会发送 `acr_values`（即 `mfa_acr` 的值）以及 `prompt=login` 和 `max_age=0`。如果新的 ID 令牌仍然没有显示第二因素，回调会拒绝它，而不是陷入循环。

- **Keycloak**：为浏览器流程设置认证级别（在级别 2 处加入条件 OTP 或 WebAuthn 步骤），并使用 `mfa_acr(["2"])`。Keycloak 在 `acr` 中返回所达到的级别；当客户端把名称映射到级别（`acr.loa.map`）时，则返回其名称，此时请列出这些名称。
- **Authentik**：当流程使用了相应阶段时，Authentik 通过 `amr` 报告用户的登录方式。请为该应用使用带有认证器验证阶段的流程。

`OidcAuth` 之下的 `reauth` action 会转到 `/_zeyn/auth/login?reauth=1`，它发送 `prompt=login` 和 `max_age=0`，因此新的 `auth_time` 是最新的。

## 内置 TOTP

适用于登录没有第二因素的应用（没有 MFA 的代理、应用专用的认证器）：

```rust
use zeyn::{Totp, TotpStore};

App::new("Back office")
    .auth(ForwardAuth::new("x-forwarded-user"))
    .totp(Totp::new(TotpTable::new(pool), config.secret("auth.totp_key")?).issuer("Back office"))
```

- `/_zeyn/auth/totp` 向没有第二因素的用户展示注册页面：在服务器上绘制为 SVG 的二维码（没有脚本，没有图片请求）、可手动输入的密钥，以及一个验证码输入框。密钥在一个封装的 cookie 中等待十分钟，直到有验证码证明应用已经掌握它。
- 注册之后，用户会看到十个恢复码，且只显示一次。存储只保存它们的带密钥哈希（使用由 TOTP 密钥派生的密钥做 HMAC-SHA256），因此仅凭存储的副本无法离线猜出恢复码。每个恢复码只能使用一次。
- 已注册的用户在那里用验证码或恢复码验证。一个验证码只被接受一次（会保存最后使用的时间步），两个方向各允许一步的时钟偏差。
- 尝试次数按用户限制（除非 `attempts` 另行指定，否则每五分钟五次），通过应用的速率存储实现，因此多台服务器使用 `RedisStore` 共享计数。
- 验证通过的验证码会设置一个有效期为 `lifetime`（未设置时为十二小时）的封装 `zeyn_mfa` cookie，它为身份标记 `mfa`，并被视为 `reauth` action 的一次新登录。
- 已注册的因素不能仅凭一个会话来替换；重置它是管理员在存储中完成的任务。
- 表单以 `zeyn_csrf` 令牌作为表单字段，提交到同源地址。

`TotpStore` 是两个异步方法，`load(user)` 和 `save(user, record)`；`MemoryTotpStore` 用于测试。请保持 TOTP 密钥稳定：用另一把密钥封装的记录将无法再打开。

## 测试

`crates/zeyn/tests/oidc.rs` 针对测试内的一个小型签发者运行每个流程，该签发者使用为本次运行生成的密钥签名；并检查以下情况会被拒绝：过期的令牌、错误的受众和签发者、`alg=none`、以公钥为密钥的 HMAC 令牌、被改动的 claims、被篡改的 cookie、被重用或不匹配的 `state`，以及错误的 `nonce`。`crates/zeyn/tests/totp.rs` 涵盖注册、验证、重放、恢复码和速率限制。

在开发模式下的 Keycloak 上，同样的流程也可以针对它运行：

```
docker run --rm -p 8080:8080 -e KC_BOOTSTRAP_ADMIN_USERNAME=admin -e KC_BOOTSTRAP_ADMIN_PASSWORD=admin quay.io/keycloak/keycloak start-dev
ZEYN_TEST_OIDC=http://127.0.0.1:8080 cargo test -p zeyn --features oidc --test oidc
```

测试会创建一个带有自己的客户端和用户的 `zeyn-test` realm，并在结束时删除它。没有 `ZEYN_TEST_OIDC` 时会被跳过。
