//! Drives the admin app through its router and its socket, on the sample data in memory: every
//! screen opens, each role sees and changes only what it may, and saving an order notifies the
//! pages that show orders.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use std::time::Duration;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::realtime::TestSocket;
use zeyn::{Authenticator, Config};
use zeyn_example_admin::models::OrderEvent;
use zeyn_example_admin::roles::Role;
use zeyn_example_admin::store::Store;

/// Signs every request in with the role named in `x-role`.
fn by_role() -> impl Authenticator {
    |h: &HeaderMap| {
        let role = Role::named(h.get("x-role")?.to_str().ok()?)?;
        Some(role.identity("tester"))
    }
}

fn sample() -> Store {
    Store::sample(TimeZone::UTC.today(DateTime::now()))
}

async fn app_with(store: Store) -> App {
    let config = Config::load(vec![]).await.unwrap();
    zeyn_example_admin::app(config, store, by_role()).unwrap()
}

async fn app() -> App {
    app_with(sample()).await
}

async fn get(path: &str, role: &str) -> (StatusCode, String) {
    let router = app().await.router().unwrap();
    let req = Request::get(path)
        .header("x-role", role)
        .header("accept-language", "en")
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
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
async fn the_app_has_no_structural_problems() {
    assert_eq!(app().await.check(), Ok(()));
}

#[tokio::test]
async fn every_screen_opens_for_an_administrator() {
    for (path, marker) in [
        ("/", "data-zeyn-live=\"orders\""),
        ("/orders", "data-zeyn-grid"),
        ("/orders/42", "SO-00042"),
        ("/customers", "data-zeyn-grid"),
        ("/customers/8", "role=\"tablist\""),
        ("/products", "data-zeyn-grid"),
        ("/admin/settings", "z-form-settings"),
        ("/admin/users", "data-zeyn-grid"),
        ("/admin/roles", "<table"),
    ] {
        let (status, body) = get(path, "Admin").await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(body.contains(marker), "{path} has no {marker}");
    }
}

#[tokio::test]
async fn an_unknown_order_answers_not_found() {
    assert_eq!(get("/orders/99999", "Admin").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn signed_out_visitors_open_nothing() {
    let req = Request::get("/orders")
        .body(Body::empty())
        .unwrap();
    let res = app()
        .await
        .router()
        .unwrap()
        .oneshot(req)
        .await
        .unwrap();
    assert_ne!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn each_role_opens_only_its_screens() {
    let cases = [
        ("Viewer", "/orders", StatusCode::OK),
        ("Viewer", "/admin/settings", StatusCode::FORBIDDEN),
        ("Viewer", "/admin/users", StatusCode::FORBIDDEN),
        ("Viewer", "/admin/roles", StatusCode::FORBIDDEN),
        ("Manager", "/admin/settings", StatusCode::OK),
        ("Manager", "/admin/users", StatusCode::OK),
        ("Admin", "/admin/users", StatusCode::OK),
    ];
    for (role, path, expected) in cases {
        assert_eq!(get(path, role).await.0, expected, "{role} {path}");
    }
}

#[tokio::test]
async fn only_roles_that_may_change_a_record_get_its_editor() {
    let (_, body) = get("/admin/settings", "Manager").await;
    assert!(!body.contains("z-form-settings"));
    assert!(body.contains("Acme Trading"));
    let (_, body) = get("/orders/42", "Viewer").await;
    assert!(!body.contains("Status and notes"));
    let (_, body) = get("/orders/42", "Manager").await;
    assert!(body.contains("Status and notes"));
    let (_, body) = get("/admin/users", "Manager").await;
    assert!(!body.contains("New user"));
}

#[tokio::test]
async fn the_menu_leaves_out_what_the_role_may_not_open() {
    let (_, body) = get("/orders", "Viewer").await;
    assert!(!body.contains("href=\"/admin/settings\""));
    let (_, body) = get("/orders", "Admin").await;
    assert!(body.contains("href=\"/admin/settings\""));
    assert!(body.contains("href=\"/admin/roles\""));
}

#[tokio::test]
async fn saving_an_order_notifies_open_pages_and_writes_its_history() {
    let store = sample();
    let addr = TestSocket::serve(
        app_with(store.clone())
            .await
            .router()
            .unwrap(),
    )
    .await;
    let mut viewer = TestSocket::connect(addr, &[("x-role", "Viewer")])
        .await
        .unwrap();
    viewer
        .subscribe(&["orders"])
        .await
        .unwrap();
    let mut manager = TestSocket::connect(addr, &[("x-role", "Manager")])
        .await
        .unwrap();
    let page = manager
        .call("search_orders", serde_json::json!({}))
        .await
        .unwrap();
    let mut order = page["items"][0].clone();
    let number = order["number"].clone();
    order["status"] = "Cancelled".into();
    order["number"] = "FORGED".into();
    assert!(
        viewer
            .call("save_order", &order)
            .await
            .is_err(),
        "a viewer may not save orders"
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    let saved = manager
        .call("save_order", &order)
        .await
        .unwrap();
    assert_eq!(saved["status"], "Cancelled");
    assert_eq!(
        saved["number"], number,
        "read only fields come from the store"
    );
    assert_eq!(viewer.next_topic().await.unwrap(), "orders");
    assert!(
        manager
            .call("save_order", &order)
            .await
            .is_err(),
        "an old row version is a conflict"
    );
    let id = order["id"].as_i64().unwrap();
    let history = store
        .find::<OrderEvent>(&DataQuery::default())
        .await
        .unwrap()
        .items;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].order_id, id);
    assert_eq!(history[0].user_name, "tester");
    assert!(history[0].detail.contains("Cancelled"));
}
