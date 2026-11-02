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

Only use `ForwardAuth` when every request goes through the proxy; anyone who can reach the app directly can send these headers. `require_header` checks a shared secret the proxy adds. `DevAuth::superuser(name)` gives every request full access for local development, and the server prints a warning when it is on.

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
- The Content Security Policy allows scripts, styles and images only from the app's own origin. There are no inline scripts; themes are stylesheets, never inline styles.
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

- `rate_limit` applies to every request except static assets; `write_limit` adds a stricter one for calls that change data: form submits, grid edits, row, bulk and menu actions, commands, imports, saved grid views, dock layouts and themes, API calls, MCP calls and the assistant. Refused requests get `429` with `Retry-After`.
- `X-Forwarded-For` is only believed from `trusted_proxies`.
- Page sizes, line counts, query lengths and upload sizes are capped on the server whatever the client sends.

## Sessions

`App::idle_timeout(d)` signs users out of the page after a time without input, with a warning dialog first. Actions marked `reauth` need a recent sign in (`App::reauth_within`).

## Data protection

Field masking, reasons before opening sensitive records, field level audit and read audit are covered in [Audit, masking and jobs](enterprise.md).

## The API and agents

API calls need an `Authorization` header; a browser session cookie alone never calls the API or the MCP endpoint, so they need no CSRF token. Agents get exactly the permissions of the user they act for. See [API and OpenAPI](api.md) and [MCP and AI agents](mcp.md).
