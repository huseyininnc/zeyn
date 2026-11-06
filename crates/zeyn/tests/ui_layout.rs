//! Layout containers and nested node paths: rendering, schema, endpoints and access checks.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::{Arc, RwLock};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::NodePath;
use zeyn::{I18n, Identity};

#[entity(table = "items")]
pub struct Item {
    #[key]
    pub id: ItemId,
    #[rule(required, max_len = 20)]
    pub code: String,
}

#[derive(Clone, Default)]
pub struct Store(Arc<RwLock<Vec<Item>>>);

impl Service for Store {}

#[action(permission = "items.read")]
pub async fn search_items(q: DataQuery, store: Store) -> Result<Page<Item>> {
    Ok(q.apply(store.0.read().unwrap().clone()))
}

#[action(permission = "items.write")]
pub async fn save_item(item: Valid<Item>, store: Store) -> Result<Item> {
    store
        .0
        .write()
        .unwrap()
        .push(item.clone().into_inner());
    Ok(item.into_inner())
}

fn grid() -> Node {
    DataGrid::of(search_items)
        .paging(Paging::pages(5))
        .column(Item::code)
        .into()
}

#[screen(route = "/layout", permission = "items.read")]
pub fn layout() -> Screen {
    Screen::new("Layout")
        .body(
            Panel::new("Intro")
                .collapsible()
                .add(Card::new().title("Note")),
        )
        .body(
            Tabs::new("Sections")
                .tab(
                    "DataGrid",
                    Row::new()
                        .col(grid())
                        .weight(2)
                        .col(Stack::new().add(Card::new())),
                )
                .tab(
                    "Form",
                    Form::of(Item::default())
                        .field(Item::code)
                        .submit("Save", save_item),
                )
                .tab(
                    "Later",
                    Split::new(grid(), Card::new().title("Detail")).ratio(30),
                ),
        )
}

fn app() -> Router {
    let rows = (1..=12)
        .map(|i| Item {
            id: ItemId(i),
            code: format!("I-{i:02}"),
        })
        .collect();
    App::new("Test")
        .i18n(I18n::new("en"))
        .service(Store(Arc::new(RwLock::new(rows))))
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
        .feature(Feature::new("items").screen(layout))
        .router()
        .unwrap()
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn send(router: &Router, req: Request<Body>) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(req)
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

fn get(path: &str, perms: &str) -> Request<Body> {
    Request::get(path)
        .header("x-perms", perms)
        .body(Body::empty())
        .unwrap()
}

fn post(path: &str, perms: &str, body: serde_json::Value) -> Request<Body> {
    Request::post(path)
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap()
}

#[test]
fn containers_write_nested_node_paths_and_only_the_first_tab() {
    let html = layout::run().html(&layout::META);
    for path in ["0", "0.0", "1", "1.0", "1.0.0", "1.0.1", "1.0.1.0"] {
        assert!(
            html.contains(&format!(r#"data-zeyn-node="{path}""#)),
            "missing node {path}"
        );
    }
    assert!(
        !html.contains(r#"data-zeyn-node="1.1""#),
        "closed tabs are not rendered"
    );
    assert!(html.contains(r#"data-zeyn-lazy="1.1""#));
    assert!(html.contains(r#"role="tablist" aria-label="Sections""#));
    assert!(
        html.contains(r#"role="tab" class="z-tab" id="layout-tabs-1-tab-0" aria-selected="true""#)
    );
    assert!(html.contains(r#"aria-expanded="true" aria-controls="layout-panel-0-body""#));
    assert!(html.contains(r#"style="--z-flex:2""#));
}

#[test]
fn screen_resolves_paths_and_walks_only_rendered_nodes() {
    let screen = layout::run();
    let path = |s: &str| NodePath::parse(s).unwrap();
    assert!(
        screen
            .node(&path("1.0.0"))
            .unwrap()
            .grid()
            .is_some()
    );
    assert!(
        screen
            .node(&path("1.1"))
            .unwrap()
            .form()
            .is_some()
    );
    assert!(
        screen
            .node(&path("1.2.0"))
            .unwrap()
            .grid()
            .is_some()
    );
    assert!(screen.node(&path("1.9")).is_none());
    assert!(screen.node(&path("0.0.0")).is_none());
    let walked: Vec<String> = screen
        .walk()
        .into_iter()
        .map(|(p, _)| p.to_string())
        .collect();
    assert_eq!(
        walked,
        ["0", "0.0", "1", "1.0", "1.0.0", "1.0.1", "1.0.1.0"]
    );
    let below: Vec<String> = screen
        .walk_from(&path("1.2"))
        .into_iter()
        .map(|(p, _)| p.to_string())
        .collect();
    assert_eq!(below, ["1.2", "1.2.0", "1.2.1"]);
}

#[test]
fn node_paths_are_checked() {
    for bad in ["", "1..2", "a", "1.-2", "1.2.", &"1.".repeat(20)] {
        assert!(NodePath::parse(bad).is_none(), "{bad:?} must be refused");
    }
    assert_eq!(
        NodePath::parse("01.2")
            .unwrap()
            .as_str(),
        "1.2"
    );
}

#[test]
fn schema_describes_every_container() {
    let schema = layout::run().schema(&layout::META);
    let body = &schema["body"];
    assert_eq!(body[0]["type"], "panel");
    assert_eq!(body[0]["collapsible"], true);
    assert_eq!(body[0]["children"][0]["type"], "card");
    assert_eq!(body[1]["type"], "tabs");
    assert_eq!(body[1]["tabs"][0]["node"]["type"], "row");
    assert_eq!(body[1]["tabs"][0]["node"]["columns"][0]["weight"], 2);
    assert_eq!(body[1]["tabs"][2]["node"]["type"], "split");
    assert_eq!(body[1]["tabs"][2]["node"]["ratio"], 30);
}

#[tokio::test]
async fn page_preloads_grids_inside_containers_and_links_the_component_assets() {
    let (status, html) = send(&app(), get("/layout", "items.read")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        html.contains("I-01"),
        "the grid in the first tab has its first page"
    );
    assert!(!html.contains("I-06"), "page size is 5");
    assert!(html.contains(r#"href="/_zeyn/components.css?v="#));
    assert!(html.contains(r#"src="/_zeyn/components.js?v="#));
}

#[tokio::test]
async fn component_assets_are_served_without_sign_in() {
    let router = app();
    for path in ["/_zeyn/components.js", "/_zeyn/components.css"] {
        let req = Request::get(path)
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(&router, req).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!body.is_empty());
    }
}

#[tokio::test]
async fn grid_and_form_endpoints_accept_nested_paths_and_plain_indexes() {
    let router = app();
    let call = |node: serde_json::Value| {
        json_body(
            "layout",
            node,
            serde_json::json!({ "page": { "type": "offset", "offset": 5, "limit": 5 } }),
        )
    };
    let (status, body) = send(
        &router,
        post(
            "/_zeyn/grid",
            "items.read",
            call(serde_json::json!("1.0.0")),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("I-06"));
    let (status, _) = send(
        &router,
        post("/_zeyn/grid", "items.read", call(serde_json::json!(1))),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "node 1 is the tabs, not a grid"
    );
    let (status, _) = send(
        &router,
        post("/_zeyn/grid", "items.read", call(serde_json::json!("1..0"))),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    let form =
        serde_json::json!({ "screen": "layout", "node": "1.1", "values": { "code": "NEW" } });
    let (status, _) = send(&router, post("/_zeyn/form", "items.read", form.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "saving needs items.write");
    let (status, body) = send(&router, post("/_zeyn/form", "items.read,items.write", form)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

fn json_body(screen: &str, node: serde_json::Value, query: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "screen": screen, "node": node, "query": query })
}

#[tokio::test]
async fn lazy_tab_panels_load_through_the_node_endpoint_with_access_checks() {
    let router = app();
    let body = serde_json::json!({ "screen": "layout", "node": "1.2" });
    let (status, html) = send(&router, post("/_zeyn/node", "items.read", body.clone())).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.starts_with(r#"<div class="z-node" data-zeyn-node="1.2">"#));
    assert!(html.contains(r#"data-zeyn-node="1.2.0""#));
    assert!(
        html.contains("I-01"),
        "grids in a lazy panel come with their first page"
    );
    assert!(html.contains(r#"role="separator""#));

    let (status, _) = send(&router, post("/_zeyn/node", "", body.clone())).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the screen's permission is checked"
    );
    let (status, _) = send(
        &router,
        post(
            "/_zeyn/node",
            "items.read",
            serde_json::json!({ "screen": "layout", "node": "7" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let no_csrf = Request::post("/_zeyn/node")
        .header("x-perms", "items.read")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, _) = send(&router, no_csrf).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "posts need the CSRF token");
}

#[screen(route = "/orders-head", public)]
pub fn orders_head() -> Screen {
    header_screen()
}

/// The screen of `orders_head`, built where the test can call it.
fn header_screen() -> Screen {
    Screen::new("Orders <all>")
        .subtitle("Every order of the last <90> days.")
        .primary(|t| {
            t.button("New order")
                .icon(Icon::Plus)
                .shortcut("Alt+N")
        })
        .toolbar(|t| t.export(Export::Csv))
}

#[test]
fn a_screen_header_has_the_title_a_subtitle_and_primary_actions_in_one_row() {
    let meta = <orders_head as ScreenDef>::META;
    let html = header_screen().html(&meta);
    assert!(
        html.contains(
            r#"aria-labelledby="orders_head-title" aria-describedby="orders_head-subtitle">"#
        ),
        "{html}"
    );
    assert!(html.contains(r#"<header class="z-screen-head"><div class="z-screen-heading"><h1 id="orders_head-title">Orders &lt;all&gt;</h1><p class="z-screen-subtitle" id="orders_head-subtitle">Every order of the last &lt;90&gt; days.</p></div><div class="z-toolbar z-screen-primary" role="toolbar" aria-label="Main actions">"#), "{html}");
    assert!(
        html.contains(r#"<button type="button" class="z-button z-primary" data-testid="orders_head.primary.new-order" aria-keyshortcuts="Alt+N" aria-describedby="orders_head-tooltip-0""#),
        "{html}"
    );
    let head_end = html.find("</header>").unwrap();
    let tools = html
        .find(r#"data-testid="orders_head.toolbar.export-csv""#)
        .unwrap();
    assert!(tools > head_end, "the toolbar stays under the header");
    let schema = header_screen().schema(&meta);
    assert_eq!(schema["subtitle"], "Every order of the last <90> days.");
    assert_eq!(schema["primary"][0]["label"], "New order");
    let plain = Screen::new("Plain").html(&meta);
    assert!(
        plain.contains(
            r#"aria-labelledby="orders_head-title"><h1 id="orders_head-title">Plain</h1>"#
        ),
        "a screen without a header keeps its markup"
    );
    assert!(!plain.contains("z-screen-head"));
    let window = Screen::window("Edit")
        .subtitle("One order")
        .html(&meta);
    assert!(window.contains(r#"<header class="z-screen-head"><div class="z-screen-heading"><h2 id="orders_head-title">Edit</h2>"#));
}

#[test]
fn a_card_puts_its_action_in_the_title_row_and_its_footer_last() {
    let meta = <orders_head as ScreenDef>::META;
    let html = Screen::new("Cards")
        .body(
            Card::new()
                .title("Latest")
                .action(Badge::new("3").count())
                .add("Body")
                .footer("More"),
        )
        .html(&meta);
    let head = html
        .find(r#"<header class="z-card-head"><h3 class="z-card-title""#)
        .expect("title row");
    let action = html
        .find(r#"<div class="z-card-action"><div class="z-node" data-zeyn-node="0.0">"#)
        .expect("the action keeps its node path");
    let body = html
        .find(r#"data-zeyn-node="0.1">Body"#)
        .expect("body");
    let foot = html
        .find(r#"<footer class="z-card-foot"><div class="z-node" data-zeyn-node="0.2">More"#)
        .expect("footer");
    assert!(head < action && action < body && body < foot, "{html}");
}

#[test]
fn two_or_more_exports_fold_into_one_export_menu_button() {
    let meta = <orders_head as ScreenDef>::META;
    let html = Screen::new("Orders")
        .toolbar(|t| {
            t.export(Export::Csv)
                .export(Export::Xlsx)
        })
        .html(&meta);
    assert!(html.contains(r#"<div class="z-menu-button"><button type="button" class="z-button z-menu-trigger" id="orders_head-toolbar-export-button" aria-haspopup="menu" aria-expanded="false" aria-controls="orders_head-toolbar-export" data-testid="orders_head.toolbar.export"><span class="z-icon z-icon-download" aria-hidden="true"></span><span>Export</span>"#), "{html}");
    assert!(html.contains(r#"<li role="none"><button type="button" class="z-menu-entry" role="menuitem" tabindex="-1" data-zeyn-export="csv" data-testid="orders_head.toolbar.export-csv">CSV</button></li><li role="none"><button type="button" class="z-menu-entry" role="menuitem" tabindex="-1" data-zeyn-export="xlsx" data-testid="orders_head.toolbar.export-xlsx">Excel</button></li></ul></div>"#), "{html}");
    assert_eq!(html.matches("z-menu-trigger").count(), 1);
}
