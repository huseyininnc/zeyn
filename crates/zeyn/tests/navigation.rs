use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;

#[screen(route = "/one", public)]
pub fn one() -> Screen {
    Screen::new("First <screen>")
}

#[screen(route = "/two", permission = "two.read")]
pub fn two() -> Screen {
    Screen::new("Second")
}

fn app(theme: Option<Theme>) -> axum::Router {
    let mut app = App::new("Nav").feature(Feature::new("n").screen(one).screen(two));
    if let Some(t) = theme {
        app = app.theme(t);
    }
    app.router().unwrap()
}

async fn get(router: &axum::Router, path: &str, partial: bool) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut b = Request::get(path);
    if partial {
        b = b.header("x-zeyn-partial", "1");
    }
    let res = router.clone().oneshot(b.body(Body::empty()).unwrap()).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    (status, headers, String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap())
}

#[tokio::test]
async fn full_pages_set_the_color_scheme_before_css_loads_and_include_navigation() {
    let (_, _, body) = get(&app(None), "/one", false).await;
    assert!(body.contains(r#"<meta name="color-scheme" content="light dark">"#));
    assert!(body.contains(r#"<script type="module" src="/_zeyn/nav.js"></script>"#));
    let (_, _, dark) = get(&app(Some(Theme::new().dark())), "/one", false).await;
    assert!(dark.contains(r#"<meta name="color-scheme" content="dark">"#));
}

#[tokio::test]
async fn partial_requests_return_only_the_screen_and_its_title() {
    let (status, headers, body) = get(&app(None), "/one", true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("x-zeyn-partial").unwrap(), "1");
    assert_eq!(headers.get("x-zeyn-title").unwrap(), "First%20%3Cscreen%3E%20%C2%B7%20Nav");
    assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-store");
    assert!(body.starts_with(r#"<section class="z-screen""#));
    assert!(body.contains("First &lt;screen&gt;"));
    assert!(!body.contains("<html"));
}

#[tokio::test]
async fn partial_requests_that_fail_get_a_full_page_so_the_runtime_falls_back() {
    let (status, headers, body) = get(&app(None), "/two", true).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(headers.get("x-zeyn-partial").is_none());
    assert!(body.contains("<html"));
}

#[tokio::test]
async fn the_navigation_script_is_served() {
    let (status, headers, body) = get(&app(None), "/_zeyn/nav.js", false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "text/javascript; charset=utf-8");
    assert!(body.contains("x-zeyn-partial"));
}
