//! Shop components: prices, range sliders, quantity steppers, variant pickers, product
//! galleries, facet panels, review summaries and countdowns.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Component, I18n, Identity, Render};

fn html(c: &impl Component, locale: &str) -> String {
    let mut out = String::new();
    c.render(&mut out, &Render::new("s", locale, I18n::builtin()));
    out
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[screen(route = "/plain", public)]
pub fn plain() -> Screen {
    Screen::new("Plain").body(Paragraph::new("Only text"))
}

#[screen(route = "/price", public)]
pub fn priced() -> Screen {
    Screen::new("Price")
        .body(Stack::new().add(Price::new(Money::tl(129_900)).was(Money::tl(159_900))))
}

fn shop_app() -> App {
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
        .write_limit(zeyn::RateLimit::per_user(
            8,
            std::time::Duration::from_secs(60),
        ))
        .feature(
            Feature::new("shop")
                .screen(plain)
                .screen(priced),
        )
}

async fn get(path: &str) -> (StatusCode, String) {
    let res = shop_app()
        .router()
        .unwrap()
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

/// Posts a component data call and returns the status and the JSON answer.
#[allow(dead_code)]
async fn data(
    router: &axum::Router,
    perms: &str,
    csrf: bool,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
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
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

/// Names of the framework scripts and styles a page links.
fn links(page: &str) -> Vec<String> {
    page.split("/_zeyn/")
        .skip(1)
        .filter_map(|s| {
            let name = s.split('?').next()?;
            (name.ends_with(".js") || name.ends_with(".css")).then(|| name.to_string())
        })
        .collect()
}

#[test]
fn a_price_reads_as_one_sentence_with_the_struck_price_hidden_from_screen_readers() {
    let price = Price::new(Money::tl(129_900))
        .was(Money::tl(159_900))
        .unit(Money::tl(25_980), PriceUnit::Kg);
    let en = html(&price, "en");
    assert!(
        en.contains(r#"<span class="z-sr-only">Was ₺1,599.00, now ₺1,299.00, 19% off</span>"#),
        "{en}"
    );
    assert!(en.contains(r#"<span class="z-price-row" aria-hidden="true">"#));
    assert!(
        en.contains(r#"<s class="z-price-was">₺1,599.00</s><span class="z-price-off">-19%</span>"#)
    );
    assert!(en.contains(r#"<span class="z-price-unit">₺259.80 per kg</span>"#));
    let tr = html(&price, "tr");
    assert!(
        tr.contains("Eski fiyat 1.599,00 ₺, yeni fiyat 1.299,00 ₺, %19 indirim"),
        "{tr}"
    );
    assert!(tr.contains("Kg fiyatı 259,80 ₺"));
    assert!(
        !en.contains("<script") && !en.contains("data-zeyn-"),
        "no script"
    );
}

#[test]
fn an_old_price_that_is_not_higher_or_in_another_currency_is_left_out() {
    for was in [
        Money::tl(129_900),
        Money::tl(99_900),
        Money::new(159_900, Currency::Usd),
    ] {
        let out = html(
            &Price::new(Money::tl(129_900))
                .was(was)
                .large(),
            "en",
        );
        assert!(
            !out.contains("z-price-was") && !out.contains("z-sr-only"),
            "{out}"
        );
        assert!(out.starts_with(r#"<span class="z-price z-price-lg">"#));
    }
}

#[tokio::test]
async fn the_shop_stylesheet_comes_only_with_shop_components() {
    let (status, page) = get("/price").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(links(&page).contains(&"commerce.css".to_string()));
    let (_, text) = get("/plain").await;
    assert!(!links(&text).contains(&"commerce.css".to_string()));
    let (status, css) = get("/_zeyn/commerce.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(css.contains(".z-price"));
}
