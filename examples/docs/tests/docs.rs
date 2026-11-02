//! The documentation app: every page opens, internal links resolve, code is highlighted and
//! escaped, live examples render and the palette search finds pages.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::LazyLock;
use tower::ServiceExt;
use zeyn_example_docs::markdown::{Part, render};
use zeyn_example_docs::{Docs, PAGES, app, live};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

static ROUTER: LazyLock<Router> = LazyLock::new(|| app(Docs::build()).unwrap().router().unwrap());

async fn get(path: &str) -> (StatusCode, String) {
    let res = ROUTER.clone().oneshot(Request::get(path).body(Body::empty()).unwrap()).await.unwrap();
    let status = res.status();
    (status, String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap())
}

fn article(page: &str) -> &str {
    let start = page.find(r#"<article class="z-prose">"#).expect("the page has an article");
    &page[start..start + page[start..].find("</article>").unwrap()]
}

fn attrs<'a>(html: &'a str, name: &str) -> Vec<&'a str> {
    let pat = format!(r#"{name}=""#);
    html.match_indices(&pat)
        .map(|(i, _)| {
            let s = &html[i + pat.len()..];
            &s[..s.find('"').unwrap()]
        })
        .collect()
}

#[tokio::test]
async fn every_page_opens_with_its_title_in_the_menu() {
    assert!(PAGES.len() >= 25);
    for p in PAGES {
        let (status, page) = get(p.route).await;
        assert_eq!(status, StatusCode::OK, "{}", p.route);
        assert!(page.contains(&format!(r#"href="{}""#, p.route)), "{} is in the sidebar", p.route);
        assert!(page.contains(r#"<nav class="z-doc-toc""#), "{}", p.route);
    }
}

#[tokio::test]
async fn internal_links_and_anchors_resolve() {
    let mut pages = std::collections::HashMap::new();
    for p in PAGES {
        pages.insert(p.route, get(p.route).await.1);
    }
    for (route, page) in &pages {
        for href in attrs(article(page), "href")
            .into_iter()
            .chain(attrs(page, "href").into_iter().filter(|h| h.starts_with('#')))
        {
            if href.starts_with("http") || href.starts_with("mailto:") {
                continue;
            }
            let (path, anchor) = href.split_once('#').unwrap_or((href, ""));
            let target = if path.is_empty() {
                page
            } else {
                pages.get(path).unwrap_or_else(|| panic!("{route} links to {href}, which is not a page"))
            };
            if !anchor.is_empty() {
                assert!(
                    target.contains(&format!(r#"id="{anchor}""#)),
                    "{route} links to {href}, which has no such anchor"
                );
            }
        }
    }
}

#[tokio::test]
async fn code_blocks_are_highlighted_and_nothing_in_an_article_is_raw_html() {
    let mut highlighted = 0;
    for p in PAGES {
        let page = get(p.route).await.1;
        let a = article(&page);
        assert!(!a.contains("<script") && !a.contains("<style") && !a.contains("<iframe"), "{}", p.route);
        highlighted += a.matches(r#"<span class="z-hl-kw">"#).count();
    }
    assert!(highlighted > 100, "{highlighted}");

    let out = render(
        "```rust\nlet html = \"<script>alert('x')</script>\";\n```\n\n```js\nel.innerHTML = '<img src=x onerror=alert(1)>';\n```\n",
        &|t| t.to_string(),
    );
    let Part::Html(html) = &out.parts[0] else { panic!() };
    assert!(!html.contains("<script") && !html.contains("<img"), "{html}");
    assert!(
        html.contains(r#"<span class="z-hl-str">&quot;&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;&quot;</span>"#),
        "{html}"
    );
}

#[tokio::test]
async fn live_examples_render_real_components() {
    let docs = Docs::build();
    let used: Vec<&str> = docs.pages().iter().flat_map(|d| d.lives().iter().map(String::as_str)).collect();
    for name in live::EXAMPLES {
        assert!(used.contains(name), "live example `{name}` is not on any page");
    }
    for name in &used {
        assert!(live::EXAMPLES.contains(name), "unknown live example `{name}`");
    }
    let (_, grid) = get("/language/datagrid").await;
    assert!(grid.contains(r#"class="z-doc-live""#));
    assert!(grid.contains("S-0001"), "the grid's first page is rendered with the page");
}

#[tokio::test]
async fn the_palette_searches_every_page_without_a_reload() {
    let (_, page) = get("/").await;
    assert!(page.contains("data-zeyn-commands data-search"));
    let req = Request::post("/_zeyn/search")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(r#"{"query":"density compact"}"#))
        .unwrap();
    let res = ROUTER.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert!(!hits.is_empty() && hits.len() <= 20);
    assert_eq!(hits[0]["url"], "/guides/theming#density", "{hits:?}");
}

#[test]
fn the_app_passes_its_checks() {
    assert_eq!(app(Docs::build()).unwrap().check(), Ok(()));
}
