//! The documentation app: every page opens, internal links resolve, code is highlighted and
//! escaped, live examples render and the palette search finds pages.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::LazyLock;
use tower::ServiceExt;
use zeyn_example_docs::markdown::{Part, render};
use zeyn_example_docs::{
    Docs, LANGS, PAGES, Status, TRANSLATIONS, app, lang_path, live, source_hash,
};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

static ROUTER: LazyLock<Router> = LazyLock::new(|| {
    app(Docs::build())
        .unwrap()
        .router()
        .unwrap()
});

async fn get(path: &str) -> (StatusCode, String) {
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
    (
        status,
        String::from_utf8(
            res.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap(),
    )
}

fn article(page: &str) -> &str {
    let start = page
        .find(r#"<article class="z-prose">"#)
        .expect("the page has an article");
    &page[start
        ..start
            + page[start..]
                .find("</article>")
                .unwrap()]
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
        assert!(
            page.contains(&format!(r#"href="{}""#, p.route)),
            "{} is in the sidebar",
            p.route
        );
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
            .chain(
                attrs(page, "href")
                    .into_iter()
                    .filter(|h| h.starts_with('#')),
            )
        {
            if href.starts_with("http")
                || href.starts_with("mailto:")
                || href.ends_with(".txt")
                || href.ends_with(".xml")
                || href.contains("?params=")
            {
                continue;
            }
            let (path, anchor) = href
                .split_once('#')
                .unwrap_or((href, ""));
            let target = if path.is_empty() {
                page
            } else {
                pages
                    .get(path)
                    .unwrap_or_else(|| panic!("{route} links to {href}, which is not a page"))
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
        assert!(
            !a.contains("<script") && !a.contains("<style") && !a.contains("<iframe"),
            "{}",
            p.route
        );
        highlighted += a
            .matches(r#"<span class="z-hl-kw">"#)
            .count();
    }
    assert!(highlighted > 100, "{highlighted}");

    let out = render(
        "```rust\nlet html = \"<script>alert('x')</script>\";\n```\n\n```js\nel.innerHTML = '<img src=x onerror=alert(1)>';\n```\n",
        &|t| t.to_string(),
    );
    let Part::Html(html) = &out.parts[0] else {
        panic!()
    };
    assert!(
        !html.contains("<script") && !html.contains("<img"),
        "{html}"
    );
    assert!(
        html.contains(r#"<span class="z-hl-str">&quot;&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;&quot;</span>"#),
        "{html}"
    );
}

#[tokio::test]
async fn live_examples_render_real_components() {
    let docs = Docs::build();
    let used: Vec<&str> = docs
        .pages()
        .iter()
        .flat_map(|d| d.lives().iter().map(String::as_str))
        .collect();
    for name in live::EXAMPLES {
        assert!(
            used.contains(name),
            "live example `{name}` is not on any page"
        );
    }
    for name in &used {
        assert!(
            live::EXAMPLES.contains(name),
            "unknown live example `{name}`"
        );
    }
    let (_, grid) = get("/language/datagrid").await;
    assert!(grid.contains(r#"class="z-doc-live""#));
    assert!(
        grid.contains("S-0001"),
        "the grid's first page is rendered with the page"
    );
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
    let res = ROUTER
        .clone()
        .oneshot(req)
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_slice(
        &res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert!(!hits.is_empty() && hits.len() <= 20);
    assert_eq!(hits[0]["url"], "/guides/theming#density", "{hits:?}");
}

#[test]
fn the_app_passes_its_checks() {
    assert_eq!(app(Docs::build()).unwrap().check(), Ok(()));
}

#[test]
fn every_page_has_a_file_in_every_language() {
    let mut missing = Vec::new();
    for (lang, _) in &LANGS[1..] {
        for p in PAGES {
            let file = p.translation_file();
            if !TRANSLATIONS
                .iter()
                .any(|(l, f, _)| l == lang && *f == file)
            {
                missing.push(format!("pages/{lang}/{file}"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "translations missing (the site shows the English page there): {missing:?}"
    );
}

#[test]
fn stale_translations_are_listed() {
    let docs = Docs::build();
    let mut stale = Vec::new();
    for (lang, _) in &LANGS[1..] {
        for d in docs.lang(lang) {
            if !d.status.translated() {
                stale.push(format!("{lang}{} ({:?})", d.route, d.status));
            }
        }
    }
    if !stale.is_empty() {
        eprintln!(
            "{} pages are shown in English until translated again:\n  {}",
            stale.len(),
            stale.join("\n  ")
        );
    }
}

#[test]
fn translated_pages_keep_the_anchors_of_the_english_page() {
    let docs = Docs::build();
    for (lang, _) in &LANGS[1..] {
        for (en, d) in docs.pages().iter().zip(docs.lang(lang)) {
            assert_eq!(
                d.headings(),
                en.headings(),
                "{lang}{}: headings differ from the English page",
                d.route
            );
            assert_ne!(d.status, Status::Headings, "{lang}{}", d.route);
        }
    }
}

/// Records the English sources of the pages translated now in `pages/translations.json`, for
/// the languages in `ZEYN_DOCS_TRANSLATED` (`tr,zh` or `all`). Does nothing without it.
#[test]
fn record_translations() {
    let Ok(wanted) = std::env::var("ZEYN_DOCS_TRANSLATED") else {
        return;
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pages/translations.json");
    let mut manifest: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (lang, _) in &LANGS[1..] {
        if wanted != "all"
            && !wanted
                .split(',')
                .any(|w| w.trim() == *lang)
        {
            continue;
        }
        let mut files = serde_json::Map::new();
        for p in PAGES {
            if TRANSLATIONS
                .iter()
                .any(|(l, f, _)| l == lang && *f == p.translation_file())
            {
                files.insert(
                    p.translation_file().to_string(),
                    source_hash(p.source).into(),
                );
            }
        }
        manifest.insert(lang.to_string(), files.into());
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&manifest).unwrap() + "\n",
    )
    .unwrap();
}

#[tokio::test]
async fn every_language_serves_every_page_at_its_path() {
    for (lang, tag) in LANGS {
        for p in PAGES {
            let path = lang_path(lang, p.route);
            let (status, page) = get(&path).await;
            assert_eq!(status, StatusCode::OK, "{path}");
            assert!(
                page.starts_with(&format!(r#"<!doctype html><html lang="{lang}""#)),
                "{path}"
            );
            assert_eq!(page.contains(r#" dir="rtl""#), *lang == "ar", "{path}");
            assert!(
                page.contains(&format!(r#"href="{path}""#)),
                "{path} is in the sidebar in its language"
            );
            let canonical = format!(
                r#"<link rel="canonical" href="https://docs.zeynsoft.com{path}" data-zeyn-head>"#
            );
            assert!(page.contains(&canonical), "{path}");
            assert_eq!(
                page.matches(r#"<link rel="alternate" hreflang=""#)
                    .count(),
                10,
                "{path}"
            );
            assert!(
                page.contains(&format!(
                    r#"hreflang="{tag}" href="https://docs.zeynsoft.com{path}""#
                )),
                "{path}"
            );
            assert!(page.contains(&format!(
                r#"hreflang="x-default" href="https://docs.zeynsoft.com{}""#,
                p.route
            )));
            let note = page.contains(r#"<p class="z-doc-note" role="note">"#);
            assert_eq!(note, *lang != "en", "{path} says it is a translation");
            if *lang != "en" {
                assert!(
                    page.contains(&format!(
                        r#"<a href="{}" hreflang="en" lang="en">"#,
                        p.route
                    )),
                    "{path}"
                );
            }
        }
    }
}

#[tokio::test]
async fn a_page_without_a_current_translation_is_shown_in_english_under_the_language_path() {
    let stale = format!(
        r#"{{ "tr": {{ "datagrid.md": "{}" }} }}"#,
        source_hash("an older English source")
    );
    let docs = Docs::build_from(&[("tr", "datagrid.md", "# Eski\n")], &stale);
    let at = PAGES
        .iter()
        .position(|p| p.route == "/language/datagrid")
        .unwrap();
    assert_eq!(docs.lang("tr")[at].status, Status::Stale);
    assert_eq!(docs.lang("zh")[at].status, Status::Missing);
    let router = app(docs).unwrap().router().unwrap();
    let res = router
        .oneshot(
            Request::get("/tr/language/datagrid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let page = String::from_utf8(
        res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(page.starts_with(r#"<!doctype html><html lang="tr""#));
    assert!(page.contains(r#"<article class="z-prose" lang="en"><p class="z-doc-note" role="note">Bu sayfa henüz çevrilmedi"#));
    assert!(
        page.contains("<h2 id=\"a-grid\">A grid</h2>"),
        "the English page is shown"
    );
    assert!(page.contains(r#"<link rel="canonical" href="https://docs.zeynsoft.com/language/datagrid" data-zeyn-head>"#));
    assert!(
        !page.contains(r#"hreflang="tr""#),
        "an untranslated page is not offered as Turkish"
    );
    assert!(
        page.contains(r#"href="/tr/language/datagrid-reference"#),
        "links stay in Turkish"
    );
}

#[tokio::test]
async fn the_sitemap_and_llms_txt_list_the_languages() {
    let (status, xml) = get("/sitemap.xml").await;
    assert_eq!(status, StatusCode::OK);
    for (lang, _) in LANGS {
        assert!(
            xml.contains(&format!(
                "<loc>https://docs.zeynsoft.com{}</loc>",
                lang_path(lang, "/guides/security")
            )),
            "{lang}"
        );
    }
    assert_eq!(xml.matches("<loc>").count(), PAGES.len() * LANGS.len());
    let (_, llms) = get("/llms.txt").await;
    assert!(llms.contains("https://docs.zeynsoft.com/tr/"));
    let (_, robots) = get("/robots.txt").await;
    assert!(robots.contains("Sitemap: https://docs.zeynsoft.com/sitemap.xml"));
}

#[tokio::test]
async fn the_palette_searches_the_pages_of_the_language() {
    let req = Request::post("/_zeyn/search")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .header(header::REFERER, "http://127.0.0.1/tr/guides/theming")
        .body(Body::from(r#"{"query":"density"}"#))
        .unwrap();
    let res = ROUTER
        .clone()
        .oneshot(req)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(
        &res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let hits = body["hits"].as_array().unwrap();
    assert!(!hits.is_empty());
    assert!(
        hits.iter().all(|h| h["url"]
            .as_str()
            .unwrap()
            .starts_with("/tr/")),
        "{hits:?}"
    );
}

#[tokio::test]
async fn links_in_translated_pages_stay_in_the_language_and_their_anchors_exist() {
    for (lang, _) in &LANGS[1..] {
        let mut pages = std::collections::HashMap::new();
        for p in PAGES {
            let path = lang_path(lang, p.route);
            let page = get(&path).await.1;
            pages.insert(path, page);
        }
        for (path, page) in &pages {
            for href in attrs(article(page), "href") {
                if !href.starts_with('/') || href.ends_with(".txt") || href.ends_with(".xml") {
                    continue;
                }
                let (target, anchor) = href
                    .split_once('#')
                    .unwrap_or((href, ""));
                if target == p_route_en(path, lang) {
                    continue;
                }
                let target = pages
                    .get(target)
                    .unwrap_or_else(|| panic!("{path} links to {href}, not a {lang} page"));
                if !anchor.is_empty() {
                    assert!(
                        target.contains(&format!(r#"id="{anchor}""#)),
                        "{path} links to {href}, which has no such anchor"
                    );
                }
            }
        }
    }
}

/// The English path of a page path in a language, which the translation note links to.
fn p_route_en<'a>(path: &'a str, lang: &str) -> &'a str {
    let rest = &path[lang.len() + 1..];
    if rest == "/" { "/" } else { rest }
}
