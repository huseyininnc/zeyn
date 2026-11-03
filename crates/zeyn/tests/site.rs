use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;

#[screen(route = "/", public)]
pub fn home() -> Screen {
    Screen::new("Welcome")
}

#[screen(route = "/shop/tea", public)]
pub fn tea() -> Screen {
    Screen::new("Tea")
}

#[screen(route = "/shop/coffee", public)]
pub fn coffee() -> Screen {
    Screen::new("Coffee")
}

#[screen(route = "/staff", permission = "staff.read")]
pub fn staff() -> Screen {
    Screen::new("Staff only")
}

#[screen(route = "/panel", public)]
pub fn panel() -> Screen {
    Screen::new("Panel")
}

fn site() -> Feature {
    Feature::new("site")
        .screen(home)
        .screen(tea)
        .screen(coffee)
        .screen(staff)
        .menu(|m| {
            m.group("Start", Icon::Home)
                .item("Home", home)
                .group("Shop", Icon::Cart)
                .item("Tea", tea)
                .item("Coffee", coffee)
                .item("Staff", staff)
        })
        .shell(
            Shell::site()
                .header(|h| h.sign_in().language().theme())
                .footer(|f| {
                    f.column("Shop", |c| {
                        c.link("Tea", tea)
                            .link("Staff", staff)
                            .url("Source", "https://example.com/src")
                    })
                    .text("Made with care")
                }),
        )
}

fn app() -> App {
    App::new("Corner Shop")
        .login_url("/sign-in")
        .feature(site())
        .feature(
            Feature::new("admin")
                .screen(panel)
                .menu(|m| {
                    m.group("Admin", Icon::Table)
                        .item("Panel", panel)
                }),
        )
}

async fn get(
    router: &axum::Router,
    path: &str,
    partial: bool,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut b = Request::get(path);
    if partial {
        b = b.header("x-zeyn-partial", "1");
    }
    let res = router
        .clone()
        .oneshot(b.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    (
        status,
        headers,
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

#[tokio::test]
async fn site_screens_open_in_the_site_shell_and_panel_screens_keep_the_app_shell() {
    let router = app().router().unwrap();
    let (status, _, page) = get(&router, "/shop/tea", false).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains(r#"<div class="z-app z-site" data-zeyn-shell="site">"#));
    assert!(
        page.contains(r#"href="/_zeyn/site.css?v="#) && page.contains(r#"src="/_zeyn/site.js?v="#)
    );
    assert!(page.contains(r#"<a class="z-site-brand" href="/">Corner Shop</a>"#));
    assert!(
        !page.contains(r#"<nav class="z-nav""#),
        "no side menu in the site shell"
    );
    assert!(
        !page.contains("/panel"),
        "menus of app shell features stay out of the site menu"
    );
    let (_, _, admin) = get(&router, "/panel", false).await;
    assert!(admin.contains(r#"<div class="z-app" data-zeyn-shell="app"><nav class="z-nav""#));
    assert!(
        !admin.contains("site.js"),
        "app shell pages do not load the site shell's assets"
    );
    assert!(admin.contains(r#"href="/panel""#) && !admin.contains(r#"href="/shop/tea""#));
}

#[tokio::test]
async fn the_site_menu_is_a_disclosure_navigation_with_one_link_groups_inline() {
    let (_, _, page) = get(&app().router().unwrap(), "/shop/tea", false).await;
    assert!(
        page.contains(r#"<ul class="z-site-menu"><li><a href="/">Home</a></li>"#),
        "a group of one is a link"
    );
    assert!(page.contains(
        r#"<button type="button" class="z-site-group-button" aria-expanded="false" aria-controls="z-site-group-1" data-current>Shop"#
    ));
    assert!(page.contains(r#"<ul class="z-site-sub" id="z-site-group-1" hidden>"#));
    assert!(page.contains(r#"<a href="/shop/tea" aria-current="page">Tea</a>"#));
    assert!(
        !page.contains(r#"href="/staff""#),
        "screens the visitor may not open are left out"
    );
    assert!(page.contains(r#"aria-controls="z-site-nav" data-zeyn-site-toggle hidden"#));
}

#[tokio::test]
async fn the_header_shows_the_actions_asked_for_and_the_footer_its_columns() {
    let (_, _, page) = get(&app().router().unwrap(), "/", false).await;
    assert!(page.contains(
        r#"<a class="z-button z-site-sign-in" href="/sign-in" data-zeyn-reload>Sign in</a>"#
    ));
    assert!(page.contains(r#"data-zeyn-theme-picker"#));
    assert!(page.contains(r#"<footer class="z-site-footer">"#));
    assert!(page.contains(r#"<h2 id="z-site-footer-0">Shop</h2><ul><li><a href="/shop/tea">Tea</a></li><li><a href="https://example.com/src">Source</a></li></ul>"#));
    assert!(page.contains(r#"<p class="z-site-note">Made with care</p>"#));
}

#[tokio::test]
async fn partial_screens_tell_the_runtime_which_shell_they_belong_to() {
    let router = app().router().unwrap();
    let (_, site, _) = get(&router, "/shop/coffee", true).await;
    assert_eq!(site.get("x-zeyn-shell").unwrap(), "site");
    let (_, other, _) = get(&router, "/panel", true).await;
    assert_eq!(other.get("x-zeyn-shell").unwrap(), "app");
}

#[tokio::test]
async fn the_site_assets_are_served_without_sign_in() {
    let router = app().router().unwrap();
    let (status, headers, js) = get(&router, "/_zeyn/site.js", false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .unwrap(),
        "text/javascript; charset=utf-8"
    );
    assert!(js.contains("aria-expanded"));
    let (status, _, css) = get(&router, "/_zeyn/site.css", false).await;
    assert_eq!(status, StatusCode::OK);
    assert!(css.contains(".z-site-header"));
}

#[test]
fn footer_links_must_point_to_registered_screens_and_safe_addresses() {
    let bad = Feature::new("bad")
        .screen(tea)
        .shell(Shell::site().footer(|f| {
            f.column("Links", |c| {
                c.link("Coffee", coffee)
                    .url("Run", "javascript:alert(1)")
            })
        }));
    let problems = App::new("Bad")
        .feature(bad)
        .check()
        .unwrap_err();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("screen `coffee`, which is not registered")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("`javascript:alert(1)` is not a path")),
        "{problems:?}"
    );
}
