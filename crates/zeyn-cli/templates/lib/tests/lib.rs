//! The library mounted into a throwaway app, the way real apps use it.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use zeyn::I18n;
use zeyn::prelude::*;

fn app() -> App {
    let i18n = I18n::new("en").catalogs({{crate}}::CATALOGS).unwrap();
    App::new("Library test")
        .i18n(i18n)
        .auth(DevAuth::superuser("tester"))
        .service({{crate}}::customers::CustomerStore::default())
        .feature({{crate}}::customers::feature())
}

#[test]
fn the_feature_has_no_structural_problems() {
    assert_eq!(app().check(), Ok(()));
}

#[test]
fn every_catalog_has_the_same_keys() {
    let i18n = I18n::new("en").catalogs({{crate}}::CATALOGS).unwrap();
    assert!(i18n.missing_keys().is_empty(), "{:?}", i18n.missing_keys());
}

#[tokio::test]
async fn the_list_opens_with_translated_labels() {
    let req = Request::get("/customers/").header("accept-language", "tr").body(Body::empty()).unwrap();
    let res = app().router().unwrap().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(res.into_body()).await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("Müşteriler"));
}
