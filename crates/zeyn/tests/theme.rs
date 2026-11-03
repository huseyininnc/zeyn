//! Themes: the standard picker, the `zeyn_theme` cookie rendered on `<html>`, app themes served
//! as CSS, the saved choice in the view store, locking and the startup checks.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Identity, MemoryViewStore};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[screen(route = "/home", public)]
pub fn home() -> Screen {
    Screen::new("Home")
}

fn ocean() -> Theme {
    Theme::named("ocean", [("--z-brand", "#0b5394"), ("--z-bg", "#eef4f8")]).label("Ocean")
}

fn build(app: App) -> Router {
    app.feature(Feature::new("h").screen(home)).router().unwrap()
}

async fn get(router: &Router, path: &str, cookie: &str) -> (StatusCode, HeaderMap, String) {
    let req = Request::get(path).header(header::COOKIE, cookie).header("x-user", "ada").body(Body::empty()).unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    (status, headers, String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap())
}

async fn choose(router: &Router, theme: &str) -> (StatusCode, HeaderMap) {
    let req = Request::post("/_zeyn/theme")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .header("x-user", "ada")
        .body(Body::from(serde_json::json!({ "theme": theme }).to_string()))
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    (res.status(), res.headers().clone())
}

fn html_tag(page: &str) -> &str {
    let start = page.find("<html").unwrap();
    &page[start..start + page[start..].find('>').unwrap() + 1]
}

#[tokio::test]
async fn every_app_gets_a_picker_with_system_light_and_dark_and_follows_the_device_by_default() {
    let r = build(App::new("T"));
    let (_, _, page) = get(&r, "/home", "").await;
    assert_eq!(html_tag(&page), r#"<html lang="en">"#);
    assert!(page.contains(r#"<meta name="color-scheme" content="light dark">"#));
    assert!(page.contains(r#"<span id="z-theme-label">Theme</span><div class="z-select" data-text-none=""><select id="z-theme" data-zeyn-theme-picker>"#));
    assert!(page.contains(r#"<option value="system" data-scheme="" selected>System</option>"#));
    assert!(page.contains(r#"<option value="light" data-scheme="light">Light</option>"#));
    assert!(page.contains(r#"<option value="dark" data-scheme="dark">Dark</option>"#));
    assert!(!page.contains("/_zeyn/theme.css"), "no stylesheet request when no theme changes a token");
}

#[tokio::test]
async fn the_cookie_is_rendered_on_html_so_the_first_paint_is_right() {
    let r = build(App::new("T"));
    let (_, _, page) = get(&r, "/home", "zeyn_theme=dark").await;
    assert_eq!(html_tag(&page), r#"<html lang="en" data-theme="dark">"#);
    assert!(page.contains(r#"<meta name="color-scheme" content="dark">"#));
    assert!(page.contains(r#"<option value="dark" data-scheme="dark" selected>Dark</option>"#));
    assert!(!page.contains("<script>"), "no inline script");

    let (_, _, odd) = get(&r, "/home", r#"zeyn_theme="><script>"#).await;
    assert_eq!(html_tag(&odd), r#"<html lang="en">"#);
}

#[tokio::test]
async fn app_themes_are_served_as_one_stylesheet_and_picked_by_name() {
    let r = build(App::new("T").theme(Theme::new().radius(6)).themes([
        Theme::light(),
        Theme::dark().brand("#2a6f97"),
        ocean(),
    ]));
    let (_, h, css) = get(&r, "/_zeyn/theme.css", "").await;
    assert_eq!(h.get(header::CONTENT_TYPE).unwrap(), "text/css; charset=utf-8");
    assert!(css.starts_with(":root{--z-radius:6px;}\n"));
    assert!(css.contains("@media (prefers-color-scheme: dark){:root:not([data-theme]){--z-brand:#2a6f97;}}"));
    assert!(css.contains(r#":root[data-theme="dark"]:not([data-zeyn-theme]){--z-brand:#2a6f97;}"#));
    assert!(css.contains(r#":root[data-zeyn-theme="ocean"]{--z-brand:#0b5394;--z-bg:#eef4f8;}"#));

    let (_, _, page) = get(&r, "/home", "zeyn_theme=ocean").await;
    assert_eq!(html_tag(&page), r#"<html lang="en" data-theme="light" data-zeyn-theme="ocean">"#);
    assert!(page.contains(r#"<option value="ocean" data-scheme="light" data-custom selected>Ocean</option>"#));
    let link = page.find(r#"<link rel="stylesheet" href="/_zeyn/theme.css?v="#).unwrap();
    assert!(link > page.find("/_zeyn/components.css").unwrap(), "theme tokens load after every kit stylesheet");
}

#[tokio::test]
async fn choosing_a_theme_sets_the_cookie_and_rejects_unknown_themes_and_missing_csrf() {
    let r = build(App::new("T").themes([Theme::light(), Theme::dark(), ocean()]));
    let (status, h) = choose(&r, "ocean").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(h.get(header::SET_COOKIE).unwrap(), "zeyn_theme=ocean; Path=/; Max-Age=31536000; SameSite=Lax");
    assert_eq!(choose(&r, "system").await.0, StatusCode::NO_CONTENT);
    assert_eq!(choose(&r, "neon").await.0, StatusCode::NOT_FOUND);

    let req = Request::post("/_zeyn/theme")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"theme":"dark"}"#))
        .unwrap();
    assert_eq!(r.clone().oneshot(req).await.unwrap().status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_signed_in_users_choice_is_saved_in_the_view_store_and_follows_them_to_a_new_device() {
    let r = build(
        App::new("T")
            .auth(|h: &HeaderMap| Some(Identity::user(h.get("x-user")?.to_str().ok()?)))
            .datagrid_views(MemoryViewStore::default()),
    );
    assert_eq!(choose(&r, "dark").await.0, StatusCode::NO_CONTENT);
    let (_, h, page) = get(&r, "/home", "").await;
    assert_eq!(html_tag(&page), r#"<html lang="en" data-theme="dark">"#);
    let cookies: Vec<_> = h.get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap().to_string()).collect();
    assert!(cookies.iter().any(|c| c.starts_with("zeyn_theme=dark;")));
    let (_, _, light) = get(&r, "/home", "zeyn_theme=light").await;
    assert_eq!(html_tag(&light), r#"<html lang="en" data-theme="light">"#, "the cookie of this device wins");
}

#[tokio::test]
async fn a_hidden_picker_or_a_single_theme_locks_the_app() {
    let hidden = build(App::new("T").theme_picker(false).default_theme("dark"));
    let (_, _, page) = get(&hidden, "/home", "zeyn_theme=light").await;
    assert_eq!(html_tag(&page), r#"<html lang="en" data-theme="dark">"#);
    assert!(!page.contains("data-zeyn-theme-picker"));
    assert_eq!(choose(&hidden, "light").await.0, StatusCode::NOT_FOUND);

    let corporate = build(App::new("T").themes([ocean()]));
    let (_, _, page) = get(&corporate, "/home", "zeyn_theme=dark").await;
    assert_eq!(html_tag(&page), r#"<html lang="en" data-theme="light" data-zeyn-theme="ocean">"#);
    assert!(!page.contains("data-zeyn-theme-picker"));

    let dark = build(App::new("T").theme(Theme::dark()));
    let (_, _, page) = get(&dark, "/home", "zeyn_theme=light").await;
    assert_eq!(html_tag(&page), r#"<html lang="en" data-theme="dark">"#);
    assert!(!page.contains("data-zeyn-theme-picker"));
}

#[test]
fn the_built_in_themes_meet_wcag_aa_contrast() {
    assert_eq!(App::new("T").themes([Theme::light(), Theme::dark()]).check(), Ok(()));
}

#[test]
fn bad_tokens_names_defaults_and_low_contrast_are_reported_by_check() {
    let problems = App::new("T")
        .themes([
            Theme::light(),
            Theme::named("Bad Name", [("--z-brand", "#123456")]),
            Theme::named(
                "evil",
                [("--z-bg", "red;}body{display:none"), ("--z-bg", "url(//x)"), ("--z-made-up", "#fff")],
            ),
            Theme::named("faint", [("--z-text", "#9aa0a6")]),
            Theme::named("night", [("--z-surface", "#000000")]).dark_base(),
        ])
        .default_theme("neon")
        .check()
        .unwrap_err();
    let all = problems.join("\n");
    assert!(all.contains("theme name `Bad Name` is not allowed"), "{all}");
    assert!(all.contains("theme `evil`: the value of `--z-bg` is not allowed"), "{all}");
    assert!(all.contains("theme `evil`: `--z-made-up` is not a theme token"), "{all}");
    assert!(all.contains("theme `faint`: `--z-text` on `--z-surface` has contrast 2.6"), "{all}");
    assert!(all.contains("WCAG 2.2 AA needs 4.5:1"), "{all}");
    assert!(!all.contains("theme `night`"), "dark text tokens on a black surface still pass: {all}");
    assert!(all.contains("default theme `neon` is not one of the offered themes"), "{all}");
}
