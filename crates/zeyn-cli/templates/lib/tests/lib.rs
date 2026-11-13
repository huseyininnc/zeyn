//! The library mounted into a throwaway app, the way real apps use it, and driven in memory with
//! `zeyn::testing::TestApp`.

#![allow(clippy::unwrap_used)]

use zeyn::I18n;
use zeyn::prelude::*;
use zeyn::testing::TestApp;

fn app() -> App {
    let i18n = I18n::new("en")
        .catalogs({{crate}}::CATALOGS)
        .unwrap();
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
    let i18n = I18n::new("en")
        .catalogs({{crate}}::CATALOGS)
        .unwrap();
    assert!(i18n.missing_keys().is_empty(), "{:?}", i18n.missing_keys());
}

#[tokio::test]
async fn the_list_opens_with_translated_labels() {
    let page = TestApp::new(app())
        .header("accept-language", "tr")
        .get("/customers/")
        .await;
    assert_eq!(page.status, 200);
    assert!(page.body.contains("Müşteriler"));
}
