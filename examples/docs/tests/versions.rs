//! Versioned docs: the header's version picker, the version switch that keeps the page, a build
//! under `/v<version>/`, the notice, canonical URL and `noindex` of older versions, and the
//! checks on the version settings.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn_example_docs::versions::{MAX_VERSIONS, Versions};
use zeyn_example_docs::{Docs, app, router};

fn build(versions: Versions) -> Router {
    router(
        app(Docs::build(versions.base()), versions.clone()).unwrap(),
        &versions,
    )
    .unwrap()
}

/// The latest version at `/`, with 0.0.9 listed as older.
fn latest() -> Router {
    build(Versions::new(
        Some("0.0.10"),
        Some("0.0.10,0.0.9"),
        Some("0.0.10"),
        None,
    ))
}

/// Version 0.0.9 under `/v0.0.9`, with 0.0.10 as the latest.
fn old() -> Router {
    build(Versions::new(
        Some("0.0.9"),
        Some("0.0.10,0.0.9"),
        Some("0.0.10"),
        Some("/v0.0.9"),
    ))
}

async fn get(router: &Router, path: &str, referer: Option<&str>) -> (StatusCode, String, String) {
    let mut req = Request::get(path);
    if let Some(r) = referer {
        req = req.header(header::REFERER, r);
    }
    let res = router
        .clone()
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let location = res
        .headers()
        .get(header::LOCATION)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let body = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (
        status,
        String::from_utf8_lossy(&body).into_owned(),
        location,
    )
}

#[tokio::test]
async fn the_picker_lists_every_version_and_marks_the_current_one() {
    let (status, page, _) = get(&latest(), "/guides/security", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page.contains(">v0.0.10</button>"),
        "the button shows the version"
    );
    assert!(
        page.contains(r#"<li><a href="/_docs/switch" data-zeyn-reload aria-current="true">0.0.10 (latest)</a></li>"#),
        "{page}"
    );
    assert!(page.contains(r#"<li><a href="/v0.0.9/_docs/switch" data-zeyn-reload>0.0.9</a></li>"#));
    let (_, tr, _) = get(&latest(), "/tr/guides/security", None).await;
    assert!(
        tr.contains("0.0.10 (en yeni)"),
        "the picker speaks the page's language"
    );
    assert!(tr.contains("Belge sürümü"));
}

#[tokio::test]
async fn the_switch_opens_the_same_page_in_the_chosen_version() {
    let (status, _, to) = get(
        &old(),
        "/v0.0.9/_docs/switch",
        Some("https://docs.zeynsoft.com/tr/guides/security"),
    )
    .await;
    assert_eq!(status, StatusCode::FOUND);
    assert_eq!(to, "/v0.0.9/tr/guides/security");
    let (status, _, to) = get(
        &latest(),
        "/_docs/switch",
        Some("https://docs.zeynsoft.com/v0.0.9/guides/accounts?x=1#a"),
    )
    .await;
    assert_eq!(status, StatusCode::FOUND);
    assert_eq!(to, "/guides/accounts");
}

#[tokio::test]
async fn a_page_the_version_does_not_have_redirects_to_its_start_page() {
    for (referer, start) in [
        ("https://docs.zeynsoft.com/guides/not-in-0.0.9", "/v0.0.9/"),
        (
            "https://docs.zeynsoft.com/ja/guides/not-in-0.0.9",
            "/v0.0.9/ja/",
        ),
        ("https://evil.example/%0d%0aSet-Cookie:x", "/v0.0.9/"),
        ("not a url", "/v0.0.9/"),
    ] {
        let (status, _, to) = get(&old(), "/v0.0.9/_docs/switch", Some(referer)).await;
        assert_eq!(status, StatusCode::FOUND, "{referer}");
        assert_eq!(to, start, "{referer}");
    }
    let (_, _, to) = get(&old(), "/v0.0.9/_docs/switch", None).await;
    assert_eq!(to, "/v0.0.9/", "no referer opens the start page");
}

#[tokio::test]
async fn a_build_under_a_version_path_serves_its_pages_and_assets_there() {
    let router = old();
    let (status, page, _) = get(&router, "/v0.0.9/guides/accounts", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page.contains(r#"href="/v0.0.9/guides/security""#),
        "menu links carry the prefix"
    );
    let start = page
        .find(r#"<article class="z-prose">"#)
        .unwrap();
    let article = &page[start..];
    assert!(
        article.contains(r#"href="/v0.0.9/guides/"#),
        "links between pages stay in the version"
    );
    assert!(
        !article.contains(r#"href="/guides/"#),
        "no link leaves the version"
    );
    assert!(page.contains(r#"src="/v0.0.9/_zeyn/"#) || page.contains(r#"href="/v0.0.9/_zeyn/"#));
    for path in ["/v0.0.9/", "/v0.0.9/tr/", "/v0.0.9/tr/guides/accounts"] {
        let (status, _, _) = get(&router, path, None).await;
        assert_eq!(status, StatusCode::OK, "{path}");
    }
    let (status, js, _) = get(&router, "/v0.0.9/_zeyn/nav.js", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!js.is_empty());
    let (status, _, _) = get(&router, "/guides/accounts", None).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "nothing is served outside the prefix"
    );
}

#[tokio::test]
async fn an_older_version_has_the_notice_a_canonical_url_at_the_root_and_noindex() {
    let router = old();
    let (_, page, _) = get(&router, "/v0.0.9/tr/guides/accounts", None).await;
    assert!(page.contains("0.0.9 sürümünün belgelerini okuyorsunuz. En yeni sürüm 0.0.10."));
    assert!(page.contains(
        r#"<a href="/_docs/switch" data-zeyn-reload>Bu sayfayı en yeni sürümde okuyun</a>"#
    ));
    assert!(
        page.contains(
            r#"<link rel="canonical" href="https://docs.zeynsoft.com/tr/guides/accounts""#
        )
    );
    assert!(page.contains(r#"<meta name="robots" content="noindex"#));
    assert!(
        !page.contains(r#"rel="alternate""#),
        "no language alternates under a version path"
    );
    assert!(
        page.contains(r#"<a href="/v0.0.9/guides/accounts" hreflang="en" lang="en">"#),
        "the link to the English page stays in the version"
    );
    assert!(
        page.contains(r#"class="z-doc-next" href="/v0.0.9/tr/guides/sign-in""#),
        "the pager stays in the version"
    );
    let (status, xml, _) = get(&router, "/v0.0.9/sitemap.xml", None).await;
    assert!(
        status != StatusCode::OK || !xml.contains("<loc>"),
        "the sitemap lists nothing: {xml}"
    );
    let (status, llms, _) = get(&router, "/v0.0.9/llms.txt", None).await;
    assert_eq!(status, StatusCode::OK, "the article links it");
    assert!(
        !llms.contains("/v0.0.9"),
        "llms.txt points to the latest pages"
    );
}

#[tokio::test]
async fn the_latest_version_at_the_root_has_no_notice_and_stays_indexed() {
    let router = latest();
    let (_, page, _) = get(&router, "/guides/accounts", None).await;
    assert!(!page.contains("The latest version is"));
    assert!(!page.contains("noindex"));
    assert!(
        page.contains(r#"<link rel="canonical" href="https://docs.zeynsoft.com/guides/accounts""#)
    );
    assert!(page.contains(r#"hreflang="tr""#));
    let (_, xml, _) = get(&router, "/sitemap.xml", None).await;
    assert!(xml.contains("<loc>https://docs.zeynsoft.com/guides/accounts</loc>"));
    assert!(!xml.contains("/v0.0.9"));
    let (status, _, _) = get(&router, "/llms.txt", None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_latest_version_under_its_own_path_has_no_notice_but_is_not_indexed() {
    let router = build(Versions::new(
        Some("0.0.10"),
        Some("0.0.10,0.0.9"),
        None,
        Some("/v0.0.10"),
    ));
    let (status, page, _) = get(&router, "/v0.0.10/guides/accounts", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!page.contains("The latest version is"));
    assert!(page.contains(r#"<meta name="robots" content="noindex"#));
    assert!(page.contains(
        r#"<a href="/_docs/switch" data-zeyn-reload aria-current="true">0.0.10 (latest)</a>"#
    ));
}

#[test]
fn version_settings_fall_back_to_safe_values() {
    let v = Versions::new(
        Some("../1"),
        Some("x,,0.0.9;rm"),
        Some("9.9.9"),
        Some("/admin"),
    );
    assert_eq!(v.current(), env!("CARGO_PKG_VERSION"));
    assert_eq!(v.all(), [env!("CARGO_PKG_VERSION")]);
    assert_eq!(v.latest(), env!("CARGO_PKG_VERSION"));
    assert_eq!(v.base(), "", "only /v<version> is a base path");
    assert!(!v.is_old() && !v.prefixed());

    let v = Versions::new(
        Some(" 0.0.9 "),
        Some("0.0.10, 0.0.9"),
        Some("0.0.10"),
        Some("v0.0.9/"),
    );
    assert_eq!(
        (v.current(), v.latest(), v.base()),
        ("0.0.9", "0.0.10", "/v0.0.9")
    );
    assert!(v.is_old() && v.prefixed());
    assert_eq!(v.switch_url("0.0.10"), "/_docs/switch");
    assert_eq!(v.switch_url("0.0.9"), "/v0.0.9/_docs/switch");

    let many: Vec<String> = (0..80)
        .map(|i| format!("0.{i}.0"))
        .collect();
    let v = Versions::new(Some("1.0.0"), Some(&many.join(",")), None, None);
    assert_eq!(v.all().len(), MAX_VERSIONS);
    assert_eq!(
        v.all().last().map(String::as_str),
        Some("1.0.0"),
        "the build's own version stays listed"
    );
    assert_eq!(v.latest(), "0.0.0");
    assert_eq!(
        Versions::new(Some("0.0.9"), Some("0.0.9"), Some("0.0.8"), None).latest(),
        "0.0.9"
    );
}
