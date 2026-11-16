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
        None => vec![
            TreeItem::branch("sales", "Sales <dept>").hint("2"),
            TreeItem::leaf("hr", "People"),
        ],
        Some("sales") => vec![
            TreeItem::leaf("north", "North"),
            TreeItem::leaf("south", "South"),
        ],
        Some(_) => return Err(Error::NotFound),
    })
}

#[screen(route = "/org", public)]
pub fn org() -> Screen {
    Screen::new("Org").body(Card::new().add(Tree::of(departments).label("Departments")))
}

/// Parents of the folders a movable tree test moves: `b` in `a`, `c` in `b`.
static PARENTS: std::sync::Mutex<Vec<(&str, Option<&str>)>> = std::sync::Mutex::new(Vec::new());

/// The parent of a test folder.
fn parent_of(id: &str) -> Option<String> {
    PARENTS
        .lock()
        .unwrap()
        .iter()
        .find(|p| p.0 == id)
        .and_then(|p| p.1)
        .map(String::from)
}

#[action(permission = "org.write")]
pub async fn move_folder(m: TreeMove) -> Result<()> {
    let parent = m.parent(parent_of)?;
    let mut all = PARENTS.lock().unwrap();
    let parent = parent.and_then(|p| {
        all.iter()
            .find(|f| f.0 == p)
            .map(|f| f.0)
    });
    let folder = all
        .iter_mut()
        .find(|f| f.0 == m.id)
        .ok_or(Error::NotFound)?;
    folder.1 = parent;
    Ok(())
}

#[screen(route = "/folders", public)]
pub fn folders() -> Screen {
    Screen::new("Folders").body(
        Card::new().add(
            Tree::of(departments)
                .label("Folders")
                .movable(move_folder),
        ),
    )
}

#[tokio::test]
async fn a_movable_tree_moves_through_its_action_and_refuses_cycles_on_the_server() {
    *PARENTS.lock().unwrap() = vec![("a", None), ("b", Some("a")), ("c", Some("b"))];
    let html = folders::run().html(&folders::META);
    assert!(html.contains("data-zeyn-tree-move"));
    assert!(html.contains("z-tree-move-keys"));
    let call = |id: &str, target: &str, place: &str| serde_json::json!({ "id": id, "target": target, "place": place });
    let write = "org.read,org.write";
    let (status, _) = data_on("folders", false, write, call("c", "a", "after"), "move").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "CSRF");
    let (status, _) = data_on("folders", true, "org.read", call("c", "a", "after"), "move").await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the move action's permission"
    );
    let (status, _) = data_on("folders", true, write, call("a", "c", "inside"), "move").await;
    assert_eq!(status, StatusCode::CONFLICT, "into its own descendant");
    let (status, _) = data_on("folders", true, write, call("a", "b", "after"), "move").await;
    assert_eq!(status, StatusCode::CONFLICT, "next to its own child");
    let (status, _) = data_on("folders", true, write, call("a", "a", "inside"), "move").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "into itself");
    let (status, _) = data_on("folders", true, write, call("a", "b", "sideways"), "move").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "an unknown place");
    let long = "x".repeat(201);
    let (status, _) = data_on("folders", true, write, call(&long, "a", "after"), "move").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "an id that is too long");
    assert_eq!(parent_of("a"), None);
    let (status, body) = data_on("folders", true, write, call("c", "a", "after"), "move").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(parent_of("c"), None);
    let (status, _) = data_on("folders", true, write, call("a", "c", "inside"), "move").await;
    assert_eq!(status, StatusCode::OK, "c is no longer below a");
    assert_eq!(parent_of("a").as_deref(), Some("c"));
}

#[test]
fn tree_move_parent_stops_on_a_broken_chain() {
    let looped = |id: &str| Some(if id == "p" { "q" } else { "p" }.to_string());
    let m = TreeMove {
        id: "x".into(),
        target: "p".into(),
        place: TreePlace::Inside,
    };
    assert!(m.parent(looped).is_err());
}

#[test]
fn trees_render_an_empty_aria_tree_that_loads_its_top_level() {
    let html = org::run().html(&org::META);
    assert!(html.contains(
        r#"<ul role="tree" aria-label="Departments" data-testid="org.tree" aria-busy="true">"#
    ));
    let schema = org::run().schema(&org::META);
    assert_eq!(schema["body"][0]["children"][0]["type"], "tree");
    assert_eq!(
        schema["body"][0]["children"][0]["action"]["name"],
        "departments"
    );
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn data(perms: &str, input: serde_json::Value, key: &str) -> (StatusCode, serde_json::Value) {
    data_on("org", true, perms, input, key).await
}

async fn data_on(
    screen: &str,
    csrf: bool,
    perms: &str,
    input: serde_json::Value,
    key: &str,
) -> (StatusCode, serde_json::Value) {
    let router = App::new("Test")
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
            Feature::new("org")
                .screen(org)
                .screen(folders),
        )
        .router()
        .unwrap();
    let body = serde_json::json!({ "screen": screen, "node": "0.0", "key": key, "input": input });
    let req = Request::post("/_zeyn/data")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", if csrf { TOKEN } else { "wrong" })
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
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

#[test]
fn a_draggable_tree_marks_its_leaves_and_names_the_keyboard_insert() {
    let mut out = String::new();
    Tree::of(departments)
        .label(r#"Fields <script>alert("x")</script> & 'more'"#)
        .draggable()
        .render(&mut out, &Render::new("s", "en", I18n::builtin()));
    assert!(
        out.starts_with(r#"<div class="z-tree" data-zeyn-tree data-zeyn-tree-drag data-texts=""#),
        "{out}"
    );
    assert!(out.contains(
        r#"aria-label="Fields &lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; &#39;more&#39;""#
    ), "{out}");
    assert!(!out.contains("<script>"), "the label is escaped");
    assert!(out.contains(r#"<span class="z-sr-only z-tree-keys" aria-hidden="true">Control+Enter inserts.</span><span class="z-sr-only" aria-live="polite"></span></div>"#));
    assert!(out.contains("&quot;inserted&quot;:&quot;Inserted {label}.&quot;"));
    assert!(out.contains("&quot;insert_label&quot;:&quot;Insert {label}&quot;"));
    let mut plain = String::new();
    Tree::of(departments).render(&mut plain, &Render::new("s", "tr", I18n::builtin()));
    assert!(!plain.contains("data-zeyn-tree-drag"));
    assert!(
        !plain.contains("aria-live"),
        "a tree that does not drag has no insert"
    );
    assert_eq!(
        Tree::of(departments)
            .draggable()
            .schema()["draggable"],
        true
    );
}

#[action(permission = "org.read")]
pub async fn hostile(_q: TreeQuery) -> Result<Vec<TreeItem>> {
    Ok(vec![TreeItem::leaf(
        r#"a"},{"id":"b</script>"#,
        "<img src=x onerror=alert(1)> \"quoted\" \\ \u{2028}",
    )])
}

#[screen(route = "/fields", public)]
pub fn fields() -> Screen {
    Screen::new("Fields").body(Tree::of(hostile).draggable())
}

#[tokio::test]
async fn hostile_tree_item_ids_and_labels_reach_the_browser_unchanged_as_json() {
    let router = App::new("Test")
        .i18n(I18n::new("en"))
        .auth(|_: &HeaderMap| Some(Identity::user("u").with_permissions(["org.read"])))
        .feature(Feature::new("fields").screen(fields))
        .router()
        .unwrap();
    let body = serde_json::json!({ "screen": "fields", "node": "0", "key": "", "input": {} });
    let req = Request::post("/_zeyn/data")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let items: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(items[0]["id"], r#"a"},{"id":"b</script>"#);
    assert_eq!(
        items[0]["label"],
        "<img src=x onerror=alert(1)> \"quoted\" \\ \u{2028}"
    );
    assert_eq!(items.as_array().unwrap().len(), 1);
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
    assert!(html.contains(r#"<div class="z-chart-plot" role="group" aria-roledescription="grafik" aria-labelledby="s-chart--title s-chart--desc"><p class="z-sr-only" id="s-chart--desc">"#), "{html}");
    assert!(
        html.contains(r#"<svg class="z-chart-svg" viewBox="0 0 640 260" preserveAspectRatio="none" focusable="false">"#),
        "the plot stretches and holds only geometry"
    );
    assert!(
        html.contains(r#"<div class="z-chart-y z-chart-y-start" aria-hidden="true">"#)
            && html.contains(
                r#"<div class="z-chart-x" aria-hidden="true"><span style="left:25.00%">Jan</span>"#
            ),
        "tick labels are HTML beside the plot: {html}"
    );
    assert!(html.contains(
        r#"<figcaption class="z-chart-title" id="s-chart--title">Sales &lt;2026&gt;</figcaption>"#
    ));
    assert!(
        html.contains("2 kategoride 2 seri gösteren sütun grafiği. En yüksek: Jan, Shop, 1.200,5."),
        "{html}"
    );
    assert!(html.contains(r#"aria-label="Feb &lt;b&gt;, Web: 50,0""#));
    assert!(!html.contains("<b>"), "labels are escaped");
    assert!(
        html.contains(r#"<span style="top:0.00%">1.500,0</span>"#)
            && html.contains(r#">500,0</span>"#)
            && html.contains(r#"<span style="top:100.00%">0,0</span>"#),
        "axis goes to a round number"
    );
    let big = render(
        Chart::bar("Revenue")
            .labels(["Q1", "Q2"])
            .series("Revenue", [420_000.0, 610_000.0]),
        "en",
    );
    assert!(
        big.contains(">800K</span>") && !big.contains("z-chart-legend"),
        "ticks are shortened and a single series has no legend: {big}"
    );
    assert!(html.contains(
        r#"<li><span class="z-chart-swatch z-series-1" aria-hidden="true"></span>Web</li>"#
    ));
    assert!(
        html.contains(r#"<tr><th scope="row">Jan</th><td class="z-num">1.200,5</td><td class="z-num">0,0</td></tr>"#),
        "missing numbers count as zero"
    );
}

#[test]
fn line_and_donut_charts_describe_themselves() {
    let line = render(
        Chart::line("Orders")
            .labels(["W1", "W2", "W3"])
            .series("Orders", [4.0, 9.0, 7.0]),
        "en",
    );
    assert!(
        line.contains(r#"<polyline class="z-chart-line z-series-0""#)
            && line
                .matches(r#"<span class="z-chart-pt z-chart-mark"#)
                .count()
                == 3,
        "points are round HTML marks over the stretched line: {line}"
    );
    assert!(line.contains(r#"z-chart-pt-last" style="left:83.33%"#));
    assert!(line.contains("Highest: W2, 9."));
    let donut = render(
        Chart::donut("Units")
            .labels(["A", "B"])
            .series("Count", [30.0, 10.0]),
        "en",
    );
    assert!(
        donut.contains(r#"aria-label="A: 30 (75%)""#)
            && donut.contains(r#"aria-label="B: 10 (25%)""#)
    );
    assert!(
        donut.contains(r#">40</text>"#),
        "the total sits in the middle"
    );
    assert!(donut.contains("Donut chart of 2 parts. Largest: A, 30."));
}

/// Every chart kind with its data, for the checks that apply to all of them.
fn every_kind() -> Vec<(&'static str, Chart)> {
    let months = ["Jan", "Feb", "Mar"];
    vec![
        (
            "bar",
            Chart::bar("Sales")
                .labels(months)
                .series("Shop", [3.0, 5.0, 4.0])
                .series("Web", [1.0, 2.0, 6.0]),
        ),
        (
            "stacked",
            Chart::stacked_bar("Sales")
                .labels(months)
                .series("Shop", [3.0, 5.0, 4.0])
                .series("Web", [1.0, 2.0, 6.0]),
        ),
        (
            "line",
            Chart::line("Orders")
                .labels(months)
                .series("Orders", [3.0, 5.0, 4.0]),
        ),
        (
            "area",
            Chart::area("Orders")
                .labels(months)
                .series("Orders", [3.0, 5.0, 4.0])
                .series("Returns", [1.0, 1.0, 2.0]),
        ),
        (
            "combo",
            Chart::combo("Sales")
                .labels(months)
                .series("Sales", [300.0, 500.0, 400.0])
                .line_series("Margin", [12.0, 18.0, 9.0])
                .right_axis(),
        ),
        (
            "scatter",
            Chart::scatter("Price and units")
                .points("Tea", [(1.5, 40.0), (2.5, 22.0)])
                .points("Coffee", [(4.0, 12.0)]),
        ),
        (
            "donut",
            Chart::donut("Units")
                .labels(["A", "B"])
                .series("Count", [30.0, 10.0]),
        ),
        (
            "gauge",
            Chart::gauge("Load")
                .value(72.0)
                .range(0.0, 120.0)
                .target(90.0),
        ),
        (
            "funnel",
            Chart::funnel("Sales funnel")
                .labels(["Visits", "Carts", "Orders"])
                .series("People", [1000.0, 300.0, 120.0]),
        ),
    ]
}

#[test]
fn every_chart_kind_has_a_summary_a_table_and_one_tab_stop_over_named_marks() {
    for (name, chart) in every_kind() {
        let html = render(chart, "en");
        assert!(
            html.contains("<desc id=")
                || html.contains(r#"<p class="z-sr-only" id="s-chart--desc">"#),
            "{name}: summary"
        );
        assert!(
            !html.contains("{"),
            "{name}: every placeholder is filled: {html}"
        );
        assert!(
            html.contains(r#"<table class="z-chart-table">"#),
            "{name}: data table"
        );
        assert!(
            html.contains(r#"<div class="z-chart-tip" aria-hidden="true" hidden></div>"#),
            "{name}: tooltip"
        );
        assert_eq!(
            html.matches(r#"tabindex="0""#).count(),
            1,
            "{name}: one tab stop"
        );
        let marks = html.matches("z-chart-mark").count();
        assert!(marks >= 1, "{name}: marks");
        assert_eq!(
            html.matches(r#"role="img" aria-label="#)
                .count(),
            marks,
            "{name}: every mark is named"
        );
        assert!(
            !html.contains("<title>"),
            "{name}: no native tooltips that would be read twice"
        );
    }
}

#[test]
fn new_chart_kinds_say_what_they_show() {
    let all: std::collections::HashMap<_, _> = every_kind()
        .into_iter()
        .map(|(n, c)| (n, render(c, "en")))
        .collect();
    assert!(
        all["stacked"].contains("Highest total: Mar, 10."),
        "{}",
        all["stacked"]
    );
    assert!(all["stacked"].contains(r#"aria-label="Mar, Web: 6""#));
    assert!(all["area"].contains(r#"<polygon class="z-chart-area z-series-0""#));
    assert!(all["combo"].contains(r#"<polyline class="z-chart-line z-series-1""#));
    assert!(
        all["combo"].contains(r#"<div class="z-chart-y z-chart-y-end" aria-hidden="true">"#)
            && all["combo"].contains(">20</span>"),
        "the line has its own axis on the right"
    );
    assert!(all["combo"].contains("z-chart-swatch-line"));
    assert!(
        all["scatter"].contains(r#"aria-label="Tea: 2, 22""#)
            || all["scatter"].contains(r#"aria-label="Tea: 3, 22""#)
    );
    assert!(all["scatter"].contains("Scatter chart of 3 points in 2 groups."));
    assert!(all["gauge"].contains("Gauge at 72, between 0 and 120. Target: 90."));
    assert!(
        all["gauge"].contains(r#"aria-label="72 of 120""#)
            && all["gauge"].contains("z-chart-target")
    );
    assert!(all["funnel"].contains(r#"aria-label="Carts: 300 (30%)""#));
    let tr = render(
        Chart::gauge("Yük")
            .value(1234.5)
            .decimals(1)
            .range(0.0, 2000.0),
        "tr",
    );
    assert!(tr.contains("1.234,5"));
}

#[test]
fn kpi_cards_show_value_direction_target_and_trend() {
    let mut out = String::new();
    Kpi::new("Wait <min>", 12.0)
        .unit("min")
        .delta(-8.0)
        .lower_is_better()
        .target(10.0)
        .trend([15.0, 14.0, 12.0])
        .render(&mut out, &Render::new("s", "en", I18n::builtin()));
    assert!(out.contains(r#"role="group" aria-labelledby="s-kpi--title""#));
    assert!(out.contains("Wait &lt;min&gt;"));
    assert!(out.contains(r#"<p class="z-kpi-delta"><span class="z-kpi-chip z-kpi-good" aria-hidden="true"><span class="z-icon z-icon-trend-down" aria-hidden="true"></span>−8.0%</span> <span class="z-kpi-compare" aria-hidden="true">vs previous period</span><span class="z-sr-only">Down 8.0% from the previous period</span></p>"#), "{out}");
    assert!(
        out.contains(r#"<div class="z-kpi-trend z-kpi-good"><div class="z-kpi-spark-box"><svg class="z-kpi-spark" viewBox="0 0 100 40" preserveAspectRatio="none""#),
        "the trend spans the card in the delta's tone: {out}"
    );
    assert!(out.contains("Target 10, 120% reached"));
    assert!(out.contains("Trend of 3 values from 15 to 12, lowest 12, highest 15"));
    let mut tr = String::new();
    Kpi::new("Ciro", 1500.0)
        .delta(4.3)
        .render(&mut tr, &Render::new("s", "tr", I18n::builtin()));
    assert!(
        tr.contains("1.500")
            && tr.contains("Önceki döneme göre yüzde 4,3 artış")
            && tr.contains("z-kpi-good")
    );
}

#[test]
fn kpi_money_shows_the_currency_in_the_users_locale() {
    let render = |k: Kpi, locale: &str| {
        let mut out = String::new();
        k.render(&mut out, &Render::new("s", locale, I18n::builtin()));
        out
    };
    let en = render(
        Kpi::money("Revenue", Money::tl(18_425_050)).target(200_000.0),
        "en",
    );
    assert!(
        en.contains(r#"<p class="z-kpi-value"><span class="z-kpi-unit">₺</span>184,250.50</p>"#),
        "{en}"
    );
    assert!(en.contains("Target ₺200,000.00, 92% reached"), "{en}");
    let tr = render(Kpi::money("Ciro", Money::tl(-1_050)), "tr");
    assert!(
        tr.contains(r#"-10,50 <span class="z-kpi-unit">₺</span>"#),
        "{tr}"
    );
    let big = render(Kpi::money("Revenue", Money::tl(123_456_789)), "en");
    assert!(
        big.contains(r#"<p class="z-kpi-value" title="₺1,234,567.89"><span aria-hidden="true"><span class="z-kpi-unit">₺</span>1.23<span class="z-kpi-unit">M</span></span><span class="z-sr-only">₺1,234,567.89</span></p>"#),
        "seven digits and more are shortened, with the full value for screen readers: {big}"
    );
    let flat = render(Kpi::new("Orders", 10.0).delta(0.2), "en");
    assert!(
        flat.contains("z-kpi-chip z-kpi-neutral"),
        "under half a percent is neutral"
    );
    let schema = Kpi::money("Revenue", Money::tl(100)).schema();
    assert_eq!(schema["currency"], "Try");
    assert_eq!(schema["value"], 1.0);
}

#[test]
fn kpis_share_the_height_of_their_row_and_wrap_four_two_one() {
    let css = include_str!("../assets/charts.css");
    assert!(css.contains(".z-col > .z-node:has(> .z-kpi) { height: 100%; }"));
    assert!(css.contains(".z-autogrid > .z-node:has(> .z-kpi)"));
    assert!(css.contains("align-items: stretch"));
    assert!(css.contains("margin-top: auto; padding-top: var(--z-s-2)"));
    for basis in [
        "calc((100% - 3 * var(--z-gap)) / 4 - 0.1px)",
        "calc((100% - var(--z-gap)) / 2 - 0.1px)",
        ".z-kpi-trend { display: none; }",
    ] {
        assert!(css.contains(basis), "{basis}");
    }
    let components = include_str!("../assets/components.css");
    assert!(
        components.contains(
            ".z-chart-svg { display: block; width: 100%; height: auto; overflow: visible; }"
        ),
        "charts fill the width of their container"
    );
    assert!(components.contains(".z-chart-round { max-height: 18rem; }"));
    let mut donut = String::new();
    Chart::donut("Units")
        .labels(["A"])
        .series("N", [1.0])
        .render(&mut donut, &Render::english("s"));
    assert!(donut.contains(r#"<svg class="z-chart-svg z-chart-round""#));
    let mut bar = String::new();
    Chart::bar("Sales")
        .labels(["A"])
        .series("N", [1.0])
        .render(&mut bar, &Render::english("s"));
    assert!(bar.contains(r#"<svg class="z-chart-svg" viewBox"#));
}

#[test]
fn sparkline_and_bullet_cells_are_small_named_svgs() {
    use zeyn::CellRenderer;
    let r = Render::english("s");
    let spark = zeyn::ui::SparklineCell::new().render(&vec![1.0, f64::NAN, 4.0], &r);
    assert!(spark.starts_with(r#"<svg class="z-spark" viewBox="0 0 80 20" role="img" aria-label="Trend of 3 values from 1 to 4, lowest 0, highest 4">"#), "{spark}");
    assert_eq!(zeyn::ui::SparklineCell::new().render(&Vec::new(), &r), "");
    let bullet = zeyn::ui::BulletCell::new(80.0, 100.0).render(&Money::tl(9_000), &r);
    assert!(
        bullet.contains(r#"aria-label="₺90.00, target 80""#),
        "{bullet}"
    );
    assert!(bullet.contains(r#"<rect class="z-bullet-bar" y="3" width="90.0""#));
    assert!(bullet.contains(r#"x="79.2""#));
    let over = zeyn::ui::BulletCell::new(80.0, 100.0).render(&500_i64, &r);
    assert!(
        over.contains(r#"width="100.0""#),
        "values over the maximum are capped"
    );
}

#[test]
fn sparkline_and_bullet_cells_render_ten_thousand_rows_quickly() {
    use zeyn::CellRenderer;
    let r = Render::english("s");
    let spark = zeyn::ui::SparklineCell::new();
    let bullet = zeyn::ui::BulletCell::new(700.0, 1000.0);
    let rows: Vec<Vec<f64>> = (0..10_000)
        .map(|i| {
            (0..12)
                .map(|k| ((i * 7 + k * 13) % 100) as f64)
                .collect()
        })
        .collect();
    let started = std::time::Instant::now();
    let mut bytes = 0;
    for (i, values) in rows.iter().enumerate() {
        bytes += spark.render(values, &r).len();
        bytes += bullet
            .render(&(i as i64 % 1000), &r)
            .len();
    }
    let took = started.elapsed();
    println!(
        "10,000 rows of sparkline and bullet cells: {took:?}, {} KB of HTML",
        bytes / 1024
    );
    assert!(took.as_millis() < 2_000, "10,000 rows took {took:?}");
    assert!(bytes / 10_000 < 1_200, "about a kilobyte per row");
}

/// The hex color of `token` in the first block of `css` after `from`.
fn token(css: &str, from: &str, token: &str) -> (f64, f64, f64) {
    let start = css.find(from).unwrap();
    let at = start
        + css[start..]
            .find(&format!("{token}:"))
            .unwrap()
        + token.len()
        + 1;
    let hex = css[at..]
        .trim_start()
        .trim_start_matches('#');
    let c = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap() as f64 / 255.0;
    (c(0), c(2), c(4))
}

fn contrast(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let lin = |c: f64| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let l = |(r, g, b): (f64, f64, f64)| 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
    let (x, y) = (l(a), l(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[test]
fn chart_colors_reach_three_to_one_in_both_themes() {
    let components = include_str!("../assets/components.css");
    let zeyn = include_str!("../assets/zeyn.css");
    for (theme, chart_from, surface_from) in [
        ("light", ":root", ":root"),
        (
            "dark",
            "prefers-color-scheme: dark",
            "prefers-color-scheme: dark",
        ),
        (
            "dark (chosen)",
            ":root[data-theme=\"dark\"] {",
            ":root[data-theme=\"dark\"] {",
        ),
    ] {
        for surface in ["--z-surface", "--z-surface-2"] {
            let bg = token(zeyn, surface_from, surface);
            for n in 1..=6 {
                let fg = token(components, chart_from, &format!("--z-chart-{n}"));
                let ratio = contrast(fg, bg);
                assert!(
                    ratio >= 3.0,
                    "{theme}: --z-chart-{n} on {surface} is {ratio:.2}:1"
                );
            }
        }
    }
}
