//! Document viewer: the page sanitizer, page fetches with their checks and limits, the search
//! and the module that loads only with a viewer.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{
    DocPage, DocSearch, DocumentViewer, MAX_PAGE_BYTES, MAX_PAGE_HITS, OutlineItem, PageHit,
    PageSize, sanitize_markup,
};
use zeyn::{I18n, Identity};

#[test]
fn the_sanitizer_removes_scripts_and_everything_that_runs_or_reaches_out() {
    let dirty = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" onload="steal()">
<script>steal()</script><SCRIPT type="x">alert(1)</SCRIPT>
<foreignObject width="10" height="10"><div onclick="x()">inside</div><iframe src="//evil"></iframe></foreignObject>
<a href="javascript:alert(1)"><text x="1" y="2">click</text></a>
<a href=" java&#x09;script:alert(2)">two</a>
<a xlink:href="https://evil.example/">out</a>
<use href="https://evil.example/sprite.svg#icon"/><use xlink:href="#local"/>
<image href="https://evil.example/track.png"/><image href="data:image/svg+xml;base64,PHN2Zz4="/>
<rect width="10" height="10" style="fill:red;background:url(https://evil.example/x)" fill="url(https://evil.example/p)"/>
<rect id="local" width="5" height="5" style="fill: blue; behavior: url(x.htc)" data-zeyn-confirm="x" class="z-select keep"/>
<animate attributeName="href" to="javascript:alert(3)"/>
<iframe src="https://evil.example"></iframe><object data="x"><param name="a"></object><embed src="x">
<style>rect { fill: url(https://evil.example) }</style>
</svg>"##;
    let clean = sanitize_markup(dirty, "");
    for bad in [
        "script",
        "SCRIPT",
        "steal",
        "alert",
        "onload",
        "onclick",
        "foreignObject",
        "inside",
        "iframe",
        "evil",
        "javascript",
        "url(https",
        "behavior",
        "data-zeyn",
        "z-select",
        "animate",
        "object",
        "embed",
        "param",
        "style>",
        "svg+xml",
    ] {
        assert!(!clean.contains(bad), "{bad} survived: {clean}");
    }
    assert!(
        clean.starts_with(r#"<svg viewBox="0 0 100 100">"#),
        "{clean}"
    );
    assert!(
        clean.contains(r#"<text x="1" y="2">click</text>"#),
        "{clean}"
    );
    assert!(clean.contains(r##"<use href="#local"/>"##), "{clean}");
    assert!(
        clean.contains(r#"<rect id="local" width="5" height="5" style="fill:blue" class="keep"/>"#),
        "{clean}"
    );
    assert!(
        clean.contains(r#"<rect width="10" height="10" style="fill:red"/>"#),
        "{clean}"
    );
    assert!(clean.ends_with("</svg>"), "{clean}");
}

#[test]
fn the_sanitizer_keeps_shapes_text_gradients_and_inline_raster_images() {
    let page = r##"<svg width="794" height="1123" viewBox="0 0 595 842" preserveAspectRatio="xMidYMid meet">
<defs><linearGradient id="g" x1="0" x2="1"><stop offset="0" stop-color="#fff"/><stop offset="1" stop-color="#000"/></linearGradient>
<clipPath id="c"><rect width="10" height="10"/></clipPath></defs>
<g transform="translate(10 20)" clip-path="url(#c)"><path d="M0 0L10 10Z" stroke="#333" stroke-width="0.5" fill="url(#g)"/>
<circle cx="5" cy="5" r="2"/><polyline points="0,0 1,1"/><text font-family="Helvetica" font-size="9" text-anchor="end">Total &amp; tax &lt;2026&gt; <tspan font-weight="bold">€ 1.234,50</tspan></text>
<image width="4" height="4" href="data:image/png;base64,iVBORw0KGgo="/></g>
<!-- a comment --><![CDATA[ raw <b> ]]></svg>
<table><tr><td colspan="2" style="text-align:right;font-family:'Courier New'">Cell</td></tr></table>
<p>Line<br>two<img alt="logo" src="data:image/jpeg;base64,/9j/4AAQ"></p><div/>"##;
    let clean = sanitize_markup(page, "");
    for kept in [
        r##"<linearGradient id="g" x1="0" x2="1"><stop offset="0" stop-color="#fff"/>"##,
        r##"<g transform="translate(10 20)" clip-path="url(#c)">"##,
        r##"<path d="M0 0L10 10Z" stroke="#333" stroke-width="0.5" fill="url(#g)"/>"##,
        r#"<circle cx="5" cy="5" r="2"/>"#,
        r#"<text font-family="Helvetica" font-size="9" text-anchor="end">Total &amp; tax &lt;2026&gt; <tspan font-weight="bold">€ 1.234,50</tspan></text>"#,
        r#"<image width="4" height="4" href="data:image/png;base64,iVBORw0KGgo="/>"#,
        " raw &lt;b&gt; ",
        r#"<td colspan="2" style="text-align:right;font-family:&#39;Courier New&#39;">Cell</td>"#,
        r#"<p>Line<br>two<img alt="logo" src="data:image/jpeg;base64,/9j/4AAQ"></p><div></div>"#,
        r#"preserveAspectRatio="xMidYMid meet""#,
    ] {
        assert!(clean.contains(kept), "{kept} lost: {clean}");
    }
    assert!(!clean.contains("comment"), "{clean}");
}

#[test]
fn the_sanitizer_closes_what_it_opens_and_survives_broken_markup() {
    assert_eq!(sanitize_markup("<b><i>x</b>", ""), "<b><i>x</i></b>");
    assert_eq!(sanitize_markup("</div>a < b", ""), "a &lt; b");
    assert_eq!(
        sanitize_markup(r#"<p title="x>y">t</p>"#, ""),
        r#"<p title="x&gt;y">t</p>"#
    );
    assert_eq!(sanitize_markup(r#"<p title="never closed>t"#, ""), "");
    assert_eq!(
        sanitize_markup("<unknown>kept text</unknown>", ""),
        "kept text"
    );
    assert_eq!(
        sanitize_markup(r#"<p title="&quot;><script>x</script>">a</p>"#, ""),
        r#"<p title="&quot;&gt;&lt;script&gt;x&lt;/script&gt;">a</p>"#
    );
    assert_eq!(
        sanitize_markup("<script>if (a < b) { '</div>' }</script>after", ""),
        "after"
    );
    assert_eq!(
        sanitize_markup("<template><img src=x></template><p>ok</p>", ""),
        "<p>ok</p>"
    );
    assert_eq!(
        sanitize_markup(
            r#"<a href="/reports/2?x=1#top">r</a><a href="//evil">e</a><a href="https:x">h</a>"#,
            ""
        ),
        r#"<a href="/reports/2?x=1#top">r</a><a>e</a><a>h</a>"#
    );
    let deep = "<g>".repeat(5_000);
    assert!(sanitize_markup(&format!("<svg>{deep}</svg>"), "").len() < 3_000);
}

#[action(permission = "doc.read")]
pub async fn report_page(p: DocPage) -> Result<String> {
    Ok(match p.document.as_str() {
        "big" => "x".repeat(MAX_PAGE_BYTES + 1),
        "amps" => "&".repeat(MAX_PAGE_BYTES / 2),
        _ => format!(
            r##"<svg viewBox="0 0 100 100"><defs><linearGradient id="g"/></defs><rect fill="url(#g)" width="1" height="1" onclick="x()"/><text>{} page {}</text><script>x()</script></svg>"##,
            p.document, p.page
        ),
    })
}

#[action(permission = "doc.search")]
pub async fn search_report(q: DocSearch) -> Result<Vec<PageHit>> {
    Ok((0..200)
        .map(|i| PageHit::new(i, format!("{} hit {i} <b>{}</b>", q.text, "y".repeat(500))))
        .collect())
}

#[screen(route = "/plain", permission = "doc.read")]
pub fn plain() -> Screen {
    Screen::new("Plain").body(Paragraph::new("Only text"))
}

#[screen(route = "/report", permission = "doc.read")]
pub fn report() -> Screen {
    Screen::new("Report").body(
        Card::new().add(
            DocumentViewer::of(report_page, 500)
                .document("r1")
                .page_size(PageSize::A4)
                .title("Sales report")
                .outline(vec![
                    OutlineItem::new("Summary <b>", 1).child(OutlineItem::new("Totals", 2)),
                    OutlineItem::new("Beyond the end", 900),
                ])
                .search(search_report)
                .download("PDF", "/report.pdf"),
        ),
    )
}

#[screen(route = "/trusted", permission = "doc.read")]
pub fn trusted() -> Screen {
    Screen::new("Trusted").body(
        DocumentViewer::of(report_page, 3)
            .document("t")
            .trusted(),
    )
}

#[screen(route = "/big", permission = "doc.read")]
pub fn big() -> Screen {
    Screen::new("Big").body(DocumentViewer::of(report_page, 2).document("big"))
}

#[screen(route = "/amps", permission = "doc.read")]
pub fn amps() -> Screen {
    Screen::new("Amps").body(DocumentViewer::of(report_page, 2).document("amps"))
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ALL: &str = "doc.read,doc.search";

fn app() -> axum::Router {
    App::new("Docs")
        .i18n(I18n::new("en"))
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h
                .get("x-perms")?
                .to_str()
                .ok()?
                .split(',')
                .map(String::from)
                .collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(
            Feature::new("docs")
                .screen(plain)
                .screen(report)
                .screen(trusted)
                .screen(big)
                .screen(amps),
        )
        .router()
        .unwrap()
}

async fn body(res: axum::response::Response) -> (StatusCode, String) {
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn get_as(router: &axum::Router, path: &str, perms: &str) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(
            Request::get(path)
                .header("x-perms", perms)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    body(res).await
}

/// Posts a call to `/_zeyn/data`, with the CSRF header when `csrf` is true.
async fn post(
    router: &axum::Router,
    perms: &str,
    csrf: bool,
    call: serde_json::Value,
) -> (StatusCode, String) {
    let mut req = Request::post("/_zeyn/data")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"));
    if csrf {
        req = req.header("x-zeyn-csrf", TOKEN);
    }
    let res = router
        .clone()
        .oneshot(
            req.body(Body::from(call.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    body(res).await
}

fn page_call(screen: &str, node: &str, page: serde_json::Value) -> serde_json::Value {
    json!({ "screen": screen, "params": null, "node": node, "key": "page", "input": { "page": page, "viewer": "report-doc-0-0" } })
}

#[tokio::test]
async fn the_viewer_renders_its_toolbar_and_loads_its_module_only_where_used() {
    let router = app();
    let (_, bare) = get_as(&router, "/plain", ALL).await;
    assert!(!bare.contains("/_zeyn/document.js") && !bare.contains("/_zeyn/document.css"));
    let (status, page) = get_as(&router, "/report", ALL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("/_zeyn/document.js?v=") && page.contains("/_zeyn/document.css?v="));
    assert!(
        page.contains(
            r#"data-zeyn-doc data-pages="500" data-width="793.71" data-height="1122.52""#
        )
    );
    assert!(page.contains(r#"aria-label="Sales report""#));
    assert!(
        page.contains(r#"aria-label="Previous page""#) && page.contains(r#"aria-label="Zoom in""#)
    );
    assert!(page.contains(">of 500<") && page.contains(r#"<option value="400">400%</option>"#));
    assert!(page.contains(r#"role="status" aria-live="polite" data-doc="status""#));
    assert!(page.contains(r#"data-page="1">Summary &lt;b&gt;</button>"#));
    assert!(page.contains(r#"data-page="2">Totals</button>"#));
    assert!(
        !page.contains("Beyond the end"),
        "outline entries past the end are left out"
    );
    assert!(page.contains(r#"href="/report.pdf" download"#));
    assert!(page.contains(r#"data-doc="search""#));
    let (_, reader) = get_as(&router, "/report", "doc.read").await;
    assert!(
        !reader.contains(r#"data-doc="search""#),
        "no search without its permission"
    );
    let (status, js) = get_as(&router, "/_zeyn/document.js", "").await;
    assert_eq!(status, StatusCode::OK);
    assert!(js.contains("AbortController") && js.contains("MAX_FETCHES"));
    assert!(!js.contains("location.reload"));
}

#[tokio::test]
async fn pages_come_cleaned_with_their_ids_kept_apart_through_a_nested_path() {
    let router = app();
    let (status, body) = post(&router, ALL, true, page_call("report", "0.0", json!(7))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["page"], 7);
    let html = v["html"].as_str().unwrap();
    assert!(html.contains("<text>r1 page 7</text>"), "{html}");
    assert!(
        html.contains(r#"<linearGradient id="report-doc-0-0-p7-g"/>"#),
        "{html}"
    );
    assert!(
        html.contains(r##"fill="url(#report-doc-0-0-p7-g)""##),
        "{html}"
    );
    let other = json!({ "screen": "report", "params": null, "node": "0.0", "key": "page", "input": { "page": 7, "viewer": "second\"><b>" } });
    let (_, body) = post(&router, ALL, true, other).await;
    assert!(body.contains(r#"id=\"secondb-p7-g\""#), "{body}");
    let bare = json!({ "screen": "report", "params": null, "node": "0.0", "key": "page", "input": { "page": 7 } });
    let (_, body) = post(&router, ALL, true, bare).await;
    assert!(body.contains(r#"id=\"zdoc-p7-g\""#), "{body}");
    assert!(
        !html.contains("script") && !html.contains("onclick"),
        "{html}"
    );
}

#[tokio::test]
async fn page_numbers_outside_the_document_are_refused() {
    let router = app();
    for page in [
        json!(0),
        json!(501),
        json!(-1),
        json!("2"),
        json!(4_294_967_297u64),
    ] {
        let (status, _) = post(&router, ALL, true, page_call("report", "0.0", page.clone())).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{page}");
    }
    let (status, _) = post(&router, ALL, true, page_call("report", "0.0", json!(500))).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn pages_need_csrf_the_screen_and_the_action_permission() {
    let router = app();
    let call = page_call("report", "0.0", json!(1));
    let (status, _) = post(&router, ALL, false, call.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = post(&router, "doc.search", true, call.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = post(&router, "", true, call).await;
    assert!(status == StatusCode::FORBIDDEN || status == StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn trusted_pages_come_as_the_action_wrote_them() {
    let (status, body) = post(&app(), ALL, true, page_call("trusted", "0", json!(3))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let html = v["html"].as_str().unwrap();
    assert!(html.contains(r#"<linearGradient id="g"/>"#) && html.contains("<script>x()</script>"));
}

#[tokio::test]
async fn pages_over_the_size_limit_are_refused_before_and_after_cleaning() {
    let router = app();
    for screen in ["big", "amps"] {
        let (status, body) = post(&router, ALL, true, page_call(screen, "0", json!(1))).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{screen}");
        assert!(body.len() < 10_000 && !body.contains("xxxx") && !body.contains("&amp;&amp;"));
    }
}

#[tokio::test]
async fn searches_are_bounded_and_need_their_permission() {
    let router = app();
    let call = |text: &str| json!({ "screen": "report", "params": null, "node": "0.0", "key": "search", "input": { "text": text } });
    let (status, body) = post(&router, ALL, true, call(" total ")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let hits = v["hits"].as_array().unwrap();
    assert_eq!(hits.len(), MAX_PAGE_HITS);
    assert_eq!(v["more"], true);
    assert_eq!(hits[0]["page"], 1, "page 0 is not in the document");
    assert!(
        hits[0]["text"]
            .as_str()
            .unwrap()
            .starts_with("total hit 1 <b>")
    );
    assert!(hits.iter().all(|h| {
        h["text"]
            .as_str()
            .unwrap()
            .chars()
            .count()
            <= 160
    }));
    let (status, _) = post(&router, ALL, true, call(&"x".repeat(101))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(&router, ALL, true, call("  ")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(&router, "doc.read", true, call("total")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[test]
fn the_sanitizer_takes_linear_time_on_hostile_pages() {
    let two_mb = 2 * 1024 * 1024;
    let urls = "url(#a)".repeat(two_mb / 7);
    let cases = [
        format!(r#"<svg><rect fill="{urls}"/></svg>"#),
        format!(r#"<svg><rect style="fill:{urls}"/></svg>"#),
        "<".repeat(two_mb),
        "</".repeat(two_mb / 2),
        "<!".repeat(two_mb / 2),
        "<g>".repeat(two_mb / 3),
        format!("<svg>{}</svg>", "<g><g/>".repeat(two_mb / 7)),
        format!("<rect {}>", "fill=red ".repeat(two_mb / 9)),
        "<rect x=1 y=2 width=3 height=4 fill=red stroke=blue>".repeat(two_mb / 52),
        "<script>".repeat(two_mb / 8),
        "<object>".repeat(two_mb / 8),
        "&amp;&#x41;&bogus;".repeat(two_mb / 18),
        format!("<a href=\"{}\">", "a".repeat(two_mb)),
        format!("<p title=\"{}", "x".repeat(two_mb)),
        "<use href=\"#a\"/>".repeat(two_mb / 16),
    ];
    for (i, page) in cases.iter().enumerate() {
        let start = std::time::Instant::now();
        let clean = sanitize_markup(page, "p-");
        let took = start.elapsed();
        assert!(
            took < std::time::Duration::from_secs(2),
            "case {i} took {took:?}"
        );
        assert!(clean.len() <= page.len() * 6, "case {i}");
    }
}

#[test]
fn paint_values_follow_a_grammar_of_colors_and_local_references() {
    let paint = |v: &str| sanitize_markup(&format!(r#"<svg><rect fill="{v}"/></svg>"#), "p-");
    for ok in [
        "none",
        "currentColor",
        "transparent",
        "red",
        "#abc",
        "#a1b2c3d4",
        "rgb(10, 20, 30)",
        "rgba(10 20 30 / 50%)",
        "hsl(120deg, 50%, 50%)",
    ] {
        assert_eq!(
            paint(ok),
            format!(r#"<svg><rect fill="{ok}"/></svg>"#),
            "{ok}"
        );
    }
    assert_eq!(
        paint("url(#g) red"),
        r##"<svg><rect fill="url(#p-g) red"/></svg>"##
    );
    assert_eq!(
        paint("URL('#g')"),
        r##"<svg><rect fill="url(#p-g)"/></svg>"##
    );
    for bad in [
        "image-set(x.png 1x)",
        "url(#a) url(#b)",
        "url(https://evil.example/a)",
        "red;background:url(x)",
        "#ggg",
        "rgb(1,2,3) url(#a)",
        "var(--x)",
        "expression(alert(1))",
        "rgb(calc(1))",
        "paint(worklet)",
    ] {
        assert_eq!(paint(bad), "<svg><rect/></svg>", "{bad}");
    }
}

#[test]
fn html_inside_svg_is_unwrapped_and_use_elements_are_capped() {
    assert_eq!(
        sanitize_markup(
            r#"<svg><p>a<b>b</b></p><div><text>t</text></div><a href="/x"><title>x</title></a></svg>"#,
            ""
        ),
        r#"<svg>ab<text>t</text><a href="/x"><title>x</title></a></svg>"#
    );
    let many = r##"<use href="#a"/>"##.repeat(zeyn::ui::MAX_USE_ELEMENTS + 10);
    let clean = sanitize_markup(&format!("<svg>{many}</svg>"), "");
    assert_eq!(clean.matches("<use").count(), zeyn::ui::MAX_USE_ELEMENTS);
}

#[test]
fn the_id_prefix_is_kept_to_safe_characters() {
    assert_eq!(
        sanitize_markup(r#"<p id="x">a</p>"#, "a\"b-<c>"),
        r#"<p id="ab-cx">a</p>"#
    );
}

#[screen(route = "/two", permission = "doc.read")]
pub fn two() -> Screen {
    Screen::new("Two")
        .body(DocumentViewer::of(report_page, 2).document("a"))
        .body(
            DocumentViewer::of(report_page, 2)
                .document("b")
                .download("PDF", "/a.pdf")
                .download("Bad", " javascript:alert(1)")
                .download("Data", "data:text/html,x")
                .download("Web", "https://example.com/r.pdf"),
        )
}

#[tokio::test]
async fn two_viewers_on_one_screen_have_their_own_ids_and_only_safe_downloads() {
    let router = App::new("Two")
        .i18n(I18n::new("en"))
        .auth(|_h: &HeaderMap| {
            Some(Identity::user("u").with_permissions(vec!["doc.read".to_string()]))
        })
        .feature(Feature::new("docs").screen(two))
        .router()
        .unwrap();
    let (status, page) = get_as(&router, "/two", "doc.read").await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = page
        .match_indices(r#"<section class="z-doc" id=""#)
        .map(|(i, m)| {
            let rest = &page[i + m.len()..];
            &rest[..rest.find('"').unwrap()]
        })
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    assert!(page.contains(r#"href="/a.pdf" download"#));
    assert!(page.contains(r#"href="https://example.com/r.pdf" download"#));
    assert!(!page.contains("javascript") && !page.contains("data:text"));
}
