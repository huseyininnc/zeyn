//! The browser module of the crate: linked only by pages that hold the component, listed in the
//! runtime for content that arrives later, served as files with the build in the URL, and its
//! texts reach the page and can be reworded by the app.

#![allow(clippy::unwrap_used)]

use zeyn::I18n;
use zeyn::prelude::*;
use zeyn::testing::TestApp;
use {{crate}}::{CATALOGS, {{type}}, {{type}}Plugin};

#[screen(route = "/plain", public)]
pub fn plain() -> Screen {
    Screen::new("Plain").body(Paragraph::new("No component here"))
}

#[screen(route = "/with", public)]
pub fn with() -> Screen {
    Screen::new("With").body({{type}}::new("Press me"))
}

fn app_with(i18n: I18n) -> TestApp {
    TestApp::new(
        App::new("Test")
            .i18n(i18n)
            .plugin({{type}}Plugin)
            .feature(
                Feature::new("demo")
                    .screen(plain)
                    .screen(with),
            ),
    )
}

fn app() -> TestApp {
    app_with(I18n::new("en"))
}

#[test]
fn every_catalog_has_the_same_keys() {
    let i18n = I18n::new("en")
        .catalogs(CATALOGS)
        .unwrap();
    assert!(i18n.missing_keys().is_empty(), "{:?}", i18n.missing_keys());
}

#[tokio::test]
async fn a_page_with_the_component_links_its_module() {
    let page = app().get("/with").await;
    assert_eq!(page.status, 200);
    let links = page.links();
    assert!(links.contains(&"ext/{{component}}.js".to_string()), "{links:?}");
    assert!(links.contains(&"ext/{{component}}.css".to_string()), "{links:?}");
    assert!(
        page.body
            .contains(r#"aria-pressed="false""#)
    );
    assert!(
        page.body
            .contains(r#"role="status" aria-live="polite""#)
    );
}

#[tokio::test]
async fn a_page_without_the_component_never_references_its_module() {
    let page = app().get("/plain").await;
    assert_eq!(page.status, 200);
    assert!(!page.body.contains("ext/{{component}}"), "{:?}", page.links());
}

#[tokio::test]
async fn the_runtime_lists_the_module_for_content_that_arrives_later() {
    let js = app().get("/_zeyn/zeyn.js").await;
    assert_eq!(js.status, 200);
    assert!(
        js.body
            .contains(r#"["ext/{{component}}","[data-zeyn-{{component}}-toggle]","[data-zeyn-{{component}}-toggle]"]"#)
    );
}

#[tokio::test]
async fn the_files_are_served_with_their_type_and_kept_until_the_next_build() {
    let app = app();
    let v = zeyn::server::build_id();
    for (file, kind) in [("{{component}}.js", "text/javascript"), ("{{component}}.css", "text/css")] {
        let r = app
            .get(&format!("/_zeyn/ext/{file}?v={v}"))
            .await;
        assert_eq!(r.status, 200, "{file}");
        let header = |name: &str| {
            r.headers
                .get(name)
                .and_then(|h| h.to_str().ok())
                .unwrap_or_default()
                .to_string()
        };
        assert!(header("content-type").starts_with(kind), "{file}");
        assert!(header("cache-control").contains("immutable"), "{file}");
    }
}

#[tokio::test]
async fn the_texts_reach_the_page_and_the_app_can_reword_them() {
    let i18n = I18n::new("en")
        .catalog("en", r#"{ "{{component}}": { "on": "Yes" } }"#)
        .and_then(|i| i.catalog("tr", "{}"))
        .unwrap();
    let app = app_with(i18n);
    let tr = app
        .clone()
        .header("accept-language", "tr")
        .get("/with")
        .await;
    assert!(
        tr.body
            .contains(r#"data-text-on="Açık""#)
    );
    let en = app.get("/with").await;
    assert!(
        en.body
            .contains(r#"data-text-on="Yes""#)
    );
    assert!(
        en.body
            .contains(r#"data-text-off="Off""#)
    );
}
