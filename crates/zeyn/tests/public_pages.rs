//! Public pages: cookie free pages for visitors without a session, the CSRF token endpoint, HTTP
//! caching of `#[screen(public, cache = N)]`, and head tags for search engines.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::time::Instant;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{I18n, Identity};

#[screen(route = "/about", public, cache = 300)]
pub fn about() -> Screen {
    Screen::new("About")
}

#[screen(route = "/news", public)]
pub fn news() -> Screen {
    Screen::new("News")
}

#[screen(route = "/desk", permission = "desk.read")]
pub fn desk() -> Screen {
    Screen::new("Desk")
}

fn app() -> App {
    let i18n = I18n::new("en").catalog("tr", "{}").unwrap();
    App::new("Site")
        .i18n(i18n)
        .auth(|h: &HeaderMap| h.contains_key("x-user").then(|| Identity::user("ayse").with_permissions(["desk.read"])))
        .feature(Feature::new("site").screen(about).screen(news).screen(desk))
}

fn router() -> Router {
    app().router().unwrap()
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn header(&self, name: header::HeaderName) -> &str {
        self.headers.get(name).map(|v| v.to_str().unwrap()).unwrap_or("")
    }
}

async fn send(router: &Router, req: Request<Body>) -> Reply {
    let res = router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let body = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    Reply { status, headers, body }
}

fn visitor(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

fn member(path: &str) -> Request<Body> {
    Request::get(path).header("x-user", "1").body(Body::empty()).unwrap()
}

fn locale_post(cookie: Option<&str>, token: Option<&str>) -> Request<Body> {
    let mut req = Request::post("/_zeyn/locale").header(header::CONTENT_TYPE, "application/json");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, format!("zeyn_csrf={c}"));
    }
    if let Some(t) = token {
        req = req.header("x-zeyn-csrf", t);
    }
    req.body(Body::from(r#"{"locale":"tr"}"#)).unwrap()
}

fn cookie_token(r: &Reply) -> String {
    let c = r.header(header::SET_COOKIE);
    c.strip_prefix("zeyn_csrf=").and_then(|c| c.split(';').next()).unwrap().to_string()
}

#[tokio::test]
async fn a_public_screen_without_a_session_sets_no_cookie_and_carries_no_token() {
    let r = send(&router(), visitor("/news")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.headers.get(header::SET_COOKIE).is_none(), "{:?}", r.headers);
    assert!(r.body.contains(r#"<meta name="zeyn-csrf" content="">"#));
    assert!(r.headers.get(header::CACHE_CONTROL).is_none(), "public screens without `cache` are not cached");
}

#[tokio::test]
async fn signed_in_pages_and_private_screens_keep_the_csrf_cookie() {
    let r = send(&router(), member("/news")).await;
    assert!(r.header(header::SET_COOKIE).starts_with("zeyn_csrf="));
    assert!(!r.body.contains(r#"<meta name="zeyn-csrf" content="">"#));
    let r = send(&router(), member("/desk")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.header(header::SET_COOKIE).starts_with("zeyn_csrf="));
}

#[tokio::test]
async fn the_csrf_endpoint_sets_the_cookie_once_and_is_never_stored() {
    let router = router();
    let r = send(&router, visitor("/_zeyn/csrf")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header(header::CACHE_CONTROL), "no-store");
    let token = cookie_token(&r);
    assert!(
        r.header(header::SET_COOKIE).contains("HttpOnly") && r.header(header::SET_COOKIE).contains("SameSite=Strict")
    );
    let body: serde_json::Value = serde_json::from_str(&r.body).unwrap();
    let masked = body["token"].as_str().unwrap().to_string();
    assert!(masked.len() == 128 && !masked.contains(&token), "the token is sent masked: {masked}");
    assert_eq!(send(&router, locale_post(Some(&token), Some(&masked))).await.status, StatusCode::NO_CONTENT);

    let req =
        Request::get("/_zeyn/csrf").header(header::COOKIE, format!("zeyn_csrf={token}")).body(Body::empty()).unwrap();
    let again = send(&router, req).await;
    assert!(again.headers.get(header::SET_COOKIE).is_none());
    let body: serde_json::Value = serde_json::from_str(&again.body).unwrap();
    let remasked = body["token"].as_str().unwrap();
    assert!(remasked.len() == 128 && remasked != masked, "every response masks the token again");
    assert_eq!(send(&router, locale_post(Some(&token), Some(remasked))).await.status, StatusCode::NO_CONTENT);

    let req = Request::get("/_zeyn/csrf").header("sec-fetch-site", "cross-site").body(Body::empty()).unwrap();
    let foreign = send(&router, req).await;
    assert_eq!(foreign.status, StatusCode::FORBIDDEN);
    assert!(foreign.headers.get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn posts_from_a_cookie_free_page_are_refused_until_the_token_is_fetched() {
    let router = router();
    let page = send(&router, visitor("/about")).await;
    assert!(page.headers.get(header::SET_COOKIE).is_none());

    assert_eq!(send(&router, locale_post(None, None)).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(&router, locale_post(None, Some(&"a".repeat(64)))).await.status, StatusCode::FORBIDDEN);

    let token = cookie_token(&send(&router, visitor("/_zeyn/csrf")).await);
    assert_eq!(send(&router, locale_post(Some(&token), None)).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(&router, locale_post(Some(&token), Some(&"b".repeat(64)))).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(&router, locale_post(Some(&token), Some(&token))).await.status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn a_cached_public_screen_sends_cache_headers_an_etag_and_answers_304() {
    let router = router();
    let r = send(&router, visitor("/about")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header(header::CACHE_CONTROL), "public, max-age=300");
    assert_eq!(r.header(header::VARY), "x-zeyn-partial, Accept-Language, Cookie");
    assert!(r.headers.get(header::SET_COOKIE).is_none());
    let tag = r.header(header::ETAG).to_string();
    assert!(tag.starts_with('"') && tag.ends_with('"') && !tag.starts_with("W/"), "{tag}");

    let same = send(&router, visitor("/about")).await;
    assert_eq!(same.header(header::ETAG), tag, "the same page gets the same tag");

    let req =
        Request::get("/about").header(header::IF_NONE_MATCH, format!("\"x\", {tag}")).body(Body::empty()).unwrap();
    let not_modified = send(&router, req).await;
    assert_eq!(not_modified.status, StatusCode::NOT_MODIFIED);
    assert!(not_modified.body.is_empty());
    assert_eq!(not_modified.header(header::ETAG), tag);
    assert_eq!(not_modified.header(header::CACHE_CONTROL), "public, max-age=300");

    let req = Request::get("/about").header(header::IF_NONE_MATCH, "\"stale\"").body(Body::empty()).unwrap();
    assert_eq!(send(&router, req).await.status, StatusCode::OK);

    let req = Request::get("/about").header(header::ACCEPT_LANGUAGE, "tr").body(Body::empty()).unwrap();
    assert_ne!(send(&router, req).await.header(header::ETAG), tag, "another language is another body");

    let req = Request::get("/about").header("x-zeyn-partial", "1").body(Body::empty()).unwrap();
    let partial = send(&router, req).await;
    assert_eq!(partial.header(header::CACHE_CONTROL), "public, max-age=300");
    assert_ne!(partial.header(header::ETAG), tag, "partial screens and full pages have their own tags");
}

#[tokio::test]
async fn a_request_with_a_session_never_gets_public_cache_headers() {
    let router = router();
    let r = send(&router, member("/about")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.header(header::CACHE_CONTROL), "private, no-store");
    assert!(r.headers.get(header::ETAG).is_none());
    let tag = send(&router, visitor("/about")).await.header(header::ETAG).to_string();
    let req =
        Request::get("/about").header("x-user", "1").header(header::IF_NONE_MATCH, &tag).body(Body::empty()).unwrap();
    assert_eq!(send(&router, req).await.status, StatusCode::OK);
    let req = Request::get("/about").header("x-user", "1").header("x-zeyn-partial", "1").body(Body::empty()).unwrap();
    assert_eq!(send(&router, req).await.header(header::CACHE_CONTROL), "no-store");
}

/// Render cost of a public page: run with `--release --ignored --nocapture` to print the fastest
/// of 20 batches of 1000 requests, which is the least disturbed by other work on the machine.
#[tokio::test]
#[ignore]
async fn measure_public_page_render_cost() {
    let router = router();
    let tag = send(&router, visitor("/about")).await.header(header::ETAG).to_string();
    let cases: [(&str, fn(&str) -> Request<Body>); 4] = [
        ("public, cache = 300", |_| visitor("/about")),
        ("public", |_| visitor("/news")),
        ("signed in", |_| member("/news")),
        ("304", |tag| Request::get("/about").header(header::IF_NONE_MATCH, tag).body(Body::empty()).unwrap()),
    ];
    for (label, req) in cases {
        let mut best = std::time::Duration::MAX;
        for _ in 0..20 {
            let started = Instant::now();
            for _ in 0..1000 {
                send(&router, req(&tag)).await;
            }
            best = best.min(started.elapsed() / 1000);
        }
        println!("{label:20} {best:?} per request");
    }
}

#[screen(route = "/pricing", public)]
pub fn pricing() -> Screen {
    Screen::new("Pricing")
        .description("Plans & prices <for> \"teams\"")
        .canonical("/pricing")
        .og_image("https://cdn.example/pricing.png")
        .robots(Robots::Index)
        .json_ld(serde_json::json!({ "@type": "Product", "name": "</script><script>alert(1)</script> & co" }))
}

#[screen(route = "/bad-head", public)]
pub fn bad_head() -> Screen {
    Screen::new("Bad").canonical("http://plain.example/").og_image("javascript:alert(1)").robots(Robots::NoIndex)
}

#[action(public)]
pub async fn few_pages(part: SitemapPart) -> Result<Vec<SitemapUrl>> {
    let all = [
        SitemapUrl::new("/").lastmod(Date::new(2026, 10, 1).unwrap()),
        SitemapUrl::new("https://blog.example/a?x=1&y=2"),
        SitemapUrl::new("http://insecure.example/"),
        SitemapUrl::new("//evil.example/"),
    ];
    Ok(part.of(&all))
}

/// 120,000 generated URLs, answered for any part without building the whole list.
#[action(public)]
pub async fn many_pages(part: SitemapPart) -> Result<Vec<SitemapUrl>> {
    let end = (part.offset + u64::from(part.limit)).min(120_000);
    Ok((part.offset..end).map(|i| SitemapUrl::new(format!("/item/{i}"))).collect())
}

#[action(permission = "desk.read")]
pub async fn private_pages(part: SitemapPart) -> Result<Vec<SitemapUrl>> {
    Ok(part.of(&[SitemapUrl::new("/")]))
}

fn seo_app() -> App {
    App::new("Site")
        .site_url("https://shop.example/")
        .robots_txt("User-agent: *\nAllow: /\n")
        .feature(Feature::new("site").screen(pricing).screen(bad_head).screen(news).sitemap(few_pages))
}

#[tokio::test]
async fn head_tags_are_escaped_and_json_ld_cannot_end_its_script() {
    let r = send(&seo_app().router().unwrap(), visitor("/pricing")).await;
    assert_eq!(r.status, StatusCode::OK);
    let head = &r.body[..r.body.find("</head>").unwrap()];
    assert!(
        head.contains(
            r#"<meta name="description" content="Plans &amp; prices &lt;for&gt; &quot;teams&quot;" data-zeyn-head>"#
        ),
        "{head}"
    );
    assert!(head.contains(r#"<link rel="canonical" href="https://shop.example/pricing" data-zeyn-head>"#));
    assert!(head.contains(r#"<meta property="og:image" content="https://cdn.example/pricing.png" data-zeyn-head>"#));
    assert!(head.contains(r#"<meta name="robots" content="index, follow" data-zeyn-head>"#));
    let start = head.find(r#"<script type="application/ld+json" data-zeyn-head>"#).unwrap();
    let script = &head[start..];
    let json = &script[script.find('>').unwrap() + 1..script.find("</script>").unwrap()];
    assert!(!json.contains('<') && !json.contains('>') && !json.contains('&'), "{json}");
    let data: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(data["name"], "</script><script>alert(1)</script> & co");
    assert_eq!(r.body.matches("<script>").count(), 0, "no inline script reaches the page");
    assert!(r.header(header::CONTENT_SECURITY_POLICY).contains("script-src 'self'"));
}

#[tokio::test]
async fn head_urls_that_are_not_https_or_app_paths_are_left_out() {
    let r = send(&seo_app().router().unwrap(), visitor("/bad-head")).await;
    assert!(!r.body.contains("canonical") && !r.body.contains("og:image") && !r.body.contains("javascript:"));
    assert!(r.body.contains(r#"<meta name="robots" content="noindex" data-zeyn-head>"#));
    assert!(!zeyn::seo::valid_url("https://user@host.example/"));
    assert!(!zeyn::seo::valid_url("https:///nohost"));
    assert!(!zeyn::seo::valid_url("/a b"));
    assert!(!zeyn::seo::valid_url("/a\\b"));
}

#[tokio::test]
async fn in_app_navigation_gets_the_head_tags_in_a_template_before_the_screen() {
    let router = seo_app().router().unwrap();
    let req = Request::get("/pricing").header("x-zeyn-partial", "1").body(Body::empty()).unwrap();
    let r = send(&router, req).await;
    assert!(r.body.starts_with("<template data-zeyn-head><meta name=\"description\""), "{}", &r.body[..80]);
    assert!(r.body.contains(r#"<link rel="canonical" href="https://shop.example/pricing" data-zeyn-head>"#));
    assert!(r.header("x-zeyn-title".parse().unwrap()).starts_with("Pricing"));
    let req = Request::get("/news").header("x-zeyn-partial", "1").body(Body::empty()).unwrap();
    assert!(send(&router, req).await.body.starts_with("<section"), "a screen without head tags sends none");
}

#[tokio::test]
async fn app_paths_stay_paths_without_a_site_url() {
    let router = App::new("Site").base_path("/shop").feature(Feature::new("site").screen(pricing)).router().unwrap();
    let r = send(&router, visitor("/shop/pricing")).await;
    assert!(r.body.contains(r#"<link rel="canonical" href="/shop/pricing" data-zeyn-head>"#));
}

#[tokio::test]
async fn robots_txt_is_plain_text_for_everyone() {
    let r = send(&seo_app().router().unwrap(), visitor("/robots.txt")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body, "User-agent: *\nAllow: /\n");
    assert!(r.header(header::CONTENT_TYPE).starts_with("text/plain"));
    assert_eq!(send(&router(), visitor("/robots.txt")).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_sitemap_lists_only_https_urls_and_app_paths_behind_the_site_url() {
    let r = send(&seo_app().router().unwrap(), visitor("/sitemap.xml")).await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(r.header(header::CONTENT_TYPE).starts_with("application/xml"));
    assert_eq!(
        r.body,
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#,
            "<url><loc>https://shop.example/</loc><lastmod>2026-10-01</lastmod></url>",
            "<url><loc>https://blog.example/a?x=1&amp;y=2</loc></url>",
            "</urlset>"
        )
    );
    assert_eq!(send(&router(), visitor("/sitemap.xml")).await.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn above_50000_urls_the_sitemap_is_an_index_of_streamed_parts() {
    let router = App::new("Site")
        .site_url("https://shop.example")
        .feature(Feature::new("site").screen(news).sitemap(many_pages))
        .router()
        .unwrap();
    let index = send(&router, visitor("/sitemap.xml")).await;
    assert!(index.body.contains("<sitemapindex"), "{}", &index.body[..200]);
    let locs: Vec<&str> = index.body.split("<loc>").skip(1).map(|s| s.split("</loc>").next().unwrap()).collect();
    assert_eq!(
        locs,
        [
            "https://shop.example/sitemaps/1.xml",
            "https://shop.example/sitemaps/2.xml",
            "https://shop.example/sitemaps/3.xml"
        ]
    );

    let first = send(&router, visitor("/sitemaps/1.xml")).await;
    assert_eq!(first.body.matches("<url>").count(), 50_000);
    assert!(first.body.contains("<loc>https://shop.example/item/0</loc>") && first.body.contains("/item/49999<"));
    let last = send(&router, visitor("/sitemaps/3.xml")).await;
    assert_eq!(last.body.matches("<url>").count(), 20_000);
    assert!(last.body.contains("/item/100000<") && last.body.ends_with("</urlset>"));
    assert_eq!(send(&router, visitor("/sitemaps/4.xml")).await.body.matches("<url>").count(), 0);
    assert_eq!(send(&router, visitor("/sitemaps/0.xml")).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&router, visitor("/sitemaps/x.xml")).await.status, StatusCode::NOT_FOUND);
}

#[test]
fn a_sitemap_action_must_be_public_and_the_site_url_an_https_origin() {
    let problems = App::new("Site").feature(Feature::new("site").sitemap(private_pages)).router().unwrap_err();
    assert!(problems.iter().any(|p| p.contains("is not public")), "{problems:?}");
    for bad in ["http://shop.example", "https://shop.example/path", "shop.example"] {
        let problems = App::new("Site").site_url(bad).router().unwrap_err();
        assert!(problems.iter().any(|p| p.contains("not an https origin")), "{bad}: {problems:?}");
    }
}
