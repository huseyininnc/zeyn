use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Authenticator, MAX_MENU_LABEL, MAX_MENU_NODES, MenuSource};

#[screen(route = "/home", public)]
pub fn home() -> Screen {
    Screen::new("Home")
}

#[screen(route = "/reports", permission = "reports.read")]
pub fn reports() -> Screen {
    Screen::new("Reports")
}

#[params]
pub struct OrderParams {
    pub id: i64,
}

#[screen(route = "/orders/{id}", public)]
pub fn order(p: OrderParams) -> Screen {
    Screen::new(format!("Order {}", p.id))
}

/// Signs in the user named by the `x-user` header with the permissions of the
/// comma separated `x-perms` header.
struct HeaderAuth;

impl Authenticator for HeaderAuth {
    fn authenticate(&self, headers: &HeaderMap) -> Option<Identity> {
        let user = headers.get("x-user")?.to_str().ok()?;
        let mut id = Identity::user(user);
        if let Some(p) = headers
            .get("x-perms")
            .and_then(|v| v.to_str().ok())
        {
            id.permissions = p
                .split(',')
                .map(str::to_string)
                .collect();
        }
        Some(id)
    }
}

fn app(source: impl MenuSource) -> axum::Router {
    App::new("Menu")
        .auth(HeaderAuth)
        .header(|h| h.breadcrumb())
        .feature(
            Feature::new("m")
                .screen(home)
                .screen(reports)
                .screen(order)
                .menu(|m| {
                    m.group("Main", Icon::Home)
                        .item("Home", home)
                }),
        )
        .menu_source(source)
        .router()
        .unwrap()
}

async fn get(router: &axum::Router, path: &str, user: &str, perms: &str) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(
            Request::get(path)
                .header("x-user", user)
                .header("x-perms", perms)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let body = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// The `<nav class="z-nav">` element of a page.
fn side_menu(page: &str) -> &str {
    &page[page
        .find(r#"<nav class="z-nav""#)
        .unwrap()..page.find("</nav>").unwrap()]
}

/// A source that adds a shortcuts group, the way a menu table would.
fn shortcuts() -> Vec<MenuNode> {
    vec![
        MenuNode::group(
            "Shortcuts",
            vec![
                MenuNode::screen("Sales <reports>", "reports"),
                MenuNode::path("Audit", "/audit?from=menu").permission("audit.read"),
                MenuNode::screen("Order 7", "order").params(json!({ "id": 7 })),
            ],
        )
        .icon(Icon::Link),
    ]
}

#[tokio::test]
async fn users_with_different_permissions_see_different_dynamic_menus() {
    let router = app(|_: Ctx| async { Ok(shortcuts()) });
    let (status, page) = get(&router, "/home", "ann", "reports.read,audit.read").await;
    assert_eq!(status, StatusCode::OK);
    let nav = side_menu(&page);
    assert!(nav.contains("<span>Main</span>"), "{nav}");
    assert!(
        nav.contains(r#"<li><a href="/reports">Sales &lt;reports&gt;</a></li>"#),
        "plain labels are escaped: {nav}"
    );
    assert!(nav.contains(r#"<li><a href="/audit?from=menu">Audit</a></li>"#));
    assert!(nav.contains(r#"<li><a href="/orders/7">Order 7</a></li>"#));
    assert!(
        nav.find("Main").unwrap() < nav.find("Shortcuts").unwrap(),
        "static groups come first"
    );

    let (_, page) = get(&router, "/home", "bob", "").await;
    let nav = side_menu(&page);
    assert!(
        !nav.contains("/reports"),
        "a screen the user may not open is left out although the source returned it: {nav}"
    );
    assert!(
        !nav.contains("/audit"),
        "the node's own permission is checked"
    );
    assert!(nav.contains(r#"href="/orders/7""#));
}

#[tokio::test]
async fn a_group_left_without_links_is_not_drawn() {
    let router = app(|_: Ctx| async {
        Ok(vec![MenuNode::group(
            "Admin",
            vec![MenuNode::screen("Reports", "reports")],
        )])
    });
    let (_, page) = get(&router, "/home", "bob", "").await;
    assert!(!side_menu(&page).contains("Admin"));
}

#[tokio::test]
async fn unknown_screens_unsafe_paths_and_incomplete_routes_are_dropped() {
    let router = app(|_: Ctx| async {
        Ok(vec![MenuNode::group(
            "Links",
            vec![
                MenuNode::screen("Ghost", "no_such_screen"),
                MenuNode::path("Script", "javascript:alert(1)"),
                MenuNode::path("Other host", "//evil.com/x"),
                MenuNode::path("Absolute", "https://x.example/"),
                MenuNode::path("Backslash", "/\\evil.com"),
                MenuNode::path("Control", "/a\nb"),
                MenuNode::screen("Order", "order"),
                MenuNode::path("Fine", "/fine"),
            ],
        )])
    });
    let (_, page) = get(&router, "/home", "ann", "").await;
    let nav = side_menu(&page);
    for gone in [
        "Ghost",
        "Script",
        "evil.com",
        "x.example",
        "Backslash",
        "Control",
        "/orders",
    ] {
        assert!(!nav.contains(gone), "{gone} must be dropped: {nav}");
    }
    assert!(nav.contains(r#"<li><a href="/fine">Fine</a></li>"#));
}

#[tokio::test]
async fn depth_node_count_and_label_length_are_capped() {
    let router = app(|_: Ctx| async {
        let mut deep = MenuNode::group("L4", vec![MenuNode::path("Too deep", "/deep")]);
        for level in ["L3", "L2", "L1"] {
            deep = MenuNode::group(level, vec![MenuNode::path(level, "/ok"), deep]);
        }
        let many: Vec<MenuNode> = (0..MAX_MENU_NODES + 50)
            .map(|i| MenuNode::path(format!("n{i}x"), "/n"))
            .collect();
        Ok(vec![
            MenuNode::group("Top", vec![deep]),
            MenuNode::group(
                "Long",
                vec![MenuNode::path("y".repeat(MAX_MENU_LABEL + 20), "/long")],
            ),
            MenuNode::group("Many", many),
            MenuNode::path("Top level link", "/top"),
        ])
    });
    let (_, page) = get(&router, "/home", "ann", "").await;
    let nav = side_menu(&page);
    assert!(nav.contains("<span>L3</span>"));
    assert!(
        !nav.contains("L4") && !nav.contains("Too deep"),
        "groups deeper than MENU_DEPTH are dropped: {nav}"
    );
    assert!(nav.contains(&format!(">{}</a>", "y".repeat(MAX_MENU_LABEL))));
    assert!(!nav.contains(&"y".repeat(MAX_MENU_LABEL + 1)));
    assert!(nav.contains(">n0x<"));
    assert!(
        !nav.contains(&format!(">n{}x<", MAX_MENU_NODES)),
        "nodes past MAX_MENU_NODES are dropped"
    );
    assert!(
        !nav.contains("Top level link"),
        "top level nodes must be groups"
    );
}

#[tokio::test]
async fn answers_are_cached_per_permission_set_until_invalidated() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let router = app(move |ctx: Ctx| {
        counted.fetch_add(1, Ordering::SeqCst);
        let menus = ctx.service::<Menus>().unwrap();
        async move {
            if ctx
                .identity()
                .permissions
                .contains("menu.reset")
            {
                menus.invalidate();
            }
            Ok(shortcuts())
        }
    });
    get(&router, "/home", "ann", "reports.read").await;
    get(&router, "/home", "carl", "reports.read").await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "users with the same permissions share one answer"
    );
    get(&router, "/home", "bob", "").await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    get(&router, "/home", "dora", "menu.reset").await;
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    get(&router, "/home", "ann", "reports.read").await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        4,
        "invalidate makes the next page ask the source again"
    );
}

#[tokio::test]
async fn a_failing_or_panicking_source_leaves_the_static_menu() {
    let router = app(|_: Ctx| async { Err(Error::Internal("menu table is gone".into())) });
    let (status, page) = get(&router, "/home", "ann", "").await;
    assert_eq!(status, StatusCode::OK);
    assert!(side_menu(&page).contains("<span>Main</span>"));
    assert!(!page.contains("menu table is gone"));

    let router = app(|_: Ctx| async { panic!("boom") });
    let (status, page) = get(&router, "/home", "ann", "").await;
    assert_eq!(status, StatusCode::OK);
    assert!(side_menu(&page).contains("<span>Main</span>"));
}

#[tokio::test]
async fn the_breadcrumb_follows_a_dynamic_link() {
    let router = app(|_: Ctx| async {
        Ok(vec![MenuNode::group(
            "Shortcuts",
            vec![MenuNode::group(
                "Finance",
                vec![MenuNode::screen("Sales", "reports")],
            )],
        )])
    });
    let (_, page) = get(&router, "/reports", "ann", "reports.read").await;
    assert!(
        page.contains(r#"<ol><li>Shortcuts</li><li>Finance</li><li><span aria-current="page">Sales</span></li></ol>"#),
        "{page}"
    );
    assert!(
        side_menu(&page).contains(r#"<li><a href="/reports" aria-current="page">Sales</a></li>"#)
    );
}
