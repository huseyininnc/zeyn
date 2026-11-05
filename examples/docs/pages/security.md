# Security

Security in Zeyn is declared next to the code it protects and checked on the server for every call. The defaults are strict; loosening one is a visible line in `app.rs`.

## Permissions

Every action and screen declares `permission = "feature.resource.verb"` or `public`. Without one it does not compile. The identity of a request carries its permissions:

```rust
Identity::user("ada").with_permissions(["stock.product.read", "stock.product.write"])
```

Menu items, toolbar buttons, row actions and commands the user may not run are left out of the HTML. Leaving them out is a convenience; the server checks the permission again on every call.

## Signing in

The app does not keep passwords. An `Authenticator` finds out who is calling, usually from a single sign on proxy in front of the app:

```rust
App::new(t!("app.name")).auth(
    ForwardAuth::new("x-forwarded-user")
        .groups("x-forwarded-groups")
        .require_header("x-proxy-secret", secret.expose())
        .role("stock-admins", ["stock.product.read", "stock.product.write"]),
)
```

Only use `ForwardAuth` when every request goes through the proxy; anyone who can reach the app directly can send these headers. `require_header` checks a shared secret the proxy adds. `DevAuth::superuser(name)` gives every request full access for local development, and the server prints a warning when it is on. A release build refuses to start with it (`App::check` reports it); a demo that must run in release can call `App::allow_dev_auth()`. Apps made with `zeyn new` turn it on only in debug builds.

## Guards

A guard runs before any screen of a feature opens and before any of its grids, forms or commands is called:

```rust
fn warehouse_open(ctx: &Ctx) -> GuardResult {
    if ctx.identity().permissions.contains("stock.warehouse.read") { GuardResult::Allow } else { GuardResult::Deny(t!("stock.closed")) }
}

Feature::new("stock").guard(SignedIn).guard(warehouse_open)
```

Built in guards: `SignedIn`, `TenantRequired` and `Licensed`.

## Interceptors

An interceptor runs around every screen, grid, form, API and command call after the access checks. `before` can stop a call; `after` sees how it ended. `AuditLog(sink)` is the built in one that records who did what.

## CSRF, CSP and headers

- Every page sets a random `zeyn_csrf` cookie (`HttpOnly`, `SameSite=Strict`) and puts the same token in a meta tag. Every call from the runtime sends it in the `x-zeyn-csrf` header and the server compares the two in constant time.
- Public screens opened without a session are the exception: they set no cookie and carry no token, so they can be cached. The runtime fetches the token from `GET /_zeyn/csrf` before its first call, and every `POST` is still checked the same way (see [Public pages](public-pages.md)).
- The Content Security Policy allows scripts, styles and images only from the app's own origin. There are no inline scripts; themes are stylesheets, never inline styles.
- `App::csp` opens the policy for named hosts, one directive at a time. The header is built once at startup.

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

  Only `https://host`, `https://host:port` and `https://*.host` are accepted (a wildcard matches subdomains, not the host itself). `.media_blob()` adds `blob:` to `media-src` for streaming libraries. `App::check` refuses `*`, `http:`, bare schemes such as `data:` or `https:`, quoted keywords such as `'unsafe-inline'` or `'unsafe-eval'`, paths, user names, and values with spaces, commas or semicolons; each problem says what to write instead. `script-src`, `object-src`, `base-uri` and `frame-ancestors` cannot be changed. `Embed` and `Image` read the same lists, so a host the browser would block is never rendered (see [Display](display.md)).
- Responses carry `X-Content-Type-Options: nosniff`, `Referrer-Policy: same-origin` and `X-Frame-Options: DENY`.
- `App::secure_cookies(true)` marks cookies `Secure`; turn it on behind HTTPS.

## Escaping

User data in HTML always goes through `render::escape`. Components escape every value they render, so a product named `<script>` shows as text.

## Limits

```rust
App::new(t!("app.name"))
    .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
    .write_limit(RateLimit::per_user(60, Duration::from_secs(60)))
    .trusted_proxies(["10.0.0.0/8"])
    .allow_only_ips(["10.0.0.0/8", "192.168.0.0/16"])
    .deny_ips(["203.0.113.7"])
```

- `rate_limit` applies to every request except static assets and reads of public media sources (`Feature::public_media`); `write_limit` adds a stricter one for calls that change data: form submits, grid edits, row, bulk and menu actions, commands, imports, saved grid views, dock layouts and themes, API calls, MCP calls and the assistant. Refused requests get `429` with `Retry-After`.
- `X-Forwarded-For` is only believed from `trusted_proxies`. Every line of the header counts, in order, and the address taken is the last one that is not a trusted proxy.
- Reveals, island and data source calls count as writes too, and so do exports and printed documents (`GET /_zeyn/grid/export`, `GET /_zeyn/pdf`).
- Page sizes, line counts, query lengths and upload sizes are capped on the server whatever the client sends.

## Sessions

`App::idle_timeout(d)` signs users out of the page after a time without input, with a warning dialog first. Actions marked `reauth` need a recent sign in (`App::reauth_within`).

## Data protection

Field masking, reasons before opening sensitive records, field level audit and read audit are covered in [Audit, masking and jobs](enterprise.md).

## Client preferences

Some small choices belong in the browser, not on the server: a notice the user confirmed before embedded content loads, the tab opened last. Zeyn keeps them in one place, `zeyn.state` (`/_zeyn/state.js`, loaded only on pages that use it), and the server never reads them, so cached public pages stay the same for everyone. Preferences that change the HTML, such as the theme and the language, stay in cookies.

A key is declared once as a constant with its kind, lifetime and purpose, registered on the app, and bound to components with `.remember(KEY)`: `Embed::remember` takes a consent key, `Tabs::remember` a choice key. A plain string or a key of the other kind does not compile, and a name outside 1 to 40 ASCII letters, digits, `-` and `_` or a lifetime outside one second to 400 days (`MAX_PREF_LIFETIME`) fails when the constant is built. A component bound to a key the app did not register is not remembered, so the privacy list is always complete.

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

- **Records.** Each value is `{v, exp, val}` under `zeyn.v1.{key}`. Every read checks the version, the value (`true` or a text of at most 200 characters) and the end time, which must not have passed and must lie no further ahead than the key's lifetime. A record that fails, such as one changed by hand, is removed. A new version of the format ignores old records.
- **Purpose.** `Purpose::Necessary` is for something the user asked for, such as confirming a notice before content from another site loads; it is kept for its lifetime. `Purpose::Preference` is an interface choice. The ePrivacy Directive (article 5(3)) allows storing data in the browser without consent only when it is strictly necessary for a service the user asked for, and the EU data protection authorities (Article 29 Working Party, opinion 04/2012) count interface choices as necessary only for the session or a few hours. So preference values go to `sessionStorage` until the app's hook, `allowed_when(|headers| ...)`, says the user consented; the browser asks `GET /_zeyn/prefs` once per page, the answer is never cached, and without a hook the answer is no.
- **Tabs and blocked storage.** Other tabs follow changes through the `storage` event. When storage is blocked (private windows, strict settings) the values live in memory for the page. Without JavaScript components show their default state.
- **Openness.** `App::client_prefs_table()` lists every key with its kind, purpose and lifetime in seconds for a privacy page, and `ClearPrefs` (or `zeyn.prefs.clear()`) removes them all from the browser.

## The API and agents

API calls need an `Authorization` header; a browser session cookie alone never calls the API or the MCP endpoint, so they need no CSRF token. Agents get exactly the permissions of the user they act for. See [API and OpenAPI](api.md) and [MCP and AI agents](mcp.md).
