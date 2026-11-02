//! Runs against a real Redis when `ZEYN_TEST_REDIS` is set, for example:
//!
//! ```text
//! docker run -d --rm --name zeyn-redis -p 6379:6379 redis:7-alpine
//! ZEYN_TEST_REDIS=redis://127.0.0.1:6379 cargo test -p zeyn --features redis --test redis
//! ```
//!
//! Without the variable every test that needs a server returns early.
#![cfg(feature = "redis")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::time::Duration;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{DataGridView, DataGridViewStore, RateLimit, RateStore, RedisStore};

fn url() -> Option<String> {
    std::env::var("ZEYN_TEST_REDIS").ok().filter(|u| !u.is_empty())
}

/// Two stores on one Redis, like two app servers, with a prefix of their own per test run.
fn pair(url: &str, test: &str) -> (RedisStore, RedisStore) {
    let prefix = format!("test-{test}-{}-{}", std::process::id(), DateTime::now().unix_ms());
    (RedisStore::new(url, &prefix).unwrap(), RedisStore::new(url, &prefix).unwrap())
}

#[tokio::test]
async fn rate_limits_are_shared_between_servers() {
    let Some(url) = url() else { return };
    let (a, b) = pair(&url, "rate");
    let limit = RateLimit::per_ip(3, Duration::from_secs(60));
    assert!(a.take("ip:10.0.0.1", &limit).await.is_ok());
    assert!(a.take("ip:10.0.0.1", &limit).await.is_ok());
    assert!(b.take("ip:10.0.0.1", &limit).await.is_ok());
    let wait = b.take("ip:10.0.0.1", &limit).await.unwrap_err();
    assert!(wait > Duration::ZERO && wait <= Duration::from_secs(20), "{wait:?}");
    assert!(a.take("ip:10.0.0.1", &limit).await.is_err());
    assert!(a.take("ip:10.0.0.2", &limit).await.is_ok());
}

#[tokio::test]
async fn concurrent_takes_never_exceed_the_limit() {
    let Some(url) = url() else { return };
    let (a, b) = pair(&url, "race");
    let limit = RateLimit::per_user(10, Duration::from_secs(60));
    let calls = (0..60).map(|i| {
        let store = if i % 2 == 0 { a.clone() } else { b.clone() };
        tokio::spawn(async move { store.take("user:ayse", &limit).await.is_ok() })
    });
    let mut allowed = 0;
    for call in calls.collect::<Vec<_>>() {
        allowed += usize::from(call.await.unwrap());
    }
    assert_eq!(allowed, 10);
}

#[tokio::test]
async fn saved_views_follow_the_user_to_another_server() {
    let Some(url) = url() else { return };
    let (a, b) = pair(&url, "views");
    let view = DataGridView { sort: vec![zeyn::Sort { field: "name".into(), desc: true }], ..Default::default() };
    a.save("ayse", "acme/customers/0", Some(view.clone())).await.unwrap();
    assert_eq!(b.load("ayse", "acme/customers/0").await.unwrap(), Some(view.clone()));
    assert_eq!(b.load("mehmet", "acme/customers/0").await.unwrap(), None);

    a.save("a:b", "c", Some(view.clone())).await.unwrap();
    assert_eq!(b.load("a", "b:c").await.unwrap(), None);

    b.save("ayse", "acme/customers/0", None).await.unwrap();
    assert_eq!(a.load("ayse", "acme/customers/0").await.unwrap(), None);
}

#[screen(route = "/", public)]
pub fn home() -> Screen {
    Screen::new("Home")
}

#[tokio::test]
async fn the_server_answers_429_from_shared_counters() {
    let Some(url) = url() else { return };
    let (a, b) = pair(&url, "app");
    let app = |store: RedisStore| {
        App::new("T")
            .rate_limit(RateLimit::per_ip(2, Duration::from_secs(60)))
            .rate_store(store)
            .feature(Feature::new("home").screen(home))
            .router()
            .unwrap()
    };
    let (one, two) = (app(a), app(b));
    let get = || Request::get("/").body(Body::empty()).unwrap();
    assert_eq!(one.clone().oneshot(get()).await.unwrap().status(), StatusCode::OK);
    assert_eq!(two.clone().oneshot(get()).await.unwrap().status(), StatusCode::OK);
    let refused = one.oneshot(get()).await.unwrap();
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(refused.headers().contains_key("retry-after"));
}

#[tokio::test]
async fn an_unreachable_redis_lets_requests_through_and_refuses_views() {
    let store = RedisStore::new("redis://127.0.0.1:1", "down").unwrap();
    let limit = RateLimit::per_ip(1, Duration::from_secs(60));
    assert!(store.take("ip:1", &limit).await.is_ok());
    assert!(store.take("ip:1", &limit).await.is_ok());
    assert!(store.load("ayse", "grid").await.is_err());
    assert!(RedisStore::new("not a url", "x").is_err());
}
