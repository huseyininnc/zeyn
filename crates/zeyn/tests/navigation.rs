use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::Prefetch;
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
    let mut app = App::new("Nav").feature(
        Feature::new("n")
            .screen(one)
            .screen(two),
    );
    if let Some(t) = theme {
        app = app.theme(t);
    }
    app.router().unwrap()
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
    assert_eq!(
        headers.get("x-zeyn-title").unwrap(),
        "First%20%3Cscreen%3E%20%C2%B7%20Nav"
    );
    assert_eq!(
        headers
            .get(header::CACHE_CONTROL)
            .unwrap(),
        "no-store"
    );
    assert!(body.starts_with(r#"<section class="z-screen""#));
    assert!(body.contains("First &lt;screen&gt;"));
    assert!(!body.contains("<html"));
}

#[tokio::test]
async fn partial_requests_that_fail_get_the_error_screen_alone_with_its_status() {
    let (status, headers, body) = get(&app(None), "/two", true).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(headers.get("x-zeyn-partial").unwrap(), "1");
    assert!(
        body.starts_with(r#"<section class="z-screen z-error""#),
        "{body}"
    );
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
        .feature(
            Feature::new("n")
                .screen(one)
                .screen(boom)
                .screen(panics)
                .screen(order),
        )
        .router()
        .unwrap()
}

#[tokio::test]
async fn unknown_paths_and_missing_records_get_the_apps_404_screen_in_the_shell() {
    let router = error_app();
    for path in ["/nowhere", "/orders/404", "/orders/abc"] {
        let (status, _, body) = get(&router, path, false).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert!(
            body.contains("<html") && body.contains(r#"<nav class="z-nav""#),
            "{path} is inside the shell"
        );
        assert!(
            body.contains("Lost &lt;here&gt;") && body.contains("Try the menu"),
            "{path}: {body}"
        );
        assert!(body.contains("<title>Lost &lt;here&gt; · Nav</title>"));
    }
    let (status, headers, body) = get(&router, "/nowhere", true).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(headers.get("x-zeyn-partial").unwrap(), "1");
    assert_eq!(
        headers.get("x-zeyn-title").unwrap(),
        "Lost%20%3Chere%3E%20%C2%B7%20Nav"
    );
    assert!(
        body.starts_with(r#"<section class="z-screen" data-zeyn-screen="missing""#),
        "{body}"
    );
}

#[tokio::test]
async fn errors_and_panics_get_a_500_without_details() {
    let router = error_app();
    for (path, partial) in [
        ("/boom", false),
        ("/boom", true),
        ("/panic", false),
        ("/panic", true),
    ] {
        let (status, _, body) = get(&router, path, partial).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{path}");
        assert!(body.contains("Something broke"), "{path}: {body}");
        assert!(
            !body.contains("hunter2")
                && !body.contains("secret panic")
                && !body.contains("database"),
            "{path}"
        );
        assert_eq!(body.contains("<html"), !partial, "{path}");
    }
    let (status, _, body) = get(&router, "/_zeyn/screen/panics", false).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!body.contains("secret panic"));
    assert!(
        body.contains(r#""error":"internal""#),
        "endpoints answer JSON: {body}"
    );
}

#[tokio::test]
async fn without_error_screens_or_when_they_fail_the_built_in_message_is_shown() {
    let plain = App::new("Nav")
        .feature(Feature::new("n").screen(boom))
        .router()
        .unwrap();
    let (status, _, body) = get(&plain, "/nowhere", false).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains(r#"<section class="z-screen z-error""#));
    let failing = App::new("Nav")
        .server_error(broken_error_screen)
        .feature(Feature::new("n").screen(boom))
        .router()
        .unwrap();
    let (status, _, body) = get(&failing, "/boom", false).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body.contains(r#"<section class="z-screen z-error""#) && !body.contains("hunter2"));
}

#[tokio::test]
async fn the_navigation_script_is_served() {
    let (status, headers, body) = get(&app(None), "/_zeyn/nav.js", false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .unwrap(),
        "text/javascript; charset=utf-8"
    );
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
    Ok(
        Screen::new(format!("Order {} {}", p.id, p.tab.unwrap_or_default()))
            .body(Stack::new().add(tabs))
            .body(
                Breadcrumb::new()
                    .link(
                        "Cups",
                        catalog,
                        CatalogParams {
                            slug: Slug::from_title("Çay Bardağı"),
                            n: 2,
                        },
                    )
                    .current("Order"),
            ),
    )
}

#[params]
pub struct CatalogParams {
    pub slug: Slug,
    pub n: u32,
}

#[screen(route = "/catalog/{slug}/{n}", public)]
pub fn catalog(p: CatalogParams) -> Screen {
    if p.slug.as_str() == "old-name" {
        return Screen::moved_to(
            catalog,
            CatalogParams {
                slug: Slug::from_title("New Name"),
                n: p.n,
            },
        );
    }
    Screen::new(format!("Catalog {} page {}", p.slug, p.n))
}

/// Sends every visit on to one catalog page, like a random pick.
#[screen(route = "/pick", public)]
pub fn pick() -> Screen {
    Screen::redirect(
        catalog,
        CatalogParams {
            slug: Slug::from_title("New Name"),
            n: 7,
        },
    )
}

/// A screen with a route that the app does not register.
#[screen(route = "/orphan", public)]
pub fn orphan() -> Screen {
    Screen::new("Orphan")
}

/// Redirects to a screen the app does not have.
#[screen(route = "/lost", public)]
pub fn lost() -> Screen {
    Screen::redirect(orphan, ())
}

#[params]
pub struct SecretParams {
    pub id: i64,
}

#[screen(route = "/secret/{id}", permission = "secret.read")]
pub fn secret(p: SecretParams) -> Screen {
    Screen::new(format!("Secret {}", p.id))
}

#[screen(route = "/empty", public)]
pub fn empty_page() -> Screen {
    Screen::new("Empty")
        .body(
            Hero::new("Welcome")
                .image("/hero.jpg")
                .link("See order", order, OrderParams { id: 7, tab: None })
                .link("Home", one, ()),
        )
        .body(EmptyState::new("Nothing here").link(
            "Browse cups",
            catalog,
            CatalogParams {
                slug: Slug::from_title("Cups"),
                n: 1,
            },
        ))
}

fn path_app() -> axum::Router {
    App::new("Nav")
        .login_url("/login")
        .feature(
            Feature::new("n")
                .screen(empty_page)
                .screen(one)
                .screen(order)
                .screen(catalog)
                .screen(pick)
                .screen(lost)
                .screen(secret),
        )
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
    assert!(
        body.contains(r#"data-zeyn-params="{&quot;id&quot;:42}""#),
        "{body}"
    );

    let (status, _, body) = get(
        &router,
        "/orders/42?params=%7B%22tab%22%3A%22lines%22%7D",
        false,
    )
    .await;
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
    assert_eq!(
        headers.get(header::LOCATION).unwrap(),
        "/catalog/new-name/3"
    );
}

#[tokio::test]
async fn a_screen_can_send_one_request_on_to_another_screen_without_caching() {
    let router = path_app();
    for partial in [false, true] {
        let (status, headers, _) = get(&router, "/pick", partial).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "partial {partial}");
        assert_eq!(
            headers.get(header::LOCATION).unwrap(),
            "/catalog/new-name/7"
        );
        assert_eq!(
            headers
                .get(header::CACHE_CONTROL)
                .unwrap(),
            "no-store"
        );
    }
    let (status, headers, _) = get(&router, "/lost", false).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a target the app does not serve is not followed"
    );
    assert!(headers.get(header::LOCATION).is_none());
    let req = Request::get("/_zeyn/nav.js")
        .body(Body::empty())
        .unwrap();
    let res = router
        .clone()
        .oneshot(req)
        .await
        .unwrap();
    let js = String::from_utf8(
        res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(
        js.contains(
            "else if (res.redirected) history.replaceState({ zeyn: true }, '', finalUrl.href);"
        ),
        "a redirected partial load that adds no history entry still shows the target's address"
    );
}

#[tokio::test]
async fn framework_links_use_the_path_form() {
    let router = path_app();
    let (status, headers, _) = get(
        &router,
        "/_zeyn/route/order?params=%7B%22id%22%3A42%2C%22tab%22%3A%22a.b%22%7D",
        false,
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        headers.get(header::LOCATION).unwrap(),
        "/orders/42?params=%7B%22tab%22%3A%22a%2Eb%22%7D"
    );
    let (status, _, _) = get(
        &router,
        "/_zeyn/route/order?params=%7B%22tab%22%3A1%7D",
        false,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "params without the segment field cannot fill the route"
    );
    let (_, headers, _) = get(&router, "/_zeyn/route/one", false).await;
    assert_eq!(headers.get(header::LOCATION).unwrap(), "/one");

    let (_, _, body) = get(&router, "/orders/42", true).await;
    assert!(
        body.contains(r#"<a href="/catalog/cay-bardagi/2" data-zeyn-base-href>Cups</a>"#),
        "{body}"
    );
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
    let res = router
        .clone()
        .oneshot(req)
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let html = String::from_utf8(
        res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("Lines of 42"), "{html}");

    let (status, _, html) = get(
        &router,
        "/_zeyn/screen/order?params=%7B%22id%22%3A7%7D",
        false,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(html.contains("Order 7"));
}

#[tokio::test]
async fn path_screens_check_access_and_return_to_keeps_the_path() {
    let router = path_app();
    let (status, headers, _) = get(&router, "/secret/5?params=%7B%7D", false).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(
        headers.get(header::LOCATION).unwrap(),
        "/login?return_to=%2Fsecret%2F5%3Fparams%3D%257B%257D"
    );
    let (status, _, _) = get(
        &router,
        "/_zeyn/screen/secret?params=%7B%22id%22%3A5%7D",
        false,
    )
    .await;
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
    let problems = App::new("Nav")
        .home(order)
        .feature(
            Feature::new("n")
                .screen(order)
                .screen(order_by_code),
        )
        .check()
        .unwrap_err();
    assert!(
        problems
            .iter()
            .any(|p| p
                .contains("route `/orders/{code}` is used by screens `order` and `order_by_code`")),
        "{problems:?}"
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("home screen `order` has path segments")),
        "{problems:?}"
    );
}

#[test]
fn slugs_map_turkish_letters_fold_accents_and_drop_the_rest() {
    assert_eq!(Slug::from_title("Çay Bardağı").as_str(), "cay-bardagi");
    assert_eq!(
        Slug::from_title("İSTANBUL ışık Şeker Ğ Ö Ü").as_str(),
        "istanbul-isik-seker-g-o-u"
    );
    assert_eq!(Slug::from_title("Ünlü -- Çiçek!!").as_str(), "unlu-cicek");
    assert_eq!(
        Slug::from_title("Straße Æon Œuvre Łódź").as_str(),
        "strasse-aeon-oeuvre-lodz"
    );
    assert_eq!(Slug::from_title("日本 tea ☕ time").as_str(), "tea-time");
    assert_eq!(Slug::from_title("  ").as_str(), "");
    let long = Slug::from_title(&"word ".repeat(40));
    assert!(
        long.as_str().len() <= Slug::MAX_LEN && !long.as_str().ends_with('-'),
        "{long}"
    );
    assert_eq!(
        Slug::from_title(&"a".repeat(200))
            .as_str()
            .len(),
        Slug::MAX_LEN
    );
    assert_eq!(
        Slug::parse("cay-bardagi"),
        Some(Slug::from_title("Çay Bardağı"))
    );
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
    assert!(
        !body.contains(r#"id="z-nav-body" data-open"#),
        "the menu starts closed only when the script folds it"
    );
}

#[tokio::test]
async fn link_buttons_are_real_links_to_canonical_paths_in_empty_states_and_heroes() {
    let (status, _, body) = get(&path_app(), "/empty", true).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains(r#"<a class="z-button z-link-button z-primary" href="/orders/7" data-zeyn-base-href data-testid="empty_page.link.see-order">See order</a>"#),
        "{body}"
    );
    assert!(
        body.contains(r#"<a class="z-button z-link-button" href="/one" data-zeyn-base-href"#),
        "a second hero link has the secondary look: {body}"
    );
    assert!(
        body.contains(r#"<a class="z-button z-link-button z-primary" href="/catalog/cups/1""#),
        "{body}"
    );
    assert!(!body.contains("/_zeyn/route/"), "{body}");
    assert!(!body.contains("data-zeyn-effect"), "{body}");
}

#[screen(route = "/counted", public)]
pub fn counted(_: (), ctx: Ctx) -> Screen {
    Screen::new(if ctx.is_prefetch() { "Ahead" } else { "Seen" })
}

#[screen(route = "/checkout", public, no_prefetch)]
pub fn checkout() -> Screen {
    Screen::new("Checkout")
}

fn prefetch_app(on: bool) -> axum::Router {
    let app = App::new("Nav").feature(
        Feature::new("n")
            .screen(counted)
            .screen(checkout),
    );
    let app = if on {
        app.prefetch(Prefetch::Moderate)
    } else {
        app
    };
    app.router().unwrap()
}

async fn get_with(router: &axum::Router, path: &str, header: (&str, &str)) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(
            Request::get(path)
                .header(header.0, header.1)
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
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn prefetch_sends_speculation_rules_by_header_and_screens_see_prefetches() {
    let (_, headers, page) = get(&prefetch_app(false), "/counted", false).await;
    assert!(
        headers
            .get("speculation-rules")
            .is_none(),
        "off by default"
    );
    assert!(!page.contains("zeyn-prefetch"));

    let router = prefetch_app(true);
    let (_, headers, page) = get(&router, "/counted", false).await;
    assert_eq!(
        headers
            .get("speculation-rules")
            .unwrap(),
        r#""/_zeyn/speculation.json""#
    );
    assert!(page.contains(r#"<meta name="zeyn-prefetch" content="moderate">"#));
    assert!(
        !page.contains("speculationrules"),
        "no inline rules, the CSP stays"
    );
    let (_, partial_headers, _) = get(&router, "/counted", true).await;
    assert!(
        partial_headers
            .get("speculation-rules")
            .is_none()
    );

    let (status, headers, rules) = get(&router, "/_zeyn/speculation.json", false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .unwrap(),
        "application/speculationrules+json"
    );
    let rules: serde_json::Value = serde_json::from_str(&rules).unwrap();
    assert!(rules.get("prerender").is_none());
    assert_eq!(rules["prefetch"][0]["eagerness"], "moderate");
    let text = rules.to_string();
    assert!(
        text.contains("/_zeyn/*")
            && text.contains("data-zeyn-reload")
            && text.contains("[download]")
    );

    let (status, body) = get_with(&router, "/counted", ("sec-purpose", "prefetch")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Ahead"), "{body}");
    let (_, body) = get_with(&router, "/counted", ("x-zeyn-prefetch", "1")).await;
    assert!(body.contains("Ahead"));
    let (_, body) = get_with(&router, "/counted", ("accept", "text/html")).await;
    assert!(body.contains("Seen"));
    let (status, body) = get_with(
        &router,
        "/checkout",
        ("sec-purpose", "prefetch;anonymous-client-ip"),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(!body.contains("Checkout"), "refused before the screen runs");
    let (status, _) = get_with(&router, "/checkout", ("accept", "text/html")).await;
    assert_eq!(status, StatusCode::OK);
}

/// Params whose fields are all optional, such as a filter a menu link leaves out.
#[params]
pub struct FeedParams {
    pub show: Option<String>,
    pub page: Option<u32>,
}

#[screen(route = "/feed", public)]
pub fn feed(p: FeedParams) -> Screen {
    Screen::new(format!(
        "Feed {} {}",
        p.show.unwrap_or_else(|| "all".into()),
        p.page.unwrap_or(1)
    ))
}

/// Params with a required field.
#[params]
pub struct NeedsId {
    pub id: i64,
}

#[screen(route = "/needs-id", public)]
pub fn needs_id(p: NeedsId) -> Screen {
    Screen::new(format!("Item {}", p.id))
}

#[tokio::test]
async fn screens_whose_params_are_all_optional_open_without_params() {
    let router = App::new("Opt")
        .feature(
            Feature::new("o")
                .screen(feed)
                .screen(needs_id)
                .menu(|m| {
                    m.group("Feed", Icon::Home)
                        .item("Feed", feed)
                }),
        )
        .router()
        .unwrap();
    let (status, _, page) = get(&router, "/feed", false).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains("Feed all 1"), "{page}");
    let (status, _, page) = get(
        &router,
        "/feed?params=%7B%22show%22%3A%22unread%22%7D",
        false,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("Feed unread 1"));
    let (status, _, _) = get(&router, "/_zeyn/screen/feed", true).await;
    assert_eq!(status, StatusCode::OK, "fragments open without params too");
    let (status, _, _) = get(&router, "/needs-id", false).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a required field still needs its value"
    );
    let (status, _, _) = get(&router, "/feed?params=%7B%22page%22%3A%22x%22%7D", false).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "values of the wrong type are refused"
    );
    assert!(
        <FeedParams as zeyn::ApiInput>::from_json(serde_json::Value::Null).is_ok(),
        "the API reads a missing value the same way"
    );
}

#[tokio::test]
async fn the_side_menu_starts_with_the_logo_and_draws_item_icons() {
    let router = App::new("Nav")
        .logo("/static/logo.svg")
        .logo_dark("/static/logo-dark.svg")
        .feature(Feature::new("n").screen(one).menu(|m| {
            m.group("Group", Icon::Box)
                .item("One", one)
                .icon(Icon::Home)
        }))
        .router()
        .unwrap();
    let (_, _, page) = get(&router, "/one", false).await;
    assert!(
        page.contains(r#"<p class="z-app-name"><img class="z-logo z-logo-light" src="/static/logo.svg" alt=""><img class="z-logo z-logo-dark" src="/static/logo-dark.svg" alt=""><span>Nav</span></p>"#),
        "{page}"
    );
    assert!(
        page.contains(r#"<h2><button type="button" class="z-nav-heading" aria-expanded="true" aria-controls="z-nav-g0"><span>Group</span>"#),
        "the group label drops its icon"
    );
    assert!(page.contains(r#"aria-current="page"><span class="z-icon z-icon-home" aria-hidden="true"></span><span>One</span></a>"#));

    let (_, _, plain) = get(&app(None), "/one", false).await;
    assert!(plain.contains(
        r#"<p class="z-app-name"><span class="z-logo z-logo-letter" aria-hidden="true">N</span><span>Nav</span></p>"#
    ));
}

#[tokio::test]
async fn the_top_bar_has_a_breadcrumb_a_search_field_and_the_account_menu() {
    let router = App::new("Nav")
        .header(|h| h.breadcrumb().search().account())
        .feature(Feature::new("n").screen(one).menu(|m| {
            m.group("Group", Icon::Box)
                .item("One", one)
        }))
        .router()
        .unwrap();
    let (_, _, page) = get(&router, "/one", false).await;
    assert!(page.contains(r#"<div class="z-app-header-slot z-app-header-slot-start" id="z-app-header-slot-start"><nav class="z-crumbs" aria-label="Breadcrumb" data-zeyn-crumbs><ol><li>Group</li><li><span aria-current="page">One</span></li></ol></nav></div>"#), "{page}");
    assert!(
        page.contains(r#"<button type="button" class="z-topbar-search" data-zeyn-palette-open"#)
    );
    assert!(page.contains(r#"aria-controls="z-account" aria-label="Account">"#));
    let nav = &page[page
        .find(r#"<nav class="z-nav""#)
        .unwrap()..page.find("</nav>").unwrap()];
    assert!(
        !nav.contains("data-zeyn-theme-picker") && !nav.contains("z-palette-open"),
        "the pickers and the palette button move to the top bar"
    );
    let bar = &page[page
        .find(r#"<header class="z-app-header""#)
        .unwrap()..];
    assert!(bar.contains("data-zeyn-theme-picker"));
}

#[test]
fn a_logo_from_another_origin_is_reported() {
    for bad in [
        "https://cdn.example.com/logo.svg",
        "//cdn.example.com/x.svg",
        "logo.svg",
        "/a\"b.svg",
    ] {
        let problems = App::new("Nav")
            .logo(bad)
            .check()
            .unwrap_err();
        assert!(
            problems
                .iter()
                .any(|p| p.contains("is not a path on the app's origin")),
            "{bad}"
        );
    }
}

#[tokio::test]
async fn navigation_never_leaves_the_app_origin() {
    let (_, _, js) = get(&app(None), "/_zeyn/nav.js", false).await;
    let handler = &js[js
        .find("addEventListener('zeyn:navigate'")
        .unwrap()..];
    let handler = &handler[..handler.find("\n});").unwrap()];
    let canceled = handler
        .find("e.preventDefault()")
        .unwrap();
    let checked = handler
        .find("url.origin !== location.origin")
        .unwrap();
    assert!(
        canceled < checked,
        "a request for another origin (or javascript:, data:) is canceled, so the sender does not load it either"
    );
    assert!(handler.contains("console.warn"));
    assert_eq!(
        js.matches("if (!sameOrigin(res))")
            .count(),
        2,
        "a fetch that a redirect took to another origin is never swapped in or pushed to history"
    );
}

#[screen(route = "/three", public)]
pub fn three() -> Screen {
    Screen::new("Third")
}

#[screen(route = "/four", public)]
pub fn four() -> Screen {
    Screen::new("Fourth")
}

/// The side menu of a nested menu: `Work` holds `One` and the subgroup `Deep` with `Three` and
/// the subgroup `Hidden`, whose only item needs a permission the visitor does not have; `Other`
/// holds `Four`.
fn nested(accordion: bool) -> axum::Router {
    App::new("Nav")
        .header(|h| h.breadcrumb())
        .feature(
            Feature::new("n")
                .screen(one)
                .screen(two)
                .screen(three)
                .screen(four)
                .menu(|m| {
                    let m = m
                        .group("Work", Icon::Box)
                        .item("One", one)
                        .sub("Deep", |m| {
                            m.item("Three", three)
                                .sub("Hidden", |m| m.item("Two", two))
                        })
                        .group("Other", Icon::Home)
                        .item("Four", four);
                    if accordion { m.accordion() } else { m }
                }),
        )
        .router()
        .unwrap()
}

/// The `<nav class="z-nav">` element of a page.
fn side_menu(page: &str) -> &str {
    &page[page
        .find(r#"<nav class="z-nav""#)
        .unwrap()..page.find("</nav>").unwrap()]
}

#[tokio::test]
async fn nested_menu_groups_render_as_disclosures_open_on_the_current_branch() {
    let (_, _, page) = get(&nested(false), "/three", false).await;
    let nav = side_menu(&page);
    assert!(
        nav.starts_with(r#"<nav class="z-nav" aria-label="Main" data-zeyn-menu>"#),
        "{nav}"
    );
    assert!(
        nav.contains(r#"<div class="z-nav-group"><h2><button type="button" class="z-nav-heading" aria-expanded="true" aria-controls="z-nav-g0"><span class="z-icon z-icon-box" aria-hidden="true"></span><span>Work</span><span class="z-nav-caret" aria-hidden="true"></span></button></h2><ul id="z-nav-g0"><li><a href="/one">One</a></li><li class="z-nav-group z-nav-sub"><h3><button type="button" class="z-nav-heading" aria-expanded="true" aria-controls="z-nav-g0-0"><span>Deep</span><span class="z-nav-caret" aria-hidden="true"></span></button></h3><ul id="z-nav-g0-0"><li><a href="/three" aria-current="page">Three</a></li></ul></li></ul></div>"#),
        "the current branch is open and nested in the group: {nav}"
    );
    assert!(
        nav.contains(r#"aria-expanded="false" aria-controls="z-nav-g1"><span class="z-icon z-icon-home" aria-hidden="true"></span><span>Other</span>"#),
        "groups away from the current screen start closed: {nav}"
    );
    assert!(
        !nav.contains("Hidden") && !nav.contains(r#"href="/two""#),
        "a subgroup without items the user may open is left out"
    );
    assert!(
        page.contains(
            r#"<ol><li>Work</li><li>Deep</li><li><span aria-current="page">Three</span></li></ol>"#
        ),
        "the breadcrumb holds the whole group path"
    );
    assert!(page.contains(r#"<script type="module" src="/_zeyn/menu.js?v="#));

    let (_, _, home) = get(&nested(false), "/one", false).await;
    let nav = side_menu(&home);
    assert!(nav.contains(r#"aria-expanded="true" aria-controls="z-nav-g0">"#));
    assert!(
        nav.contains(r#"aria-expanded="false" aria-controls="z-nav-g0-0">"#),
        "a subgroup without the current screen starts closed: {nav}"
    );
}

#[tokio::test]
async fn an_accordion_menu_marks_the_side_menu() {
    let (_, _, page) = get(&nested(true), "/one", false).await;
    assert!(side_menu(&page).starts_with(
        r#"<nav class="z-nav" aria-label="Main" data-zeyn-menu data-zeyn-accordion>"#
    ));
}

#[tokio::test]
async fn an_open_group_has_no_toggle_and_loads_no_menu_script() {
    let router = App::new("Nav")
        .feature(Feature::new("n").screen(one).menu(|m| {
            m.group("Group", Icon::Box)
                .open()
                .item("One", one)
        }))
        .router()
        .unwrap();
    let (_, _, page) = get(&router, "/one", false).await;
    let nav = side_menu(&page);
    assert!(nav.starts_with(r#"<nav class="z-nav" aria-label="Main">"#));
    assert!(nav.contains(r#"<div class="z-nav-group"><h2><span class="z-icon z-icon-box" aria-hidden="true"></span>Group</h2><ul><li>"#), "{nav}");
    assert!(!page.contains("/_zeyn/menu.js"));
}

#[tokio::test]
async fn nested_groups_with_the_same_label_merge_across_features() {
    let router = App::new("Nav")
        .feature(Feature::new("a").screen(one).menu(|m| {
            m.group("Work", Icon::Box)
                .sub("Deep", |m| m.item("One", one))
        }))
        .feature(
            Feature::new("b")
                .screen(three)
                .menu(|m| {
                    m.group("Work", Icon::Box)
                        .sub("Deep", |m| m.item("Three", three))
                }),
        )
        .router()
        .unwrap();
    let (_, _, page) = get(&router, "/three", false).await;
    let nav = side_menu(&page);
    assert_eq!(nav.matches(">Work<").count(), 1, "{nav}");
    assert_eq!(nav.matches(">Deep<").count(), 1, "{nav}");
    assert!(
        nav.contains(r#"<ul id="z-nav-g0-0"><li><a href="/one">One</a></li><li><a href="/three" aria-current="page">Three</a></li></ul>"#),
        "{nav}"
    );
}

#[test]
#[should_panic(expected = "at most 3 levels of `sub` below a `group`")]
fn menus_nest_at_most_three_levels_below_a_group() {
    let _ = Feature::new("n").screen(one).menu(|m| {
        m.group("A", Icon::Box).sub("B", |m| {
            m.sub("C", |m| m.sub("D", |m| m.sub("E", |m| m.item("One", one))))
        })
    });
}

#[test]
fn three_levels_of_nested_groups_are_allowed() {
    let f = Feature::new("n").screen(one).menu(|m| {
        m.group("A", Icon::Box)
            .sub("B", |m| m.sub("C", |m| m.sub("D", |m| m.item("One", one))))
    });
    assert_eq!(f.menu_groups()[0].depth(), 3);
    assert_eq!(
        f.menu_groups()[0]
            .find("one")
            .unwrap()
            .0
            .len(),
        4
    );
}
