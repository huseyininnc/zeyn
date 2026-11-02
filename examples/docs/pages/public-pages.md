# Public pages

Screens marked `public` can be opened by anyone, also by visitors who never signed in: a landing
page, a price list, a help center. Zeyn serves them so that they are cheap to deliver and easy for
caches and search engines to handle.

## No cookies for visitors

A public screen opened by a request without a session sets no cookie. The page carries an empty
CSRF token, so every visitor in the same language and theme gets the same bytes.

The browser runtime asks `GET /_zeyn/csrf` for a token right before the first call that needs one
(a form, an action, the language picker). That call sets the `zeyn_csrf` cookie and is never
cached. Every `POST` is checked exactly as on any other page: without a matching token it is
refused with `403`, and nothing is retried silently.

Signed in users always get the cookie and the token in the page, as before.

## HTTP caching

```rust
#[screen(route = "/pricing", public, cache = 300)]
pub fn pricing() -> Screen {
    Screen::new("Pricing")
}
```

`cache = N` lets browsers and shared caches keep the page for `N` seconds (1 to 86400). For a
request without a session the page then gets:

| Header | Value |
|---|---|
| `Cache-Control` | `public, max-age=300` |
| `ETag` | A strong tag over the rendered body |
| `Vary` | `x-zeyn-partial, Accept-Language, Cookie` |

A request with a matching `If-None-Match` gets `304 Not Modified` without a body. Screens loaded by
in-app navigation are cached the same way, with their own tag.

A request with a session never gets public cache headers: the page is sent with
`Cache-Control: private, no-store`. `cache` on a screen that is not `public` does not compile.

The screen is still built and rendered for every request that reaches the server; the tag saves the
transfer, and a cache in front of the app saves the request. Keep cached screens free of anything
that depends on the visitor: the time of day, a counter, a random pick.
