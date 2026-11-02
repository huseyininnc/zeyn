//! Desktop class shell: commands and the palette, shortcut checks, menu bar, toolbox, property
//! inspector, status bar and docking panels.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{I18n, Identity, normalize_shortcut};

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
    Command::open("orders.new", "New order", order_form, OrderParams { id: None }).shortcut("Ctrl+Alt+N").icon(Icon::Plus)
}

fn archive() -> Command {
    Command::action("orders.archive", "Archive old orders", archive_orders, 30).shortcut("Ctrl+Shift+A")
}

fn hello() -> Command {
    Command::effect("orders.hello", "Say hello", Effect::Toast { message: "Hello".into() })
}

fn denied(_: &Ctx) -> GuardResult {
    GuardResult::Deny("No".into())
}

fn app(counter: Counter) -> Router {
    App::new("Test")
        .i18n(I18n::new("en"))
        .service(counter)
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h.get("x-perms")?.to_str().ok()?.split(',').map(String::from).collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(Feature::new("orders").screen(orders).screen(order_form).command(new_order()).command(archive()).command(hello()))
        .feature(
            Feature::new("hidden")
                .guard(denied)
                .command(Command::effect("hidden.secret", "Secret command", Effect::Toast { message: "x".into() })),
        )
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
    assert!(html.contains(r#"data-zeyn-command="orders.new" data-zeyn-effect="{&quot;effect&quot;:&quot;open_window&quot;"#));
    assert!(html.contains(r#"&quot;screen&quot;:&quot;order_form&quot;"#));
    assert!(html.contains(r#"aria-keyshortcuts="Control+Alt+N" data-hint="Ctrl+Alt+N" data-icon="plus">New order</button>"#));
    assert!(html.contains(r#"data-zeyn-command="orders.archive" data-zeyn-command-run="orders.archive" aria-keyshortcuts="Control+Shift+A""#));
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
    let unknown = send(&router, post("/_zeyn/command", "orders.admin", serde_json::json!({ "command": "orders.nope" }))).await;
    assert_eq!(unknown.0, StatusCode::NOT_FOUND);
    let effect = send(&router, post("/_zeyn/command", "orders.admin", serde_json::json!({ "command": "orders.hello" }))).await;
    assert_eq!(effect.0, StatusCode::NOT_FOUND, "effect commands run in the browser");
    let guarded = send(&router, post("/_zeyn/command", "orders.admin", serde_json::json!({ "command": "hidden.secret" }))).await;
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
            .command(hello().shortcut("Ctrl+K"))
            .command(Command::effect("f.other", "Other", Effect::CloseAndRefresh).shortcut("control + k")),
    );
    assert_eq!(dup.len(), 1, "{dup:?}");
    assert!(dup[0].contains("commands `orders.hello` and `f.other` both use the shortcut `Control+K`"), "{dup:?}");
    assert!(dup[0].contains("= help:"));

    let reserved = problems(Feature::new("f").command(hello().shortcut("Ctrl+Shift+P")));
    assert!(reserved[0].contains("opens the command palette"), "{reserved:?}");
    let single = problems(Feature::new("f").command(hello().shortcut("Shift+K")));
    assert!(single[0].contains("single character key"), "{single:?}");
    let unreadable = problems(Feature::new("f").command(hello().shortcut("Ctrl+Hyper")));
    assert!(unreadable[0].contains("cannot be read"), "{unreadable:?}");
    let ids = problems(Feature::new("f").command(hello()).command(hello()).command(Command::effect("Bad Id", "x", Effect::CloseAndRefresh)));
    assert!(ids.iter().any(|p| p.contains("command id `orders.hello` is registered twice")), "{ids:?}");
    assert!(ids.iter().any(|p| p.contains("command `Bad Id` has an id that is not usable")), "{ids:?}");
    assert!(problems(Feature::new("f").command(hello().shortcut("Alt+F2")).command(new_order())).is_empty());
}

#[test]
fn shortcuts_shared_across_features_are_reported_by_the_app_check() {
    let app = App::new("Test")
        .feature(Feature::new("a").command(hello().shortcut("Ctrl+J")))
        .feature(Feature::new("b").command(Command::effect("b.jump", "Jump", Effect::CloseAndRefresh).shortcut("Ctrl+J")));
    let problems = app.check().unwrap_err();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("both use the shortcut `Control+J` (in `a` and `b`)"), "{problems:?}");
    assert!(app.router().is_err(), "the app does not start");
}
