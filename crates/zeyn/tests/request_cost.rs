//! What one request costs: the caller is authenticated once, and the grids of a screen load at
//! the same time.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tower::ServiceExt;
use zeyn::prelude::*;

#[entity(table = "slow_rows")]
pub struct SlowRow {
    #[key]
    pub id: SlowRowId,
    pub name: String,
}

/// Loads of [`slow_rows`] running now, and the most that ever ran at once.
#[derive(Clone, Default)]
pub struct Probe(Arc<(AtomicUsize, AtomicUsize)>);

impl Service for Probe {}

#[action(permission = "rows.read")]
pub async fn slow_rows(_q: DataQuery, probe: Probe) -> Result<Page<SlowRow>> {
    let now = probe.0.0.fetch_add(1, Ordering::SeqCst) + 1;
    probe
        .0
        .1
        .fetch_max(now, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(300)).await;
    probe.0.0.fetch_sub(1, Ordering::SeqCst);
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

fn app(calls: Arc<AtomicUsize>, probe: Probe) -> axum::Router {
    App::new("Cost")
        .service(probe)
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
    let router = app(calls.clone(), Probe::default());
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
    let probe = Probe::default();
    let router = app(Arc::default(), probe.clone());
    let (status, body) = get(&router, "/two").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.matches(">one<").count(), 2, "{body}");
    assert_eq!(
        probe.0.1.load(Ordering::SeqCst),
        2,
        "the two grids' first pages load at the same time"
    );
}
