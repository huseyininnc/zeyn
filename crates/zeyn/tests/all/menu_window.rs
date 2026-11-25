//! Menu items that open a screen as a window (`Menu::window`): their kind in the menu and the app
//! description, the check of their screen, and the side menu button that runs `open_window`
//! for users who may open the screen.

use axum::body::Body;
use axum::http::{HeaderMap, Request};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Authenticator, Identity, MenuTarget};

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AboutParams {
    pub tab: String,
}

impl zeyn::Params for AboutParams {}

#[screen(public)]
pub fn about(p: AboutParams) -> Screen {
    Screen::window(format!("About {}", p.tab))
}

#[screen(permission = "logs.read")]
pub fn log_viewer() -> Screen {
    Screen::window("Logs")
}

#[screen(route = "/home", public)]
pub fn home() -> Screen {
    Screen::new("Home")
}

/// A feature whose `Help` group holds a page item, a window item with params and, in a nested
/// group, a window item that needs a permission.
fn feature() -> Feature {
    Feature::new("help")
        .screen(home)
        .screen(about)
        .screen(log_viewer)
        .menu(|m| {
            m.group("Help", Icon::Info)
                .open()
                .item("Home", home)
                .window(
                    "About",
                    about,
                    AboutParams {
                        tab: "<team>".into(),
                    },
                )
                .icon(Icon::Info)
                .badge("New")
                .sub("Tools", |m| m.window("Logs", log_viewer, ()))
        })
}

#[test]
fn window_items_have_the_window_kind_and_their_params() {
    let f = feature();
    let items = &f.menu_groups()[0].items;
    assert_eq!(items[0].target, MenuTarget::Page);
    assert!(!items[0].is_window());
    assert!(items[1].is_window());
    assert_eq!(
        items[1].target,
        MenuTarget::Window {
            params: serde_json::json!({ "tab": "<team>" })
        }
    );
    let json = serde_json::to_value(&items[1]).unwrap();
    assert_eq!(json["kind"], "window");
    assert_eq!(json["screen"], "about");
    assert_eq!(json["params"]["tab"], "<team>");
    assert!(items[1].icon.is_some() && items[1].badge.is_some());
    assert_eq!(serde_json::to_value(&items[0]).unwrap()["kind"], "page");
    let nested = &f.menu_groups()[0].groups[0].items[0];
    assert!(nested.is_window());
    assert_eq!(nested.screen, "log_viewer");
}

#[test]
fn find_returns_page_items_only() {
    let f = feature();
    assert!(
        f.menu_groups()[0]
            .find("home")
            .is_some()
    );
    assert!(
        f.menu_groups()[0]
            .find("about")
            .is_none()
    );
    assert_eq!(f.menu_groups()[0].all_items().len(), 3);
}

#[test]
fn window_items_pass_the_check_without_a_route() {
    assert!(feature().check().is_ok());
    assert!(
        App::new("T")
            .feature(feature())
            .check()
            .is_ok()
    );
}

#[test]
fn a_window_item_may_open_a_screen_of_another_feature() {
    let app = App::new("T")
        .feature(Feature::new("a").screen(about))
        .feature(Feature::new("b").menu(|m| {
            m.group("Help", Icon::Info)
                .window("About", about, AboutParams::default())
        }));
    assert!(app.check().is_ok(), "{:?}", app.check());
}

#[test]
fn a_window_item_whose_screen_no_feature_registers_is_a_problem() {
    let problems = App::new("T")
        .feature(Feature::new("a").menu(|m| {
            m.group("Help", Icon::Info)
                .sub("Deep", |m| m.window("Logs", log_viewer, ()))
        }))
        .check()
        .unwrap_err();
    let p = problems
        .iter()
        .find(|p| p.contains("Z0418"))
        .expect("Z0418");
    assert!(p.contains("menu item `Logs` opens screen `log_viewer` as a window"));
}

#[test]
fn describe_lists_the_kind_and_params_of_menu_items() {
    let d = App::new("T")
        .feature(feature())
        .describe();
    let items = &d["features"][0]["menu"][0]["items"];
    assert_eq!(items[0]["kind"], "page");
    assert_eq!(items[1]["kind"], "window");
    assert_eq!(items[1]["params"]["tab"], "<team>");
    assert_eq!(
        d["features"][0]["menu"][0]["groups"][0]["items"][0]["kind"],
        "window"
    );
}

/// Signs in the user named by the `x-user` header with the permissions of the comma separated
/// `x-perms` header.
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

/// Lets only `admin` through, for the guard of the `tools` feature.
fn admins_only(ctx: &Ctx) -> GuardResult {
    if ctx.identity().user.as_deref() == Some("admin") {
        GuardResult::Allow
    } else {
        GuardResult::Deny("admins only".into())
    }
}

#[screen(public)]
pub fn console() -> Screen {
    Screen::window("Console")
}

/// The help feature, and a `tools` feature behind a guard whose `console` screen the help menu
/// opens as a window.
fn router() -> axum::Router {
    App::new("Help")
        .auth(HeaderAuth)
        .header(|h| h.breadcrumb())
        .feature(feature().menu(|m| {
            m.group("Help", Icon::Info)
                .open()
                .item("Home", home)
                .window(
                    "About",
                    about,
                    AboutParams {
                        tab: "<team>".into(),
                    },
                )
                .icon(Icon::Info)
                .badge("New")
                .sub("Tools", |m| {
                    m.window("Logs", log_viewer, ())
                        .window("Console", console, ())
                })
        }))
        .feature(
            Feature::new("tools")
                .guard(admins_only)
                .screen(console),
        )
        .router()
        .unwrap()
}

async fn page(path: &str, user: &str, perms: &str) -> String {
    let res = router()
        .oneshot(
            Request::get(path)
                .header("x-user", user)
                .header("x-perms", perms)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    String::from_utf8(body.to_vec()).unwrap()
}

/// The `<nav class="z-nav">` element of a page.
fn side_menu(page: &str) -> &str {
    &page[page
        .find(r#"<nav class="z-nav""#)
        .unwrap()..page.find("</nav>").unwrap()]
}

#[tokio::test]
async fn window_items_render_as_buttons_that_run_open_window_without_an_address() {
    let html = page("/home", "ann", "").await;
    let nav = side_menu(&html);
    assert!(
        nav.contains(r#"<li><button type="button" class="z-nav-window" aria-haspopup="dialog" data-zeyn-menu-window data-zeyn-effect="{&quot;effect&quot;:&quot;open_window&quot;,&quot;params&quot;:{&quot;tab&quot;:&quot;&lt;team&gt;&quot;},&quot;screen&quot;:&quot;about&quot;}"><span class="z-icon z-icon-info" aria-hidden="true"></span><span>About</span> <span class="z-nav-badge">New</span></button></li>"#),
        "{nav}"
    );
    assert!(!nav.contains("<team>"), "params are escaped: {nav}");
    assert_eq!(
        nav.matches("href=").count(),
        1,
        "only the page item has an address: {nav}"
    );
    assert!(
        nav.contains(r#"<a href="/home" aria-current="page">"#),
        "{nav}"
    );
    assert!(
        html.contains(r#"<li><span aria-current="page">Home</span></li>"#),
        "the breadcrumb still names the page item: {html}"
    );
}

#[tokio::test]
async fn window_items_show_only_to_users_who_may_open_their_screen() {
    let nav_of = |html: &str| side_menu(html).to_string();
    let plain = nav_of(&page("/home", "ann", "").await);
    assert!(
        !plain.contains(">Logs<"),
        "the screen's permission hides it: {plain}"
    );
    assert!(
        !plain.contains(">Console<"),
        "the screen's feature guard hides it: {plain}"
    );
    assert!(
        !plain.contains(">Tools<"),
        "a group left without items is left out: {plain}"
    );
    let reader = nav_of(&page("/home", "ann", "logs.read").await);
    assert!(
        reader.contains(r#"<li class="z-nav-group z-nav-sub"><h3><button type="button" class="z-nav-heading" aria-expanded="false" aria-controls="z-nav-g0-0"><span>Tools</span><span class="z-nav-caret" aria-hidden="true"></span></button></h3><ul id="z-nav-g0-0"><li><button type="button" class="z-nav-window" aria-haspopup="dialog" data-zeyn-menu-window data-zeyn-effect="{&quot;effect&quot;:&quot;open_window&quot;,&quot;params&quot;:null,&quot;screen&quot;:&quot;log_viewer&quot;}">Logs</button></li></ul></li>"#),
        "a nested window item never marks its group as the current branch: {reader}"
    );
    assert!(!reader.contains(">Console<"), "{reader}");
    let admin = nav_of(&page("/home", "admin", "logs.read").await);
    assert!(admin.contains(">Console</button>"), "{admin}");
}
