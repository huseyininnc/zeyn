//! The widget crate's browser module: linked only by pages that hold a toggle, listed in the
//! runtime for content that arrives later, served as files with the build in the URL, and its
//! texts reach the page.

use zeyn::prelude::*;
use zeyn::testing::TestApp;
use zeyn_example_widget::{Toggle, WidgetPlugin};

#[screen(route = "/plain", public)]
pub fn plain() -> Screen {
    Screen::new("Plain").body(Paragraph::new("No toggle here"))
}

#[screen(route = "/favorite", public)]
pub fn favorite() -> Screen {
    Screen::new("Favorite").body(Toggle::new("Mark as favorite"))
}

fn app() -> TestApp {
    TestApp::new(
        App::new("Test")
            .plugin(WidgetPlugin)
            .feature(
                Feature::new("demo")
                    .screen(plain)
                    .screen(favorite),
            ),
    )
}

#[tokio::test]
async fn a_page_with_the_component_links_its_module() {
    let page = app().get("/favorite").await;
    assert_eq!(page.status, 200);
    let links = page.links();
    assert!(links.contains(&"ext/widget.js".to_string()), "{links:?}");
    assert!(links.contains(&"ext/widget.css".to_string()), "{links:?}");
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
    assert!(!page.body.contains("ext/widget"), "{:?}", page.links());
}

#[tokio::test]
async fn the_runtime_lists_the_module_for_content_that_arrives_later() {
    let js = app().get("/_zeyn/zeyn.js").await;
    assert_eq!(js.status, 200);
    assert!(
        js.body
            .contains(r#"["ext/widget","[data-zeyn-widget-toggle]","[data-zeyn-widget-toggle]"]"#)
    );
}

#[tokio::test]
async fn the_files_are_served_with_their_type_and_kept_until_the_next_build() {
    let app = app();
    let v = zeyn::server::build_id();
    for (file, kind) in [("widget.js", "text/javascript"), ("widget.css", "text/css")] {
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
    assert_eq!(
        app.get("/_zeyn/ext/other.js")
            .await
            .status,
        404
    );
}

/// The app with its own catalogs: Turkish offered, and the English "on" text reworded.
fn app_with(i18n: zeyn::I18n) -> TestApp {
    TestApp::new(
        App::new("Test")
            .i18n(i18n)
            .plugin(WidgetPlugin)
            .feature(Feature::new("demo").screen(favorite)),
    )
}

#[tokio::test]
async fn the_texts_of_the_module_reach_the_page_and_the_app_can_reword_them() {
    let i18n = zeyn::I18n::new("en")
        .catalog("en", r#"{ "widget": { "on": "Yes" } }"#)
        .and_then(|i| i.catalog("tr", "{}"))
        .unwrap();
    let app = app_with(i18n);
    let tr = app
        .clone()
        .header("accept-language", "tr")
        .get("/favorite")
        .await;
    assert!(
        tr.body
            .contains(r#"data-text-on="Açık""#)
    );
    let en = app.get("/favorite").await;
    assert!(
        en.body
            .contains(r#"data-text-on="Yes""#)
    );
    assert!(
        en.body
            .contains(r#"data-text-off="Off""#)
    );
}
