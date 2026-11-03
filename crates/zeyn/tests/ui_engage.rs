//! Components that react to the reader: the rating control, relative times, share buttons,
//! chips, heroes, reactions, the site shell's bottom bar and card previews.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{ReactionPress, ReactionState};
use zeyn::{Component, I18n, Identity, Render};

fn html(c: &impl Component, locale: &str) -> String {
    let mut out = String::new();
    c.render(&mut out, &Render::new("s", locale, I18n::builtin()));
    out
}

#[model]
pub struct Review {
    #[rule(required, min = 1, max = 5)]
    pub stars: Option<i64>,
    #[rule(max = 3)]
    pub effort: i64,
    pub note: String,
}

#[action(public)]
pub async fn save_review(review: Valid<Review>) -> Result<i64> {
    Ok(review.into_inner().stars.unwrap_or(0))
}

fn review_form(review: Review) -> zeyn::SubmitForm<Review, save_review> {
    Form::of(review)
        .field(Review::stars)
        .control(Review::stars, Control::Rating)
        .field(Review::effort)
        .control(Review::effort, Control::Rating)
        .submit("Save", save_review)
}

#[test]
fn a_rating_is_a_radio_group_from_one_to_the_fields_max() {
    let en = html(&review_form(Review { stars: Some(4), ..Review::default() }), "en");
    assert!(en.contains(r#"<fieldset class="z-rating" aria-describedby="s-stars-error"><legend>"#), "{en}");
    assert!(
        en.contains(r#"<input type="radio" class="z-rating-input" id="s-stars" name="stars" value="1""#),
        "the first star carries the field id for error messages"
    );
    assert!(en.contains(r#"id="s-stars-4" name="stars" value="4" data-testid="s.field.stars.4" checked required>"#));
    assert!(!en.contains(r#"name="stars" value="6""#), "five stars from the max rule");
    assert!(en.contains(r#"<span class="z-sr-only">4 of 5</span>"#), "each star is named for screen readers");
    assert!(en.contains(r#"name="effort" value="3""#) && !en.contains(r#"name="effort" value="4""#));
    assert!(!en.contains(r#"name="effort" value="1" data-testid="s.field.effort.1" checked"#));
    let tr = html(&review_form(Review::default()), "tr");
    assert!(tr.contains("3 / 5"), "{tr}");
    assert!(!tr.contains(r#"role="spinbutton" inputmode="numeric" autocomplete="off" class="z-num" id="s-stars""#));
}

#[test]
fn rating_values_are_checked_by_the_fields_rules_on_the_server() {
    let f = review_form(Review::default());
    let read = |v: serde_json::Value| {
        f.form()
            .read(v.as_object().unwrap(), "en", TimeZone::UTC)
    };
    assert_eq!(
        read(serde_json::json!({ "stars": "4", "effort": "2", "note": "" }))
            .unwrap()
            .stars,
        Some(4)
    );
    for bad in [
        serde_json::json!({ "stars": "9", "effort": "1" }),
        serde_json::json!({ "stars": "0", "effort": "1" }),
        serde_json::json!({ "effort": "1" }),
    ] {
        let errors = read(bad.clone()).unwrap_err();
        assert!(
            errors
                .errors
                .iter()
                .any(|e| e.field == "stars"),
            "{bad}"
        );
    }
}

#[test]
fn relative_times_carry_the_exact_moment_for_the_browser_to_rewrite() {
    let at = DateTime::from_unix_ms(1_778_580_000_000);
    let en = html(&RelativeTime::new(at), "en");
    assert!(en.starts_with(&format!(r#"<time class="z-relative" datetime="{at}" title=""#)), "{en}");
    assert!(en.contains("data-zeyn-relative>") && en.ends_with("</time>"));
    assert!(!en.contains("ago"), "the server writes the exact time, so cached pages stay the same");
    assert_eq!(en, html(&RelativeTime::new(at), "en"));
}

#[params]
pub struct ItemParams {
    pub id: i64,
}

#[screen(route = "/items/{id}", public)]
pub fn item(p: ItemParams) -> Screen {
    Screen::new("Item").body(Paragraph::new(p.id.to_string()))
}

#[test]
fn share_buttons_start_hidden_and_carry_their_link_and_texts() {
    let page = html(&Share::page(), "tr");
    assert!(
        page.contains(
            r#"<button type="button" class="z-button z-share" data-zeyn-share data-copied="Bağlantı kopyalandı.""#
        ),
        "{page}"
    );
    assert!(page.contains(" hidden>Paylaş</button>"), "{page}");
    assert!(!page.contains("data-url"));
    let other = html(
        &Share::screen(item, ItemParams { id: 7 })
            .title("<b>Seven</b>")
            .label("Send"),
        "en",
    );
    assert!(other.contains(r#"data-url="/_zeyn/route/item?params="#), "{other}");
    assert!(other.contains(r#"data-title="&lt;b&gt;Seven&lt;/b&gt;""#) && other.contains(">Send</button>"));
}

fn app() -> axum::Router {
    App::new("Test")
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
        .feature(Feature::new("items").screen(item))
        .router()
        .unwrap()
}

async fn get(path: &str) -> (StatusCode, String) {
    let res = app()
        .oneshot(
            Request::get(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

#[tokio::test]
async fn the_lazy_modules_are_served_and_stay_out_of_the_page_head() {
    let (_, page) = get("/items/1").await;
    for name in ["share", "chips", "hero", "reactions"] {
        let (status, body) = get(&format!("/_zeyn/{name}.js")).await;
        assert_eq!(status, StatusCode::OK, "{name}");
        assert!(body.contains("@module"), "{name}");
        assert!(!page.contains(&format!("/_zeyn/{name}.js")), "{name} loads only when a page holds it");
    }
    let (_, components) = get("/_zeyn/components.js").await;
    assert!(components.contains("['button[data-zeyn-share]', 'share']"));
    assert!(components.contains("['a[data-zeyn-chip]', 'chips']"));
    assert!(components.contains("['[data-zeyn-hero]', 'hero']"));
    assert!(components.contains("['[data-zeyn-reactions]', 'reactions']"));
}

#[params]
pub struct TagParams {
    pub tags: String,
}

#[screen(route = "/tagged", public)]
pub fn tagged(p: TagParams) -> Screen {
    Screen::new("Tagged").body(Paragraph::new(p.tags))
}

fn tag_chips(current: &str) -> Chips {
    Chips::filter("Tags", tagged, |tags| TagParams { tags: tags.join(",") })
        .current(current.split(','))
        .option("guide", "Guides")
        .option("news", "<News>")
        .option("release", "Releases")
}

/// The href of the chip with this value.
fn chip_href<'a>(html: &'a str, value: &str) -> &'a str {
    let at = html
        .find(&format!(r#"data-value="{value}""#))
        .unwrap();
    let start = html[..at].rfind("href=\"").unwrap() + 6;
    &html[start..start + html[start..].find('"').unwrap()]
}

/// The params of a `/_zeyn/route/...?params=` link, decoded.
fn params_of(href: &str) -> String {
    let encoded = href
        .split("?params=")
        .nth(1)
        .unwrap_or("");
    let mut out = Vec::new();
    let bytes = encoded.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            out.push(u8::from_str_radix(&encoded[i + 1..i + 3], 16).unwrap());
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap()
}

#[test]
fn filter_chips_render_the_pressed_state_and_the_toggled_params_from_the_server() {
    let one = html(&tag_chips("news,unknown"), "en");
    assert!(one.contains(r#"<div class="z-chips z-chips-filter" role="group" aria-label="Tags">"#), "{one}");
    assert_eq!(
        one.matches(r#"aria-pressed="true""#)
            .count(),
        1,
        "unknown values are ignored"
    );
    assert!(one.contains("&lt;News&gt;"), "texts are escaped");
    assert_eq!(params_of(chip_href(&one, "news")), r#"{"tags":""}"#, "pressing the pressed chip clears it");
    assert_eq!(params_of(chip_href(&one, "guide")), r#"{"tags":"guide"}"#, "one value at a time");

    let many = html(&tag_chips("release,guide").multiple(), "en");
    assert_eq!(
        many.matches(r#"aria-pressed="true""#)
            .count(),
        2
    );
    assert_eq!(params_of(chip_href(&many, "news")), r#"{"tags":"guide,news,release"}"#, "in the order of the options");
    assert_eq!(params_of(chip_href(&many, "guide")), r#"{"tags":"release"}"#);
    assert!(many.contains("data-zeyn-chip") && many.contains("data-zeyn-base-href"));
}

#[test]
fn link_chips_are_plain_links_in_a_named_nav() {
    let links = html(
        &Chips::links("Topics")
            .link("Guides", tagged, TagParams { tags: "guide".into() })
            .link("News", item, ItemParams { id: 3 }),
        "en",
    );
    assert!(
        links.starts_with(r#"<nav class="z-chips" aria-label="Topics"><ul class="z-chips-list"><li><a class="z-chip" href="/_zeyn/route/tagged?params="#),
        "{links}"
    );
    assert!(!links.contains("aria-pressed") && !links.contains("data-zeyn-chip"));
}

#[test]
fn a_hero_is_static_unless_it_has_slides() {
    let one = html(
        &Hero::new("Fresh <every> morning")
            .text("By noon")
            .image("/img/a.jpg")
            .action(Paragraph::new("Open daily")),
        "en",
    );
    assert!(one.starts_with(r#"<section class="z-hero"><div class="z-hero-slide" id="s-hero-"#), "{one}");
    assert!(one.contains(r#"<img class="z-hero-img" src="/img/a.jpg" alt="" fetchpriority="high" decoding="async">"#));
    assert!(one.contains(r#"<h2 class="z-hero-title">Fresh &lt;every&gt; morning</h2>"#));
    assert!(one.contains(r#"<div class="z-node" data-zeyn-node="0">"#), "actions are children with a path");
    assert!(!one.contains("data-zeyn-hero") && !one.contains("button") && !one.contains("aria-roledescription"));

    let outside = html(&Hero::new("A").image("https://elsewhere.example/a.jpg"), "en");
    assert!(!outside.contains("<img"), "pictures off the app's img-src are left out");
}

#[test]
fn hero_slides_follow_the_carousel_pattern_and_rotate_only_when_asked() {
    let still = html(
        &Hero::new("One")
            .slide("Two")
            .image("/img/b.jpg")
            .action(Paragraph::new("x"))
            .slide("Three"),
        "en",
    );
    assert!(still.contains(r#"aria-roledescription="carousel" aria-label="One" data-zeyn-hero>"#), "{still}");
    assert!(!still.contains("data-rotate") && !still.contains("data-zeyn-hero-pause"), "no rotation, no pause button");
    assert!(still.contains(r#"<div class="z-hero-controls" hidden>"#), "controls wait for the script");
    assert!(still.contains(r#"role="group" aria-roledescription="slide" aria-label="2 of 3" hidden>"#));
    assert!(still.contains(r#"aria-label="Show slide 1 of 3" data-zeyn-hero-show="0" aria-current="true">"#));
    assert!(still.contains(r#"<div class="z-hero-track" aria-live="polite">"#));
    assert!(still.contains(r#"alt="" loading="lazy""#), "later slides load their picture lazily");

    let moving = Hero::new("One").slide("Two").rotate(1);
    let en = html(&moving, "en");
    assert!(en.contains(r#"data-zeyn-hero data-rotate="4">"#), "rotation is at least four seconds");
    let pause = en.find("data-zeyn-hero-pause").unwrap();
    assert!(pause < en.find("data-zeyn-hero-show").unwrap(), "the pause button comes first");
    assert!(en.contains(r#"aria-label="Start the slides""#) && en.contains(r#"aria-live="off""#));
    assert_eq!(Hero::new("A").rotate(200).schema()["rotate"], 30);
    assert!(html(&moving, "tr").contains("Slayt 2 / 2 göster"));
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Echoes a press as a state: the target as the count key, so tests see what the action got.
#[action(permission = "items.react")]
pub async fn react(press: ReactionPress) -> Result<ReactionState> {
    let state = ReactionState::default().count(format!("{}:{}", press.target, press.reaction), 1);
    Ok(if press.pressed { state.mine(press.reaction) } else { state })
}

#[screen(route = "/answers/{id}", public)]
pub fn answer(p: ItemParams) -> Screen {
    Screen::new("Answer").body(
        Stack::new().add(
            Reactions::of(react, format!("answer-{}", p.id))
                .reaction("helpful", "Helpful", "+")
                .reaction("thanks", "<Thanks>", "!")
                .state(
                    ReactionState::default()
                        .count("helpful", 12)
                        .mine("helpful"),
                ),
        ),
    )
}

fn reactions_app() -> axum::Router {
    App::new("Test")
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
        .write_limit(zeyn::RateLimit::per_user(6, std::time::Duration::from_secs(60)))
        .feature(Feature::new("answers").screen(answer))
        .router()
        .unwrap()
}

async fn press(
    router: &axum::Router,
    perms: &str,
    csrf: bool,
    input: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let body =
        serde_json::json!({ "screen": "answer", "params": { "id": 7 }, "node": "0.0", "key": "react", "input": input });
    let mut req = Request::post("/_zeyn/data")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"));
    if csrf {
        req = req.header("x-zeyn-csrf", TOKEN);
    }
    let res = router
        .clone()
        .oneshot(
            req.body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[test]
fn reactions_are_toggle_buttons_with_their_counts() {
    let row = Reactions::of(react, "a")
        .label("Say thanks")
        .reaction("helpful", "Helpful", "+")
        .reaction("thanks", "<Thanks>", "!")
        .reaction("helpful", "Twice", "?")
        .state(
            ReactionState::default()
                .count("helpful", 12)
                .mine("helpful"),
        );
    let en = html(&row, "en");
    assert!(
        en.starts_with(r#"<div class="z-reactions" role="group" aria-label="Say thanks" data-zeyn-reactions"#),
        "{en}"
    );
    assert!(
        en.contains(r#"aria-pressed="true" data-reaction="helpful""#)
            && en.contains(r#"aria-pressed="false" data-reaction="thanks""#)
    );
    assert!(
        en.contains(r#"<span class="z-reaction-count">12</span>"#)
            && en.contains(r#"<span class="z-reaction-count">0</span>"#)
    );
    assert!(en.contains(
        r#"<span class="z-reaction-symbol" aria-hidden="true">+</span><span class="z-sr-only">Helpful</span>"#
    ));
    assert!(en.contains("&lt;Thanks&gt;") && !en.contains("Twice"), "names are escaped, keys are unique");
    assert!(html(&Reactions::of(react, "a"), "tr").contains(r#"aria-label="Tepkiler""#));
}

#[tokio::test]
async fn a_press_runs_the_action_with_the_screens_target_through_the_write_checks() {
    let router = reactions_app();
    let (status, state) =
        press(&router, "items.react", true, serde_json::json!({ "reaction": "thanks", "pressed": true })).await;
    assert_eq!(status, StatusCode::OK, "{state}");
    assert_eq!(state["counts"]["answer-7:thanks"], 1, "the target comes from the screen: {state}");
    assert_eq!(state["mine"], serde_json::json!(["thanks"]));

    let forged = serde_json::json!({ "reaction": "thanks", "pressed": true, "target": "answer-1" });
    let (_, state) = press(&router, "items.react", true, forged).await;
    assert!(state["counts"]["answer-7:thanks"] == 1, "a target from the browser is ignored: {state}");

    let (status, _) = press(&router, "items.react", true, serde_json::json!({ "reaction": "angry" })).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "reactions outside the set never reach the action");
    let (status, _) = press(&router, "", true, serde_json::json!({ "reaction": "thanks" })).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the action's permission");
    let (status, _) = press(&router, "items.react", false, serde_json::json!({ "reaction": "thanks" })).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the CSRF check");

    let mut last = StatusCode::OK;
    for _ in 0..4 {
        last = press(&router, "items.react", true, serde_json::json!({ "reaction": "helpful" }))
            .await
            .0;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS, "presses count against the write limit");
}

#[screen(route = "/shop", public)]
pub fn shop_home() -> Screen {
    Screen::new("Shop")
}

#[screen(route = "/shop/search", public)]
pub fn shop_search() -> Screen {
    Screen::new("Search")
}

#[screen(route = "/shop/orders", permission = "orders.read")]
pub fn shop_orders() -> Screen {
    Screen::new("Orders")
}

fn shop_app() -> App {
    App::new("Shop")
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
            Feature::new("shop")
                .screen(shop_home)
                .screen(shop_search)
                .screen(shop_orders)
                .menu(|m| {
                    m.group("Shop", Icon::Home)
                        .item("Home", shop_home)
                })
                .shell(Shell::site().bottom_nav([
                    zeyn::BottomNavItem::new("Home", shop_home).icon(Icon::Home),
                    zeyn::BottomNavItem::new("Search <now>", shop_search).icon(Icon::Search),
                    zeyn::BottomNavItem::new("Orders", shop_orders),
                ])),
        )
}

#[tokio::test]
async fn the_site_shell_bottom_bar_marks_the_current_page_and_hides_what_the_user_may_not_open() {
    let app = shop_app();
    assert_eq!(app.check(), Ok(()));
    let router = app.router().unwrap();
    let page = |perms: &'static str| {
        let router = router.clone();
        async move {
            let res = router
                .oneshot(
                    Request::get("/shop/search")
                        .header("x-perms", perms)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let bytes = res
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes();
            String::from_utf8_lossy(&bytes).into_owned()
        }
    };
    let guest = page("").await;
    let bar = &guest[guest
        .find(r#"<nav class="z-site-bottom" aria-label="Quick links"><ul>"#)
        .expect("the bar")..];
    assert!(bar.contains(r#"<li><a href="/shop"><span class="z-icon z-icon-home" aria-hidden="true"></span><span class="z-site-bottom-label">Home</span></a></li>"#), "{bar}");
    assert!(bar.contains(r#"<a href="/shop/search" aria-current="page">"#) && bar.contains("Search &lt;now&gt;"));
    assert!(!bar.contains("/shop/orders"), "screens the user may not open are left out");
    assert!(
        page("orders.read")
            .await
            .contains(r#"<li><a href="/shop/orders"><span class="z-site-bottom-label">Orders</span>"#)
    );
}
