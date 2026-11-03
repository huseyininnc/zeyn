//! Components that react to the reader: the rating control, relative times, share buttons,
//! chips, heroes, reactions, the site shell's bottom bar and card previews.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
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
    for name in ["share", "chips"] {
        let (status, body) = get(&format!("/_zeyn/{name}.js")).await;
        assert_eq!(status, StatusCode::OK, "{name}");
        assert!(body.contains("@module"), "{name}");
        assert!(!page.contains(&format!("/_zeyn/{name}.js")), "{name} loads only when a page holds it");
    }
    let (_, components) = get("/_zeyn/components.js").await;
    assert!(components.contains("['button[data-zeyn-share]', 'share']"));
    assert!(components.contains("['a[data-zeyn-chip]', 'chips']"));
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
