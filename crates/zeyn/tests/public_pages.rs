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
    assert_eq!(body["token"], token);

    let req =
        Request::get("/_zeyn/csrf").header(header::COOKIE, format!("zeyn_csrf={token}")).body(Body::empty()).unwrap();
    let again = send(&router, req).await;
    assert!(again.headers.get(header::SET_COOKIE).is_none());
    let body: serde_json::Value = serde_json::from_str(&again.body).unwrap();
    assert_eq!(body["token"], token);

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
