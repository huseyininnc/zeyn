use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;

#[screen(route = "/one", public)]
pub fn one() -> Screen {
    Screen::new("First <screen>")
}

#[screen(route = "/two", permission = "two.read")]
pub fn two() -> Screen {
    Screen::new("Second")
}

fn app(theme: Option<Theme>) -> axum::Router {
    let mut app = App::new("Nav").feature(Feature::new("n").screen(one).screen(two));
    if let Some(t) = theme {
        app = app.theme(t);
    }
    app.router().unwrap()
}

async fn get(router: &axum::Router, path: &str, partial: bool) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut b = Request::get(path);
    if partial {
        b = b.header("x-zeyn-partial", "1");
    }
    let res = router.clone().oneshot(b.body(Body::empty()).unwrap()).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    (status, headers, String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap())
}

#[tokio::test]
async fn full_pages_set_the_color_scheme_before_css_loads_and_include_navigation() {
    let (_, _, body) = get(&app(None), "/one", false).await;
    assert!(body.contains(r#"<meta name="color-scheme" content="light dark">"#));
    assert!(body.contains(r#"<script type="module" src="/_zeyn/nav.js?v="#));
    let (_, _, dark) = get(&app(Some(Theme::dark())), "/one", false).await;
    assert!(dark.contains(r#"<meta name="color-scheme" content="dark">"#));
}

#[tokio::test]
async fn partial_requests_return_only_the_screen_and_its_title() {
    let (status, headers, body) = get(&app(None), "/one", true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("x-zeyn-partial").unwrap(), "1");
    assert_eq!(headers.get("x-zeyn-title").unwrap(), "First%20%3Cscreen%3E%20%C2%B7%20Nav");
    assert_eq!(headers.get(header::CACHE_CONTROL).unwrap(), "no-store");
    assert!(body.starts_with(r#"<section class="z-screen""#));
    assert!(body.contains("First &lt;screen&gt;"));
    assert!(!body.contains("<html"));
}

#[tokio::test]
async fn partial_requests_that_fail_get_the_error_screen_alone_with_its_status() {
    let (status, headers, body) = get(&app(None), "/two", true).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(headers.get("x-zeyn-partial").unwrap(), "1");
    assert!(body.starts_with(r#"<section class="z-screen z-error""#), "{body}");
    assert!(!body.contains("<html"));
}

#[screen(public)]
pub fn missing() -> Screen {
    Screen::new("Lost <here>").body(Card::new().title("Try the menu"))
}

#[screen(public)]
pub fn broken() -> Screen {
    Screen::new("Something broke")
}

#[screen(route = "/boom", public)]
pub fn boom() -> Result<Screen> {
    Err(Error::Internal("database password=hunter2 refused".into()))
}

#[screen(route = "/panic", public)]
pub fn panics() -> Screen {
    panic!("secret panic detail");
}

#[screen(public)]
pub fn broken_error_screen() -> Screen {
    panic!("the error screen itself fails");
}

fn error_app() -> axum::Router {
    App::new("Nav")
        .not_found(missing)
        .server_error(broken)
        .feature(Feature::new("n").screen(one).screen(boom).screen(panics).screen(order))
        .router()
        .unwrap()
}

#[tokio::test]
async fn unknown_paths_and_missing_records_get_the_apps_404_screen_in_the_shell() {
    let router = error_app();
    for path in ["/nowhere", "/orders/404", "/orders/abc"] {
        let (status, _, body) = get(&router, path, false).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(body.contains("<html") && body.contains(r#"<nav class="z-nav""#), "{path} is inside the shell");
        assert!(body.contains("Lost &lt;here&gt;") && body.contains("Try the menu"), "{path}: {body}");
        assert!(body.contains("<title>Lost &lt;here&gt; · Nav</title>"));
    }
    let (status, headers, body) = get(&router, "/nowhere", true).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(headers.get("x-zeyn-partial").unwrap(), "1");
    assert_eq!(headers.get("x-zeyn-title").unwrap(), "Lost%20%3Chere%3E%20%C2%B7%20Nav");
    assert!(body.starts_with(r#"<section class="z-screen" data-zeyn-screen="missing""#), "{body}");
}

#[tokio::test]
async fn errors_and_panics_get_a_500_without_details() {
    let router = error_app();
    for (path, partial) in [("/boom", false), ("/boom", true), ("/panic", false), ("/panic", true)] {
        let (status, _, body) = get(&router, path, partial).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{path}");
        assert!(body.contains("Something broke"), "{path}: {body}");
        assert!(!body.contains("hunter2") && !body.contains("secret panic") && !body.contains("database"), "{path}");
        assert_eq!(body.contains("<html"), !partial, "{path}");
    }
    let (status, _, body) = get(&router, "/_zeyn/screen/panics", false).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!body.contains("secret panic"));
    assert!(body.contains(r#""error":"internal""#), "endpoints answer JSON: {body}");
}

#[tokio::test]
async fn without_error_screens_or_when_they_fail_the_built_in_message_is_shown() {
    let plain = App::new("Nav").feature(Feature::new("n").screen(boom)).router().unwrap();
    let (status, _, body) = get(&plain, "/nowhere", false).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains(r#"<section class="z-screen z-error""#));
    let failing =
        App::new("Nav").server_error(broken_error_screen).feature(Feature::new("n").screen(boom)).router().unwrap();
    let (status, _, body) = get(&failing, "/boom", false).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body.contains(r#"<section class="z-screen z-error""#) && !body.contains("hunter2"));
}

#[tokio::test]
async fn the_navigation_script_is_served() {
    let (status, headers, body) = get(&app(None), "/_zeyn/nav.js", false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "text/javascript; charset=utf-8");
    assert!(body.contains("x-zeyn-partial"));
}

#[params]
pub struct OrderParams {
    pub id: u64,
    #[serde(default)]
    pub tab: Option<String>,
}

#[screen(route = "/orders/{id}", public)]
pub fn order(p: OrderParams) -> Result<Screen> {
    if p.id == 404 {
        return Err(Error::NotFound);
    }
    let tabs = Tabs::new("Parts")
        .tab("Head", Card::new().title("Head"))
        .tab("Lines", Card::new().title(format!("Lines of {}", p.id)));
    Ok(Screen::new(format!("Order {} {}", p.id, p.tab.unwrap_or_default())).body(Stack::new().add(tabs)).body(
        Breadcrumb::new()
            .link("Cups", catalog, CatalogParams { slug: Slug::from_title("Çay Bardağı"), n: 2 })
            .current("Order"),
    ))
}

#[params]
pub struct CatalogParams {
    pub slug: Slug,
    pub n: u32,
}

#[screen(route = "/catalog/{slug}/{n}", public)]
pub fn catalog(p: CatalogParams) -> Screen {
    if p.slug.as_str() == "old-name" {
        return Screen::moved_to(catalog, CatalogParams { slug: Slug::from_title("New Name"), n: p.n });
    }
    Screen::new(format!("Catalog {} page {}", p.slug, p.n))
}

#[params]
pub struct SecretParams {
    pub id: i64,
}

#[screen(route = "/secret/{id}", permission = "secret.read")]
pub fn secret(p: SecretParams) -> Screen {
    Screen::new(format!("Secret {}", p.id))
}

fn path_app() -> axum::Router {
    App::new("Nav")
        .login_url("/login")
        .feature(Feature::new("n").screen(one).screen(order).screen(catalog).screen(secret))
        .router()
        .unwrap()
}

#[tokio::test]
async fn path_segments_bind_to_params_fields_and_the_json_query_adds_the_rest() {
    let router = path_app();
    let (status, headers, body) = get(&router, "/orders/42", true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(headers.get("x-zeyn-partial").unwrap(), "1");
    assert!(body.contains("Order 42"));
    assert!(body.contains(r#"data-zeyn-params="{&quot;id&quot;:42}""#), "{body}");

    let (status, _, body) = get(&router, "/orders/42?params=%7B%22tab%22%3A%22lines%22%7D", false).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Order 42 lines"));
    assert!(body.contains("<html"), "a full load gets the shell");

    let (status, _, body) = get(&router, "/catalog/cay-bardagi/2", true).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Catalog cay-bardagi page 2"));
}

#[tokio::test]
async fn path_values_that_do_not_parse_or_records_that_are_missing_answer_404() {
    let router = path_app();
    for path in [
        "/orders/abc",
        "/orders/007",
        "/orders/+7",
        "/orders/-1",
        "/orders/404",
        "/catalog/Cay/2",
        "/catalog/cay-/2",
        "/catalog/cay/x",
    ] {
        let (status, _, _) = get(&router, path, false).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn a_screen_can_answer_a_permanent_redirect_to_its_new_address() {
    let router = path_app();
    let (status, headers, _) = get(&router, "/catalog/old-name/3", true).await;
    assert_eq!(status, StatusCode::MOVED_PERMANENTLY);
    assert_eq!(headers.get(header::LOCATION).unwrap(), "/catalog/new-name/3");
}

#[tokio::test]
async fn framework_links_use_the_path_form() {
    let router = path_app();
    let (status, headers, _) =
        get(&router, "/_zeyn/route/order?params=%7B%22id%22%3A42%2C%22tab%22%3A%22a.b%22%7D", false).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers.get(header::LOCATION).unwrap(), "/orders/42?params=%7B%22tab%22%3A%22a%2Eb%22%7D");
    let (status, _, _) = get(&router, "/_zeyn/route/order?params=%7B%22tab%22%3A1%7D", false).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "params without the segment field cannot fill the route");
    let (_, headers, _) = get(&router, "/_zeyn/route/one", false).await;
    assert_eq!(headers.get(header::LOCATION).unwrap(), "/one");

    let (_, _, body) = get(&router, "/orders/42", true).await;
    assert!(body.contains(r#"<a href="/catalog/cay-bardagi/2" data-zeyn-base-href>Cups</a>"#), "{body}");
}

#[tokio::test]
async fn lazy_parts_and_fragments_of_a_path_screen_get_its_params() {
    let router = path_app();
    let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let body = serde_json::json!({ "screen": "order", "params": { "id": 42 }, "node": "0.0.1" });
    let req = Request::post("/_zeyn/node")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={token}"))
        .header("x-zeyn-csrf", token)
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(html.contains("Lines of 42"), "{html}");

    let (status, _, html) = get(&router, "/_zeyn/screen/order?params=%7B%22id%22%3A7%7D", false).await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Order 7"));
}

#[tokio::test]
async fn path_screens_check_access_and_return_to_keeps_the_path() {
    let router = path_app();
    let (status, headers, _) = get(&router, "/secret/5?params=%7B%7D", false).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers.get(header::LOCATION).unwrap(), "/login?return_to=%2Fsecret%2F5%3Fparams%3D%257B%257D");
    let (status, _, _) = get(&router, "/_zeyn/screen/secret?params=%7B%22id%22%3A5%7D", false).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[params]
pub struct CodeParams {
    pub code: String,
}

#[screen(route = "/orders/{code}", public)]
pub fn order_by_code(p: CodeParams) -> Screen {
    Screen::new(p.code)
}

#[test]
fn routes_that_differ_only_in_segment_names_and_home_screens_with_segments_are_problems() {
    let problems =
        App::new("Nav").home(order).feature(Feature::new("n").screen(order).screen(order_by_code)).check().unwrap_err();
    assert!(
        problems.iter().any(|p| p.contains("route `/orders/{code}` is used by screens `order` and `order_by_code`")),
        "{problems:?}"
    );
    assert!(problems.iter().any(|p| p.contains("home screen `order` has path segments")), "{problems:?}");
}

#[test]
fn slugs_map_turkish_letters_fold_accents_and_drop_the_rest() {
    assert_eq!(Slug::from_title("Çay Bardağı").as_str(), "cay-bardagi");
    assert_eq!(Slug::from_title("İSTANBUL ışık Şeker Ğ Ö Ü").as_str(), "istanbul-isik-seker-g-o-u");
    assert_eq!(Slug::from_title("Ünlü -- Çiçek!!").as_str(), "unlu-cicek");
    assert_eq!(Slug::from_title("Straße Æon Œuvre Łódź").as_str(), "strasse-aeon-oeuvre-lodz");
    assert_eq!(Slug::from_title("日本 tea ☕ time").as_str(), "tea-time");
    assert_eq!(Slug::from_title("  ").as_str(), "");
    let long = Slug::from_title(&"word ".repeat(40));
    assert!(long.as_str().len() <= Slug::MAX_LEN && !long.as_str().ends_with('-'), "{long}");
    assert_eq!(Slug::from_title(&"a".repeat(200)).as_str().len(), Slug::MAX_LEN);
    assert_eq!(Slug::parse("cay-bardagi"), Some(Slug::from_title("Çay Bardağı")));
    for bad in ["", "Cay", "cay--b", "-cay", "çay"] {
        assert!(Slug::parse(bad).is_none(), "{bad}");
    }
    let s: Slug = serde_json::from_str(r#""Çay Bardağı""#).unwrap();
    assert_eq!(s.as_str(), "cay-bardagi");
}

#[tokio::test]
async fn the_side_menu_folds_behind_a_menu_button_that_only_the_script_shows() {
    let router = error_app();
    let (_, _, body) = get(&router, "/nowhere", false).await;
    assert!(
        body.contains(r#"<button type="button" class="z-nav-toggle" aria-expanded="false" aria-controls="z-nav-body" data-zeyn-nav-toggle hidden>"#),
        "{body}"
    );
    assert!(body.contains(r#"<div class="z-nav-body" id="z-nav-body">"#));
    assert!(!body.contains(r#"id="z-nav-body" data-open"#), "the menu starts closed only when the script folds it");
}
