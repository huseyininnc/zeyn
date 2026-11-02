# Authentication

Who is calling, and how sure the app is about it. Zeyn has four ways in:

| Way | Use it for | Cargo feature |
| --- | --- | --- |
| `ForwardAuth` | a single sign on proxy in front of the app sets headers | default |
| `JwtAuth` | APIs, MCP and service calls with `Authorization: Bearer <JWT>` | `oidc` |
| `OidcAuth` | browser users sign in at an OpenID Connect provider | `oidc` |
| `Totp` | a second factor for apps whose sign in has none | `totp` |
| `App::accounts` | the app keeps its own users: passwords, two factor, passkeys (`docs/accounts.md`) | `accounts` |

`RequireMfa` (always available) asks for a second factor on a feature, a screen or an action.
The default build has none of `oidc` and `totp`, so apps that do not use them pay nothing.

```toml
zeyn = { version = "0.0.2", features = ["oidc"] }
```

The providers tested are Keycloak and Authentik. Any provider that follows OpenID Connect
discovery, the authorization code flow with PKCE and JWKS should work.

## Bearer tokens for APIs and MCP

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

- The first argument is the issuer exactly as it appears in the tokens' `iss`. The signing keys
  come from the JWKS named in `<issuer>/.well-known/openid-configuration`; `jwks_url` sets the
  address when the issuer has no discovery document.
- The second argument is the audience the token must name in `aud`. In Keycloak, add an
  "Audience" mapper to the client that requests tokens for the API.
- Only `RS256` is accepted unless `algorithms([...])` says more (`RS384`, `RS512`, `PS256`,
  `PS384`, `PS512`, `ES256`, `ES384`, `EdDSA`). `none` and every `HS*` algorithm are refused
  always: there is no way to allow them. A key is only used for the algorithm family of its type,
  so a token "signed" with HMAC over the public key is refused.
- `exp` is required. `exp`, `nbf` and `iat` get one minute of leeway (`leeway(...)`, at most
  five minutes).
- Keys are cached for an hour (`key_ttl`). A token that names an unknown `kid` fetches the keys
  again, at most once every 30 seconds (`min_key_refresh`), so tokens with made up key ids cannot
  flood the provider. At most 16 keys are kept and responses over 256 KB are refused. A failed
  fetch keeps the old keys.
- A refused token is simply not signed in; the reason is logged at debug level only.

### Claims

`ClaimMap` turns claims into the `Identity`:

| Method | Default | Notes |
| --- | --- | --- |
| `user("email")` | `preferred_username`, else `sub` | a token without it is refused |
| `tenant("tenant")` | none | wins over the tenant resolved from the request |
| `roles("realm_access.roles")` | none | dotted path; Keycloak realm roles, or `groups` in Authentik |
| `role("clerks", [...])` | none | permissions for holders of a role or members of a group |
| `permissions("scope")` | none | claim whose values are permissions; a list or a space separated string |
| `time_zone("zoneinfo")` | `zoneinfo` | the user's zone |
| `mfa_acr(["2"])` | none | `acr` values that mean a second factor; see below |

`OidcAuth` reads the claims of the ID token. Keycloak puts realm roles only in the access token
unless the client's "realm roles" mapper has "Add to ID token" on; turn it on (or add a
`oidc-usermodel-realm-role-mapper` with `id.token.claim`). Authentik puts `groups` in the ID
token with the `profile` scope.

`auth_time` (else `iat`) becomes `Identity::authenticated_at`, which `reauth` actions read.

## Browser sign in

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

`App::oidc` makes it the authenticator, the `login_url`, the idle sign out address and the
re authentication address. It adds three routes:

| Route | What it does |
| --- | --- |
| `GET /_zeyn/auth/login?return_to=/path` | starts a sign in and redirects to the provider |
| `GET /_zeyn/auth/callback` | the provider's redirect back |
| `GET /_zeyn/auth/logout` | ends the session here and at the provider |

At the provider, register `<public url>/_zeyn/auth/callback` as the redirect URI and
`<public url>/` as the post logout redirect URI. The public URL is configured, never taken from
the request's `Host` header. With a base or public path the routes move under it.

### The flow

1. A signed out user opens a page that needs a permission and goes to `/_zeyn/auth/login`.
2. The server makes a new `state`, `nonce` and PKCE verifier (32 random bytes each), seals them
   with the `return_to` path into a cookie that lives ten minutes, and redirects to the
   provider's authorization endpoint with `code_challenge_method=S256`.
3. The provider sends the user back with a code and the `state`. The server opens the cookie,
   compares `state` in constant time, deletes the cookie, and exchanges the code with the PKCE
   verifier (client secret through HTTP basic authentication when set).
4. The ID token's signature is checked against the provider's JWKS, then `iss`, `aud`, `azp`
   when there are several audiences, `exp`, `nbf`, `iat` and the `nonce`.
5. The session cookie is set and the user goes back to `return_to`. Only a local path is
   accepted there; anything else goes to the app's start page.

### The session cookie

There is no session store on the server. The session lives in one cookie, `zeyn_session`:

- Encrypted and authenticated with AES-256-GCM. The key is derived from the session key
  (`Config::secret`, at least 32 bytes; a shorter one stops the app at startup). The cookie name
  is bound in, so a value sealed for one cookie never opens as another. A changed byte, or a cookie
  sealed with another key, is simply not signed in.
- It holds the user, tenant, time zone, the roles that `ClaimMap` maps, the direct permissions,
  the sign in time, whether a second factor was used, the session end and, with refresh tokens,
  the refresh token. Not the ID token or the access token.
- `HttpOnly`, `SameSite=Lax` (the provider's redirect back is a cross site navigation) and
  `Secure` when the public URL is `https://` (or `App::secure_cookies(true)`).
- A session that would be larger than 3800 bytes is refused at sign in with a log message, never
  cut. Map fewer roles or turn off refresh tokens.
- It lasts `session_lifetime` (eight hours unless set). Changing the session key signs everyone
  out.

CSRF protection does not change: every `POST` of the runtime still needs the `zeyn_csrf` token in
the `x-zeyn-csrf` header, whatever cookie signs the user in.

### Refresh tokens

`refresh_tokens(true)` keeps the refresh token in the cookie. When the tokens expire, the next
request refreshes them before it runs and sends the renewed cookie back. A refused refresh (the
user was disabled or signed out at the provider) ends the session. Without refresh tokens a
session lasts `session_lifetime`, whatever happens at the provider.

If the provider rotates refresh tokens and revokes the old one at once, two requests that refresh
at the same moment can sign the user out. Keep rotation off or allow reuse for a short time.

### Logout

`/_zeyn/auth/logout` deletes the session cookie and sends the user to the provider's
`end_session_endpoint` with `client_id` and `post_logout_redirect_uri`. Without that endpoint
the user goes to the app's start page.

### API and MCP clients of the same app

`OidcAuth::accept_bearer("stock-api")` also accepts bearer access tokens from the same issuer for
that audience, with the same claim mapping. A request with an `Authorization: Bearer` header is
judged by its token only.

## Second factor and step up

`Identity::mfa` is true when the user proved a second factor:

- with `OidcAuth` and `JwtAuth`, when `amr` names `mfa`, `otp`, `hwk`, `swk`, `sc`, `sms`, a
  biometric or another second factor (RFC 8176), or when `acr` is one of `ClaimMap::mfa_acr`;
- with `Totp`, after a verified code.

`RequireMfa` asks for it:

```rust
Feature::new("payroll").guard(RequireMfa::new())

App::new("Back office").interceptor(RequireMfa::new().screen("bank_accounts").action("approve_payment"))
```

As a feature guard it covers the whole feature. As an interceptor it covers the listed screens
and actions, or every call when both lists are empty.

When the factor is missing, a page load is redirected to the step up address with `return_to`.
Other calls answer `401` with code `mfa_required` (problem type `urn:zeyn:problem:mfa_required`
on the API); the runtime opens the step up page in a new tab and repeats the request when the
user comes back, as it does for `reauth_required`.

With `OidcAuth` the step up address is `/_zeyn/auth/login?mfa=1`. It sends `acr_values` (the
`mfa_acr` values) with `prompt=login` and `max_age=0`. If the new ID token still shows no second
factor, the callback refuses it instead of looping.

- **Keycloak**: give the browser flow levels of authentication (a conditional OTP or WebAuthn
  step at level 2) and use `mfa_acr(["2"])`. Keycloak answers `acr` with the level reached, or
  with its name when the client maps names to levels (`acr.loa.map`); list those names then.
- **Authentik**: Authentik reports how the user signed in through `amr` when the flow used
  those stages. Use a flow with an authenticator validation stage for the application.

`reauth` actions under `OidcAuth` go to `/_zeyn/auth/login?reauth=1`, which sends
`prompt=login` and `max_age=0`, so the new `auth_time` is fresh.

## Built in TOTP

For apps whose sign in has no second factor (a proxy without MFA, an app specific
authenticator):

```rust
use zeyn::{Totp, TotpStore};

App::new("Back office")
    .auth(ForwardAuth::new("x-forwarded-user"))
    .totp(Totp::new(TotpTable::new(pool), config.secret("auth.totp_key")?).issuer("Back office"))
```

- `/_zeyn/auth/totp` shows enrollment to a user without a second factor: a QR code drawn as SVG
  on the server (no script, no image request), the key to type by hand, and a code field. The
  secret waits in a sealed cookie for ten minutes until a code proves the app has it.
- After enrollment the user sees ten recovery codes once. The store keeps only their keyed
  hashes (HMAC-SHA256 with a key derived from the TOTP key), so a copy of the store alone cannot
  be guessed offline. Each works once.
- The stored secret is sealed with the TOTP key and bound to the user, so the store alone cannot
  make codes and records cannot be swapped between users.
- Enrolled users verify there with a code or a recovery code. A code is accepted once (the
  last used time step is stored) with one step of clock skew either way.
- Attempts are limited per user (five per five minutes unless `attempts` says otherwise) through
  the app's rate store, so several servers share the count with `RedisStore`.
- A verified code sets a sealed `zeyn_mfa` cookie for `lifetime` (twelve hours unless set) that
  marks the identity with `mfa` and counts as a fresh sign in for `reauth` actions.
- An enrolled factor cannot be replaced from a session alone; resetting it is an administrator
  task in the store.
- Forms post to the same origin with the `zeyn_csrf` token as a form field.

`TotpStore` is two async methods, `load(user)` and `save(user, record)`; `MemoryTotpStore` is for
tests. Keep the TOTP key stable: records sealed with another key no longer open.

## Testing

`crates/zeyn/tests/oidc.rs` runs every flow against a small issuer in the test that signs with a
key made for the run, and checks that expired tokens, wrong audiences and issuers, `alg=none`, an
HMAC token keyed with the public key, changed claims, tampered cookies, reused or mismatched
`state` and a wrong `nonce` are refused. `crates/zeyn/tests/totp.rs` covers enrollment,
verification, replay, recovery codes and the rate limit.

With a Keycloak in development mode, the same flows also run against it:

```
docker run --rm -p 8080:8080 -e KC_BOOTSTRAP_ADMIN_USERNAME=admin -e KC_BOOTSTRAP_ADMIN_PASSWORD=admin quay.io/keycloak/keycloak start-dev
ZEYN_TEST_OIDC=http://127.0.0.1:8080 cargo test -p zeyn --features oidc --test oidc
```

The test creates a `zeyn-test` realm with its own client and user and deletes it at the end.
Without `ZEYN_TEST_OIDC` it is skipped.
