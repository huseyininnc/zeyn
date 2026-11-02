//! Drives the app through its router, without opening a port.

#![allow(clippy::unwrap_used)]

#[path = "../src/app.rs"]
mod app;
#[path = "../src/features/mod.rs"]
mod features;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::{Config, StaticSource};

async fn router() -> axum::Router {
    let config = Config::load(vec![Box::new(StaticSource(Default::default()))]).await.unwrap();
    app::app(config).unwrap().router().unwrap()
}

#[tokio::test]
async fn the_app_has_no_structural_problems() {
    let config = Config::load(vec![]).await.unwrap();
    assert_eq!(app::app(config).unwrap().check(), Ok(()));
}

#[tokio::test]
async fn the_notes_list_opens() {
    let res = router().await.oneshot(Request::get("/notes/").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(body.contains("data-zeyn-grid"));
}

#[tokio::test]
async fn saving_through_the_api_validates_the_title() {
    let req = Request::post("/api/v1/notes/save_note")
        .header("authorization", "Bearer dev")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"id":0,"title":"","body":null,"due":null,"done":false,"version":0}"#))
        .unwrap();
    let res = router().await.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
