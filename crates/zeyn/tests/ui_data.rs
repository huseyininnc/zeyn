//! Tree view and charts: markup, lazy children through the action's permission, chart summaries.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Component, I18n, Identity, Render};

#[action(permission = "org.read")]
pub async fn departments(q: TreeQuery) -> Result<Vec<TreeItem>> {
    Ok(match q.parent.as_deref() {
        None => vec![TreeItem::branch("sales", "Sales <dept>").hint("2"), TreeItem::leaf("hr", "People")],
        Some("sales") => vec![TreeItem::leaf("north", "North"), TreeItem::leaf("south", "South")],
        Some(_) => return Err(Error::NotFound),
    })
}

#[screen(route = "/org", public)]
pub fn org() -> Screen {
    Screen::new("Org").body(Card::new().add(Tree::of(departments).label("Departments")))
}

#[test]
fn trees_render_an_empty_aria_tree_that_loads_its_top_level() {
    let html = org::run().html(&org::META);
    assert!(html.contains(r#"<ul role="tree" aria-label="Departments" data-testid="org.tree" aria-busy="true">"#));
    let schema = org::run().schema(&org::META);
    assert_eq!(schema["body"][0]["children"][0]["type"], "tree");
    assert_eq!(schema["body"][0]["children"][0]["action"]["name"], "departments");
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn data(perms: &str, input: serde_json::Value, key: &str) -> (StatusCode, serde_json::Value) {
    let router = App::new("Test")
        .i18n(I18n::new("en"))
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h.get("x-perms")?.to_str().ok()?.split(',').map(String::from).collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(Feature::new("org").screen(org))
        .router()
        .unwrap();
    let body = serde_json::json!({ "screen": "org", "node": "0.0", "key": key, "input": input });
    let req = Request::post("/_zeyn/data")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn tree_children_load_through_the_actions_permission() {
    let (status, roots) = data("org.read", serde_json::json!({ "parent": null }), "").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        roots[0],
        serde_json::json!({ "id": "sales", "label": "Sales <dept>", "has_children": true, "hint": "2" })
    );
    let (_, children) = data("org.read", serde_json::json!({ "parent": "sales" }), "").await;
    assert_eq!(children.as_array().unwrap().len(), 2);
    let (status, _) = data("org.read", serde_json::json!({ "parent": "nope" }), "").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = data("", serde_json::json!({}), "").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = data("org.read", serde_json::json!({}), "other").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

fn render(chart: Chart, locale: &str) -> String {
    let mut out = String::new();
    chart.render(&mut out, &Render::new("s", locale, I18n::builtin()));
    out
}

#[test]
fn bar_charts_are_accessible_svg_with_a_legend_and_a_data_table() {
    let html = render(
        Chart::bar("Sales <2026>")
            .labels(["Jan", "Feb <b>"])
            .series("Shop", [1200.5, 204.0])
            .series("Web", [f64::NAN, 50.0])
            .decimals(1),
        "tr",
    );
    assert!(html.contains(r#"<svg class="z-chart-svg" viewBox="0 0 640 260" role="img" aria-labelledby="s-chart--title s-chart--desc" focusable="false">"#));
    assert!(html.contains(r#"<figcaption class="z-chart-title" id="s-chart--title">Sales &lt;2026&gt;</figcaption>"#));
    assert!(html.contains("2 kategoride 2 seri gösteren sütun grafiği. En yüksek: Jan, Shop, 1.200,5."), "{html}");
    assert!(html.contains("<title>Feb &lt;b&gt;, Web: 50,0</title>"));
    assert!(!html.contains("<b>"), "labels are escaped");
    assert!(
        html.contains(r#">1.500,0</text>"#) && html.contains(r#">500,0</text>"#) && html.contains(r#">0,0</text>"#),
        "axis goes to a round number"
    );
    assert!(html.contains(r#"<li><span class="z-chart-swatch z-series-1" aria-hidden="true"></span>Web</li>"#));
    assert!(
        html.contains(r#"<tr><th scope="row">Jan</th><td class="z-num">1.200,5</td><td class="z-num">0,0</td></tr>"#),
        "missing numbers count as zero"
    );
}

#[test]
fn line_and_donut_charts_describe_themselves() {
    let line = render(Chart::line("Orders").labels(["W1", "W2", "W3"]).series("Orders", [4.0, 9.0, 7.0]), "en");
    assert!(line.contains(r#"<polyline class="z-chart-line z-series-0""#) && line.matches("<circle").count() == 3);
    assert!(line.contains("Highest: W2, 9."));
    let donut = render(Chart::donut("Units").labels(["A", "B"]).series("Count", [30.0, 10.0]), "en");
    assert!(donut.contains("<title>A: 30 (75%)</title>") && donut.contains("<title>B: 10 (25%)</title>"));
    assert!(donut.contains(r#">40</text>"#), "the total sits in the middle");
    assert!(donut.contains("Donut chart of 2 parts. Largest: A, 30."));
}
