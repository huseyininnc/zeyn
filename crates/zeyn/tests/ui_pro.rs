//! Desktop class shell: commands and the palette, shortcut checks, menu bar, toolbox, property
//! inspector, status bar and docking panels.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{DockArea, DockLayout};
use zeyn::{I18n, Identity, MemoryViewStore, normalize_shortcut};

#[derive(Clone, Default)]
pub struct Counter(Arc<AtomicUsize>);

impl Service for Counter {}

#[params]
pub struct OrderParams {
    pub id: Option<i64>,
}

#[screen(route = "/orders", permission = "orders.read")]
pub fn orders() -> Screen {
    Screen::new("Orders")
}

#[screen(permission = "orders.write")]
pub fn order_form(_p: OrderParams) -> Screen {
    Screen::window("Order")
}

#[action(permission = "orders.admin")]
pub async fn archive_orders(days: u32, counter: Counter) -> Result<()> {
    counter.0.fetch_add(days as usize, Ordering::SeqCst);
    Ok(())
}

fn new_order() -> Command {
    Command::open("orders.new", "New order", order_form, OrderParams { id: None })
        .shortcut("Ctrl+Alt+N")
        .icon(Icon::Plus)
}

fn archive() -> Command {
    Command::action("orders.archive", "Archive old orders", archive_orders, 30).shortcut("Ctrl+Alt+A")
}

fn hello() -> Command {
    Command::effect("orders.hello", "Say hello", Effect::Toast { message: "Hello".into() })
}

#[model]
pub struct Shape {
    #[rule(required, max_len = 10)]
    pub name: String,
    #[rule(min = 1, max = 100)]
    pub width: i64,
}

#[action(permission = "orders.write")]
pub async fn save_shape(shape: Valid<Shape>, counter: Counter) -> Result<Shape> {
    counter.0.fetch_add(1, Ordering::SeqCst);
    Ok(shape.into_inner())
}

#[screen(route = "/designer", permission = "orders.read")]
pub fn designer() -> Screen {
    Screen::new("Designer")
        .body(
            MenuBar::new("Main")
                .menu("File", |m| m.separator().command(new_order()).separator().command(archive()).separator())
                .menu("Admin", |m| m.command(archive()))
                .menu("Help", |m| m.command(hello())),
        )
        .body(
            Row::new()
                .col(
                    Toolbox::new("Tools")
                        .tool("select", "Select", Some(Icon::Search))
                        .tool("text", "Text", None)
                        .selected("text"),
                )
                .col(PropertyInspector::new(
                    "Properties",
                    Form::of(Shape { name: "A".into(), width: 5 })
                        .section("General")
                        .field(Shape::name)
                        .section("Size")
                        .field(Shape::width)
                        .submit("Apply", save_shape),
                )),
        )
        .body(StatusBar::new("Status").left("Ready").live("Pick a tool").right("100%"))
}

fn denied(_: &Ctx) -> GuardResult {
    GuardResult::Deny("No".into())
}

#[screen(route = "/work", permission = "orders.read")]
pub fn work() -> Screen {
    Screen::new("Work").body(
        Stack::new().add(Card::new().title("Intro")).add(
            Dock::new("Workspace")
                .left("files", "Files", Card::new().title("Tree"))
                .center("main", "Main", Card::new().title("Editor"))
                .center("notes", "Notes", Card::new())
                .bottom("log", "Log", Card::new())
                .sizes(30, 5, 99),
        ),
    )
}

fn app(counter: Counter) -> Router {
    app_with(counter, true)
}

fn app_with(counter: Counter, views: bool) -> Router {
    let app = App::new("Test").i18n(I18n::new("en")).service(counter).auth(|h: &HeaderMap| {
        let perms: Vec<String> = h.get("x-perms")?.to_str().ok()?.split(',').map(String::from).collect();
        let user = h.get("x-user").and_then(|v| v.to_str().ok()).unwrap_or("u").to_string();
        Some(Identity::user(user).with_permissions(perms))
    });
    let app = if views { app.datagrid_views(MemoryViewStore::default()) } else { app };
    app.feature(
        Feature::new("orders")
            .screen(orders)
            .screen(order_form)
            .screen(designer)
            .screen(work)
            .command(new_order())
            .command(archive())
            .command(hello()),
    )
    .feature(Feature::new("hidden").guard(denied).command(Command::effect(
        "hidden.secret",
        "Secret command",
        Effect::Toast { message: "x".into() },
    )))
    .router()
    .unwrap()
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn send(router: &Router, req: Request<Body>) -> (StatusCode, String) {
    let res = router.clone().oneshot(req).await.unwrap();
    let status = res.status();
    (status, String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap())
}

fn get(path: &str, perms: &str) -> Request<Body> {
    Request::get(path).header("x-perms", perms).body(Body::empty()).unwrap()
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

#[tokio::test]
async fn commands_the_user_may_not_run_never_reach_the_html() {
    let router = app(Counter::default());
    let (status, html) = send(&router, get("/orders", "orders.read")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(r#"data-zeyn-palette-open"#), "the palette button is in the menu");
    assert!(html.contains(r#"/_zeyn/pro.js"#));
    assert!(html.contains(r#"data-zeyn-command="orders.hello""#));
    assert!(!html.contains("orders.new"), "needs orders.write");
    assert!(!html.contains("orders.archive"), "needs orders.admin");
    assert!(!html.contains("Secret command"), "the guard of its feature denies the user");

    let (_, html) = send(&router, get("/orders", "orders.read,orders.write,orders.admin")).await;
    assert!(
        html.contains(
            r#"data-zeyn-command="orders.new" data-zeyn-effect="{&quot;effect&quot;:&quot;open_window&quot;"#
        )
    );
    assert!(html.contains(r#"&quot;screen&quot;:&quot;order_form&quot;"#));
    assert!(
        html.contains(
            r#"aria-keyshortcuts="Control+Alt+N" data-hint="Ctrl+Alt+N" data-icon="plus">New order</button>"#
        )
    );
    assert!(html.contains(r#"data-zeyn-command="orders.archive" data-zeyn-command-run="orders.archive" aria-keyshortcuts="Control+Alt+A""#));
    assert!(!html.contains("Secret command"));
}

#[tokio::test]
async fn action_commands_run_on_the_server_after_csrf_guard_and_permission_checks() {
    let counter = Counter::default();
    let router = app(counter.clone());
    let body = serde_json::json!({ "command": "orders.archive" });
    let no_csrf = Request::post("/_zeyn/command")
        .header("x-perms", "orders.admin")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    assert_eq!(send(&router, no_csrf).await.0, StatusCode::FORBIDDEN);
    assert_eq!(send(&router, post("/_zeyn/command", "orders.read", body.clone())).await.0, StatusCode::FORBIDDEN);
    assert_eq!(counter.0.load(Ordering::SeqCst), 0);
    let (status, json) = send(&router, post("/_zeyn/command", "orders.admin", body)).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json, r#"{"effects":[]}"#);
    assert_eq!(counter.0.load(Ordering::SeqCst), 30, "the input was fixed on the server");
    let unknown =
        send(&router, post("/_zeyn/command", "orders.admin", serde_json::json!({ "command": "orders.nope" }))).await;
    assert_eq!(unknown.0, StatusCode::NOT_FOUND);
    let effect =
        send(&router, post("/_zeyn/command", "orders.admin", serde_json::json!({ "command": "orders.hello" }))).await;
    assert_eq!(effect.0, StatusCode::NOT_FOUND, "effect commands run in the browser");
    let guarded =
        send(&router, post("/_zeyn/command", "orders.admin", serde_json::json!({ "command": "hidden.secret" }))).await;
    assert_eq!(guarded.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn shell_assets_are_served_without_sign_in() {
    let router = app(Counter::default());
    for path in ["/_zeyn/pro.js", "/_zeyn/pro.css"] {
        let (status, body) = send(&router, Request::get(path).body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(!body.is_empty());
    }
}

#[test]
fn shortcuts_are_normalized() {
    assert_eq!(normalize_shortcut("ctrl+shift+p").as_deref(), Some("Control+Shift+P"));
    assert_eq!(normalize_shortcut("Alt+Ctrl+Delete").as_deref(), Some("Control+Alt+Delete"));
    assert_eq!(normalize_shortcut("f12").as_deref(), Some("F12"));
    assert_eq!(normalize_shortcut("Ctrl+F25"), None);
    assert_eq!(normalize_shortcut(""), None);
}

fn problems(feature: Feature) -> Vec<String> {
    feature.check().err().unwrap_or_default()
}

#[test]
fn duplicate_and_unusable_shortcuts_are_reported_by_the_feature_check() {
    let dup = problems(
        Feature::new("f")
            .command(hello().shortcut("Ctrl+Alt+K"))
            .command(Command::effect("f.other", "Other", Effect::CloseAndRefresh).shortcut("control + alt + k")),
    );
    assert_eq!(dup.len(), 1, "{dup:?}");
    assert!(dup[0].contains("commands `orders.hello` and `f.other` both use the shortcut `Control+Alt+K`"), "{dup:?}");
    assert!(dup[0].contains("= help:"));

    let reserved = problems(Feature::new("f").command(hello().shortcut("Alt+Shift+P")));
    assert!(reserved[0].contains("opens the command palette"), "{reserved:?}");
    let single = problems(Feature::new("f").command(hello().shortcut("Shift+K")));
    assert!(single[0].contains("single character key"), "{single:?}");
    let unreadable = problems(Feature::new("f").command(hello().shortcut("Ctrl+Hyper")));
    assert!(unreadable[0].contains("cannot be read"), "{unreadable:?}");
    let ids = problems(Feature::new("f").command(hello()).command(hello()).command(Command::effect(
        "Bad Id",
        "x",
        Effect::CloseAndRefresh,
    )));
    assert!(ids.iter().any(|p| p.contains("command id `orders.hello` is registered twice")), "{ids:?}");
    assert!(ids.iter().any(|p| p.contains("command `Bad Id` has an id that is not usable")), "{ids:?}");
    assert!(problems(Feature::new("f").command(hello().shortcut("Alt+F2")).command(new_order())).is_empty());
}

#[test]
fn shortcuts_shared_across_features_are_reported_by_the_app_check() {
    let app = App::new("Test").feature(Feature::new("a").command(hello().shortcut("Ctrl+Alt+J"))).feature(
        Feature::new("b").command(Command::effect("b.jump", "Jump", Effect::CloseAndRefresh).shortcut("Ctrl+Alt+J")),
    );
    let problems = app.check().unwrap_err();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("both use the shortcut `Control+Alt+J` (in `a` and `b`)"), "{problems:?}");
    assert!(app.router().is_err(), "the app does not start");
}

#[tokio::test]
async fn menubar_follows_the_apg_pattern_and_leaves_out_commands_the_user_may_not_run() {
    let router = app(Counter::default());
    let (status, html) = send(&router, get("/designer", "orders.read")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains(
        r#"<ul class="z-menubar" role="menubar" aria-label="Main" data-zeyn-menubar data-testid="designer.menubar">"#
    ));
    assert!(!html.contains(">File</button>"), "a menu with nothing the user may run is left out");
    assert!(!html.contains(">Admin</button>"));
    assert!(html.contains(
        r#"<button type="button" role="menuitem" class="z-menubar-item" id="designer-menubar-0-0" aria-haspopup="menu" aria-expanded="false" aria-controls="designer-menubar-0-0-menu" tabindex="0">Help</button><ul role="menu" class="z-menu z-menubar-menu" id="designer-menubar-0-0-menu" aria-labelledby="designer-menubar-0-0" hidden>"#
    ));
    assert!(html.contains(r#"role="menuitem" class="z-menu-item" tabindex="-1" data-testid="designer.menubar.orders.hello" data-zeyn-command="orders.hello""#));

    let (_, html) = send(&router, get("/designer", "orders.read,orders.write,orders.admin")).await;
    let file = html.split(">File</button>").nth(1).unwrap().split("</ul>").next().unwrap();
    assert_eq!(
        file.matches(r#"role="separator""#).count(),
        1,
        "leading, doubled and trailing separators are dropped: {file}"
    );
    assert!(file.contains(
        r#"<span class="z-menu-label">New order</span><span class="z-menu-hint" aria-hidden="true">Ctrl+Alt+N</span>"#
    ));
    assert!(file.contains(r#"data-zeyn-command-run="orders.archive""#));
    assert!(html.contains(r#"tabindex="-1">Admin</button>"#), "only the first top item is a tab stop");
}

#[tokio::test]
async fn toolbox_status_bar_and_inspector_sections_render_their_aria() {
    let router = app(Counter::default());
    let (_, html) = send(&router, get("/designer", "orders.read")).await;
    assert!(html.contains(r#"<div class="z-toolbox" role="toolbar" aria-orientation="vertical" aria-label="Tools" data-zeyn-toolbox data-text-selected="{tool} selected""#));
    assert!(html.contains(r#"aria-pressed="false" tabindex="-1" data-tool="select" aria-label="Select""#));
    assert!(html.contains(r#"aria-pressed="true" tabindex="0" data-tool="text" aria-label="Text" title="Text" data-testid="designer.tool.text"><span aria-hidden="true">T</span>"#));
    assert!(html.contains(r#"<section class="z-inspector" aria-labelledby="designer-inspector-1-1-title"><h2 class="z-inspector-title" id="designer-inspector-1-1-title">Properties</h2><form"#));
    assert!(html.contains(r#"<details class="z-form-section" open><summary class="z-form-section-title">General</summary><div class="z-form-section-body" role="group" aria-label="General">"#));
    assert_eq!(html.matches("<details").count(), 2);
    assert_eq!(html.matches("</details>").count(), 2);
    assert!(html.contains(r#"role="status" aria-live="polite" data-zeyn-status-live>Pick a tool</span></div><div class="z-statusbar-end"><span class="z-statusbar-item">100%</span>"#));
    let schema = designer::run().schema(&designer::META).to_string();
    for t in ["\"menubar\"", "\"toolbox\"", "\"property_inspector\"", "\"statusbar\"", "\"section\":\"Size\""] {
        assert!(schema.contains(t), "{t} missing in {schema}");
    }
}

#[tokio::test]
async fn inspector_saves_through_the_form_endpoint_with_validation_and_permission() {
    let counter = Counter::default();
    let router = app(counter.clone());
    let call = |values: serde_json::Value| serde_json::json!({ "screen": "designer", "node": "1.1", "values": values });
    let (status, _) =
        send(&router, post("/_zeyn/form", "orders.read", call(serde_json::json!({ "name": "B", "width": "7" })))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "saving needs orders.write");
    let (status, body) = send(
        &router,
        post("/_zeyn/form", "orders.read,orders.write", call(serde_json::json!({ "name": "", "width": "500" }))),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(body.contains(r#""field":"name""#) && body.contains(r#""field":"width""#), "{body}");
    assert_eq!(counter.0.load(Ordering::SeqCst), 0);
    let (status, body) = send(
        &router,
        post("/_zeyn/form", "orders.read,orders.write", call(serde_json::json!({ "name": "B", "width": "7" }))),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(counter.0.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn dock_renders_areas_tabs_and_dividers_with_node_paths() {
    let router = app(Counter::default());
    let (_, html) = send(&router, get("/work", "orders.read")).await;
    assert!(html.contains(r#"<div class="z-dock" role="group" aria-label="Workspace" data-zeyn-dock="work-dock-0-1" data-testid="work.dock" style="--z-dock-left:30%;--z-dock-right:10%;--z-dock-bottom:60%""#));
    assert!(
        html.contains(r#"<section class="z-dock-area" data-area="right" aria-label="Right" hidden>"#),
        "an area without panels starts hidden"
    );
    assert!(html.contains(r#"role="tab" class="z-dock-tab" id="work-dock-0-1-tab-main" aria-controls="work-dock-0-1-panel-main" aria-selected="true" tabindex="0" draggable="true" data-panel="main""#));
    assert!(html.contains(
        r#"id="work-dock-0-1-tab-notes" aria-controls="work-dock-0-1-panel-notes" aria-selected="false" tabindex="-1""#
    ));
    assert!(html.contains(r#"role="tabpanel" id="work-dock-0-1-panel-notes" aria-labelledby="work-dock-0-1-tab-notes" tabindex="0" data-panel="notes" hidden><div class="z-node" data-zeyn-node="0.1.2">"#));
    assert!(html.contains(r#"class="z-dock-bar" data-edge="left" role="separator" tabindex="0" aria-orientation="vertical" aria-valuenow="30" aria-valuemin="10" aria-valuemax="60" aria-label="Resize the Left area""#));
    assert!(html.contains(r#"data-edge="bottom" role="separator" tabindex="0" aria-orientation="horizontal""#));
    assert!(html.contains(r#"aria-haspopup="menu" aria-expanded="false" aria-label="Panel options (Center)""#));
}

fn dock_call(perms: &str, user: &str, body: serde_json::Value) -> Request<Body> {
    let mut req = post("/_zeyn/dock", perms, body);
    req.headers_mut().insert("x-user", user.parse().unwrap());
    req
}

#[tokio::test]
async fn dock_layout_is_saved_per_user_in_the_view_store_and_cleaned() {
    let router = app(Counter::default());
    let load = serde_json::json!({ "screen": "work", "node": "0.1" });
    let (status, body) = send(&router, dock_call("orders.read", "ann", load.clone())).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, r#"{"layout":null}"#);

    let save = serde_json::json!({ "screen": "work", "node": "0.1", "save": true, "layout": {
        "sizes": { "left": 3, "right": 35, "bottom": 200 },
        "panels": [
            { "id": "log", "area": "right", "open": true, "active": true },
            { "id": "evil<script>", "area": "left", "open": true },
            { "id": "files", "area": "left", "open": false },
            { "id": "log", "area": "left", "open": true }
        ]
    }});
    let (status, _) = send(&router, dock_call("orders.read", "ann", save)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body) = send(&router, dock_call("orders.read", "ann", load.clone())).await;
    let layout: DockLayout =
        serde_json::from_value(serde_json::from_str::<serde_json::Value>(&body).unwrap()["layout"].clone()).unwrap();
    assert_eq!((layout.sizes.left, layout.sizes.right, layout.sizes.bottom), (10, 35, 60));
    let ids: Vec<(&str, DockArea, bool)> = layout.panels.iter().map(|p| (p.id.as_str(), p.area, p.open)).collect();
    assert_eq!(
        ids,
        [("log", DockArea::Right, true), ("files", DockArea::Left, false)],
        "unknown and repeated panels are dropped"
    );

    let (_, other) = send(&router, dock_call("orders.read", "bob", load.clone())).await;
    assert_eq!(other, r#"{"layout":null}"#, "layouts are per user");

    let delete = serde_json::json!({ "screen": "work", "node": "0.1", "save": true, "layout": null });
    assert_eq!(send(&router, dock_call("orders.read", "ann", delete)).await.0, StatusCode::NO_CONTENT);
    assert_eq!(send(&router, dock_call("orders.read", "ann", load)).await.1, r#"{"layout":null}"#);
}

#[tokio::test]
async fn dock_endpoint_checks_csrf_access_node_and_store() {
    let router = app(Counter::default());
    let load = serde_json::json!({ "screen": "work", "node": "0.1" });
    let no_csrf = Request::post("/_zeyn/dock")
        .header("x-perms", "orders.read")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(load.to_string()))
        .unwrap();
    assert_eq!(send(&router, no_csrf).await.0, StatusCode::FORBIDDEN);
    assert_eq!(
        send(&router, dock_call("", "ann", load.clone())).await.0,
        StatusCode::FORBIDDEN,
        "the screen needs orders.read"
    );
    let not_dock = serde_json::json!({ "screen": "work", "node": "0.0" });
    assert_eq!(send(&router, dock_call("orders.read", "ann", not_dock)).await.0, StatusCode::NOT_FOUND);
    let no_store = app_with(Counter::default(), false);
    assert_eq!(
        send(&no_store, dock_call("orders.read", "ann", load)).await.0,
        StatusCode::NOT_FOUND,
        "the browser keeps the layout"
    );
}

#[tokio::test]
async fn commands_and_saved_layouts_count_against_the_write_limit() {
    let router = App::new("Test")
        .i18n(I18n::new("en"))
        .service(Counter::default())
        .write_limit(zeyn::RateLimit::per_user(1, std::time::Duration::from_secs(60)))
        .auth(|h: &HeaderMap| {
            let user = h.get("x-user")?.to_str().ok()?.to_string();
            Some(Identity::user(user).with_permissions(["orders.read", "orders.admin"]))
        })
        .datagrid_views(MemoryViewStore::default())
        .feature(Feature::new("orders").screen(orders).screen(work).command(archive()))
        .router()
        .unwrap();
    let writes = [
        ("/_zeyn/command", serde_json::json!({ "command": "orders.archive" })),
        ("/_zeyn/grid/view", serde_json::json!({ "screen": "orders", "node": 0, "view": null })),
        ("/_zeyn/dock", serde_json::json!({ "screen": "work", "node": "0.1", "save": true, "layout": null })),
        ("/_zeyn/theme", serde_json::json!({ "theme": "dark" })),
    ];
    let as_user = |mut req: Request<Body>, user: &str| {
        req.headers_mut().insert("x-user", user.parse().unwrap());
        req
    };
    for (path, body) in writes {
        let user = path.rsplit('/').next().unwrap();
        send(&router, as_user(post(path, "", body.clone()), user)).await;
        let (status, _) = send(&router, as_user(post(path, "", body), user)).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{path} writes");
    }
    for _ in 0..2 {
        let (status, _) =
            send(&router, as_user(post("/_zeyn/search", "", serde_json::json!({ "query": "x" })), "reader")).await;
        assert_eq!(status, StatusCode::OK, "searching is not a write");
    }
}
