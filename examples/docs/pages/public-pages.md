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

## Search engines and link previews

```rust
#[screen(route = "/pricing", public, cache = 300)]
pub fn pricing() -> Screen {
    Screen::new(t!("pricing.title"))
        .description(t!("pricing.description"))
        .canonical("/pricing")
        .og_image("https://cdn.example.com/pricing.png")
        .robots(Robots::Index)
        .json_ld(serde_json::json!({
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Team plan",
        }))
}
```

| Method | Head tags |
|---|---|
| `.description(text)` | `<meta name="description">` and `og:description`, in the user's language |
| `.canonical(url)` | `<link rel="canonical">` and `og:url` |
| `.og_image(url)` | `og:image` |
| `.robots(Robots::Index)` or `Robots::NoIndex` | `<meta name="robots">` |
| `.json_ld(value)` | `<script type="application/ld+json">` |

Every value is escaped. JSON-LD is written with `<`, `>` and `&` as `\u003c`, `\u003e` and `\u0026`,
so a value holding `</script>` cannot end the element; the browser never runs it, and the content
security policy stays as strict as on every other page. App paths in address properties of
JSON-LD (`url`, `@id`, `image`, `logo`, `item` and the like) are written absolute with
`App::site_url`, at any depth, as search engines expect.

URLs must be `https` URLs or app paths starting with `/`. Anything else (`http:`, `//host`,
`javascript:`, spaces, quotes) is left out with a warning in the log. App paths are written behind
the site URL and the base path:

```rust
App::new(t!("app.name")).site_url("https://shop.example.com")
```

Without `site_url` head tags keep the app path, which browsers resolve but some crawlers do not.

In-app navigation updates the title and all of these tags without a page load.

## Sitemap

```rust
#[action(public)]
pub async fn site_pages(part: SitemapPart, db: Db) -> Result<Vec<SitemapUrl>> {
    let rows = db.products_after(part.offset, part.limit).await?;
    Ok(rows.into_iter().map(|p| SitemapUrl::new(format!("/products/{}", p.slug)).lastmod(p.changed)).collect())
}

Feature::new("site").sitemap(site_pages)
```

`/sitemap.xml` asks the action for 1,000 URLs at a time (`part.offset`, `part.limit`) and streams the
XML, so a large site never sits in memory. Up to 50,000 URLs it is one `urlset`. Above that it is a
sitemap index of `/sitemaps/1.xml`, `/sitemaps/2.xml` and so on, each with 50,000 URLs. The action
must be `public` and return the URLs in a stable order. App paths need `App::site_url`, because a
sitemap holds absolute URLs; entries that are not `https` URLs or app paths are left out. List only
public pages: search engines cannot open the others. One feature per app has a sitemap.

## robots.txt and llms.txt

```rust
App::new(t!("app.name"))
    .robots_txt("User-agent: *\nAllow: /\nSitemap: https://shop.example.com/sitemap.xml\n")
    .llms_txt(include_str!("../llms.txt"))
```

Both are served as plain text to everyone.
