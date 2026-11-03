# 公开页面

标记为 `public` 的 screen 任何人都可以打开，包括从未登录的访客：落地页、价目表、帮助中心。Zeyn 以低成本交付这些页面，并便于缓存和搜索引擎处理。

## 不给访客设置 cookie

由没有会话的请求打开的公开 screen 不会设置 cookie。页面携带一个空的 CSRF 令牌，因此使用同一语言和主题的每位访客得到的字节完全相同。

浏览器运行时会在第一个需要令牌的调用（表单、action、语言选择器）之前，通过 `GET /_zeyn/csrf` 请求令牌。该调用会设置 `zeyn_csrf` cookie，并且永远不会被缓存。每个 `POST` 的检查方式与其他页面完全相同：没有匹配的令牌就会以 `403` 拒绝，不会静默重试。

已登录的用户和以前一样，总是会在页面中得到 cookie 和令牌。

## HTTP 缓存

```rust
#[screen(route = "/pricing", public, cache = 300)]
pub fn pricing() -> Screen {
    Screen::new("Pricing")
}
```

`cache = N` 让浏览器和共享缓存把页面保留 `N` 秒（1 到 86400）。对于没有会话的请求，页面随后会得到：

| 响应头 | 值 |
|---|---|
| `Cache-Control` | `public, max-age=300` |
| `ETag` | 基于渲染后正文的强标签 |
| `Vary` | `x-zeyn-partial, Accept-Language, Cookie` |

带有匹配 `If-None-Match` 的请求会得到不带正文的 `304 Not Modified`。通过应用内导航加载的 screen 以同样方式缓存，并带有各自的标签。

带有会话的请求永远不会得到公开缓存响应头：页面以 `Cache-Control: private, no-store` 发送。在非 `public` 的 screen 上使用 `cache` 无法编译。

每个到达服务器的请求仍然会构建并渲染该 screen；标签节省的是传输，应用前面的缓存节省的是请求。请让被缓存的 screen 不包含任何依赖访客的内容：一天中的时间、计数器、随机选择。

## 搜索引擎与链接预览

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

| 方法 | head 标签 |
|---|---|
| `.description(text)` | `<meta name="description">` 和 `og:description`，使用用户的语言 |
| `.canonical(url)` | `<link rel="canonical">` 和 `og:url` |
| `.og_image(url)` | `og:image` |
| `.robots(Robots::Index)` 或 `Robots::NoIndex` | `<meta name="robots">` |
| `.json_ld(value)` | `<script type="application/ld+json">` |

每个值都会被转义。JSON-LD 中的 `<`、`>` 和 `&` 分别写成 `<`、`>` 和 `&`，因此包含 `</script>` 的值无法结束该元素；浏览器永远不会运行它，内容安全策略与其他页面一样严格。JSON-LD 地址属性（`url`、`@id`、`image`、`logo`、`item` 等）中的应用路径，无论层级多深，都会按搜索引擎的要求用 `App::site_url` 写成绝对地址。

URL 必须是 `https` URL，或以 `/` 开头的应用路径。其他任何形式（`http:`、`//host`、`javascript:`、空格、引号）都会被省略，并在日志中给出警告。应用路径会写在站点 URL 和基础路径之后：

```rust
App::new(t!("app.name")).site_url("https://shop.example.com")
```

没有 `site_url` 时，head 标签保留应用路径，浏览器能解析它，但某些爬虫不能。

应用内导航会更新标题和所有这些标签，而无需加载页面。

## 站点地图

```rust
#[action(public)]
pub async fn site_pages(part: SitemapPart, db: Db) -> Result<Vec<SitemapUrl>> {
    let rows = db.products_after(part.offset, part.limit).await?;
    Ok(rows.into_iter().map(|p| SitemapUrl::new(format!("/products/{}", p.slug)).lastmod(p.changed)).collect())
}

Feature::new("site").sitemap(site_pages)
```

`/sitemap.xml` 每次向 action 请求 1,000 个 URL（`part.offset`、`part.limit`），并以流的方式输出 XML，因此大型站点永远不会整个驻留在内存中。最多 50,000 个 URL 时，它是一个 `urlset`。超过这个数量时，它是由 `/sitemaps/1.xml`、`/sitemaps/2.xml` 等组成的站点地图索引，每个包含 50,000 个 URL。该 action 必须是 `public`，并以稳定的顺序返回这些 URL。应用路径需要 `App::site_url`，因为站点地图包含绝对 URL；不是 `https` URL 或应用路径的条目会被省略。只列出公开页面：搜索引擎无法打开其他页面。每个应用只有一个 feature 拥有站点地图。

## robots.txt 与 llms.txt

```rust
App::new(t!("app.name"))
    .robots_txt("User-agent: *\nAllow: /\nSitemap: https://shop.example.com/sitemap.xml\n")
    .llms_txt(include_str!("../llms.txt"))
```

两者都以纯文本形式提供给所有人。
