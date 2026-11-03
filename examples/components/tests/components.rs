//! The component catalog: every page opens with its examples, the shown code is the code of the
//! running example, the API tables are filled from the framework's source, every text exists in
//! both languages, and the site's own files are linked from every page.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::LazyLock;
use tower::ServiceExt;
use zeyn_example_components::page::{keys, snippet};
use zeyn_example_components::{API, COMPONENTS, app, site};

static ROUTER: LazyLock<Router> = LazyLock::new(|| site::router(app().unwrap().router().unwrap()));

async fn get(path: &str) -> (StatusCode, String, String) {
    let res = ROUTER
        .clone()
        .oneshot(
            Request::get(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let kind = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, kind, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn the_index_shows_a_live_preview_card_for_every_component() {
    let (status, _, page) = get("/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        page.matches(r#"<li class="cx-card">"#)
            .count(),
        COMPONENTS.len()
    );
    assert_eq!(
        page.matches(r#"<div class="cx-preview" inert>"#)
            .count(),
        COMPONENTS.len()
    );
    assert!(page.contains(r#"data-zeyn-searchbox"#));
    for c in COMPONENTS {
        assert!(
            page.contains(&format!(r#"href="{}""#, c.route)),
            "{} has a card",
            c.slug
        );
    }
    assert!(
        page.contains(site::HEAD),
        "the catalog's stylesheet and script are in the head"
    );
    assert!(
        page.contains("P-001"),
        "the grid preview has its first page"
    );
}

#[tokio::test]
async fn the_search_keeps_only_matching_components() {
    let (_, _, page) = get("/?params=%7B%22q%22%3A%22grid%22%7D").await;
    assert_eq!(
        page.matches(r#"<li class="cx-card">"#)
            .count(),
        3,
        "AutoGrid, DataGrid and CardGrid"
    );
    let (_, _, none) = get("/?params=%7B%22q%22%3A%22zzqq%22%7D").await;
    assert!(none.contains(r#"class="cx-none""#));
}

#[tokio::test]
async fn every_component_page_opens_with_its_examples_and_their_code() {
    for c in COMPONENTS {
        let (status, _, page) = get(c.route).await;
        assert_eq!(status, StatusCode::OK, "{}", c.slug);
        assert!(page.contains(r#"role="tablist""#), "{} has tabs", c.slug);
        assert!(
            page.contains("cx-example"),
            "{} shows its first example",
            c.slug
        );
        assert!(
            page.contains("data-cx-copy"),
            "{} has a copy button",
            c.slug
        );
        assert!(
            !page.contains("cx.ex.") && !page.contains("cx.summary."),
            "{} texts resolve",
            c.slug
        );
    }
}

#[test]
fn the_shown_code_is_the_function_that_builds_the_example() {
    for c in COMPONENTS {
        for e in c
            .entry
            .overview
            .iter()
            .chain(c.entry.examples)
        {
            let code = snippet(c.source, e.func);
            assert!(
                code.contains(&format!("pub fn {}(", e.func)),
                "{}::{}",
                c.slug,
                e.func
            );
            assert!(
                code.trim_end().ends_with('}'),
                "{}::{} is whole",
                c.slug,
                e.func
            );
            assert!(
                !code.contains("use crate::{Entry, ex};"),
                "the catalog's own import is left out"
            );
        }
    }
}

#[test]
fn the_api_tables_come_from_the_framework_source() {
    for c in COMPONENTS {
        for ty in c.entry.api {
            let methods = API
                .iter()
                .find(|(name, _)| name == ty)
                .map(|(_, m)| m.len())
                .unwrap_or(0);
            assert!(
                methods > 0,
                "{} lists `{ty}`, which has no public methods in crates/zeyn/src",
                c.slug
            );
        }
    }
    let avatar = API
        .iter()
        .find(|(name, _)| *name == "Avatar")
        .unwrap()
        .1;
    assert!(
        avatar
            .iter()
            .any(|(sig, doc)| sig.starts_with("pub fn decorative(") && doc.starts_with("Hides"))
    );
}

#[test]
fn every_text_exists_in_english_and_turkish() {
    for file in [
        include_str!("../locales/en.json"),
        include_str!("../locales/tr.json"),
    ] {
        let json: serde_json::Value = serde_json::from_str(file).unwrap();
        for key in keys() {
            let found = key
                .split('.')
                .try_fold(&json, |v, part| v.get(part));
            assert!(found.is_some_and(|v| v.is_string()), "missing {key}");
        }
    }
}

#[tokio::test]
async fn the_site_files_and_pictures_are_served() {
    let (status, kind, css) = get("/site/components.css").await;
    assert_eq!(
        (status, kind.as_str()),
        (StatusCode::OK, "text/css; charset=utf-8")
    );
    assert!(css.contains(".cx-card"));
    let (status, _, js) = get("/site/copy.js").await;
    assert_eq!(status, StatusCode::OK);
    assert!(js.contains("navigator.clipboard"));
    let (status, kind, svg) = get("/site/img/3.svg").await;
    assert_eq!((status, kind.as_str()), (StatusCode::OK, "image/svg+xml"));
    assert!(svg.starts_with("<svg"));
    let (status, _, _) = get("/site/img/13.svg").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
