//! Palette search: `Feature::search`, `POST /_zeyn/search` and the marker the palette reads.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Identity, SearchHit};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[screen(route = "/home", public)]
pub fn home() -> Screen {
    Screen::new("Home")
}

#[action(public)]
pub async fn search_help(query: String) -> Result<Vec<SearchHit>> {
    let mut hits: Vec<SearchHit> = (0..30).map(|i| SearchHit::new(format!("{query} {i}"), "Help", format!("/help/{i}"))).collect();
    hits.insert(0, SearchHit::new("Elsewhere", "Help", "//evil.example/x"));
    hits.insert(0, SearchHit::new("Script", "Help", "javascript:alert(1)"));
    Ok(hits)
}

#[action(permission = "orders.read")]
pub async fn search_orders(query: String) -> Result<Vec<SearchHit>> {
    Ok(vec![SearchHit::new(format!("Order {query}"), "Orders", "/orders/1")])
}

fn closed(_: &Ctx) -> GuardResult {
    GuardResult::Deny("closed".into())
}

#[action(public)]
pub async fn search_closed(_query: String) -> Result<Vec<SearchHit>> {
    Ok(vec![SearchHit::new("Hidden", "Closed", "/closed")])
}

fn app() -> Router {
    App::new("T")
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h.get("x-perms")?.to_str().ok()?.split(',').map(String::from).collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(Feature::new("orders").search(search_orders))
        .feature(Feature::new("closed").guard(closed).search(search_closed))
        .feature(Feature::new("help").screen(home).search(search_help))
        .router()
        .unwrap()
}

async fn search(router: &Router, query: &str, perms: &str, csrf: bool) -> (StatusCode, serde_json::Value) {
    let mut req = Request::post("/_zeyn/search").header(header::CONTENT_TYPE, "application/json").header("x-perms", perms);
    if csrf {
        req = req.header(header::COOKIE, format!("zeyn_csrf={TOKEN}")).header("x-zeyn-csrf", TOKEN);
    }
    let res = router.clone().oneshot(req.body(Body::from(serde_json::json!({ "query": query }).to_string())).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn searches_run_with_permissions_and_guards_and_only_return_app_paths() {
    let r = app();
    let (status, body) = search(&r, "tea", "", true).await;
    assert_eq!(status, StatusCode::OK);
    let hits = body["hits"].as_array().unwrap();
    assert_eq!(hits.len(), zeyn::MAX_SEARCH_HITS);
    assert_eq!(hits[0], serde_json::json!({ "label": "tea 0", "group": "Help", "url": "/help/0" }));
    assert!(hits.iter().all(|h| h["url"].as_str().unwrap().starts_with("/help/")), "no other feature, no unsafe path: {hits:?}");

    let (_, body) = search(&r, "7", "orders.read", true).await;
    assert_eq!(body["hits"][0]["url"], "/orders/1");
    assert_eq!(search(&r, "   ", "orders.read", true).await.1["hits"], serde_json::json!([]));
    assert_eq!(search(&r, "tea", "", false).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn the_palette_offers_search_only_when_a_search_is_allowed() {
    let r = app();
    let res = r.clone().oneshot(Request::get("/home").body(Body::empty()).unwrap()).await.unwrap();
    let page = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(page.contains("data-zeyn-commands data-search"));

    let plain = App::new("T").feature(Feature::new("help").screen(home)).router().unwrap();
    let res = plain.oneshot(Request::get("/home").body(Body::empty()).unwrap()).await.unwrap();
    let page = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(!page.contains("data-search"));
}
