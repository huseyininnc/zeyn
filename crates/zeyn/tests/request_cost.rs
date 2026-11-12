//! What one request costs: the caller is authenticated once, and the grids of a screen load at
//! the same time.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tower::ServiceExt;
use zeyn::prelude::*;

#[entity(table = "slow_rows")]
pub struct SlowRow {
    #[key]
    pub id: SlowRowId,
    pub name: String,
}

#[action(permission = "rows.read")]
pub async fn slow_rows(_q: DataQuery) -> Result<Page<SlowRow>> {
    tokio::time::sleep(Duration::from_millis(300)).await;
    Ok(Page::new(vec![SlowRow {
        id: SlowRowId(1),
        name: "one".into(),
    }]))
}

#[screen(route = "/two", permission = "rows.read")]
pub fn two_grids() -> Screen {
    Screen::new("Two").body(
        Stack::new()
            .add(DataGrid::of(slow_rows).column(SlowRow::name))
            .add(DataGrid::of(slow_rows).column(SlowRow::name)),
    )
}

fn app(calls: Arc<AtomicUsize>) -> axum::Router {
    App::new("Cost")
        .auth(move |h: &HeaderMap| {
            calls.fetch_add(1, Ordering::SeqCst);
            Some(Identity::user(h.get("x-user")?.to_str().ok()?).with_permissions(["rows.read"]))
        })
        .rate_limit(RateLimit::per_user(1000, Duration::from_secs(60)))
        .write_limit(RateLimit::per_user(1000, Duration::from_secs(60)))
        .feature(Feature::new("rows").screen(two_grids))
        .router()
        .unwrap()
}

async fn get(router: &axum::Router, path: &str) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(
            Request::get(path)
                .header("x-user", "ayse")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let body = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn a_request_authenticates_the_caller_once() {
    let calls = Arc::new(AtomicUsize::new(0));
    let router = app(calls.clone());
    let (status, _) = get(&router, "/two").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the rate limit by user, the handler and its grids share one authentication"
    );
}

#[tokio::test]
async fn the_grids_of_a_screen_load_at_the_same_time() {
    let router = app(Arc::default());
    let started = Instant::now();
    let (status, body) = get(&router, "/two").await;
    let took = started.elapsed();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.matches(">one<").count(), 2, "{body}");
    assert!(
        took < Duration::from_millis(550),
        "two grids of 300 ms each took {took:?}; they should load together"
    );
}
