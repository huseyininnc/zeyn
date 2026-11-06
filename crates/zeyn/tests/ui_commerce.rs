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

#[params]
#[derive(Default)]
pub struct Listing {
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
}

fn price_slider(p: &Listing) -> RangeSlider {
    RangeSlider::new("Price", ["min_price", "max_price"], 0, 5_000)
        .step(50)
        .money(Currency::Try)
        .value(p.min_price, p.max_price)
}

#[screen(route = "/range", public)]
pub fn ranged(p: Listing) -> Screen {
    Screen::new("Range")
        .body(price_slider(&p).drives("listing"))
        .body(Kpi::new("Shown", 1.0).listens("listing"))
}

/// Keeps at most 7 units and echoes the target, so tests see what the action got.
#[action(permission = "cart.edit")]
pub async fn set_quantity(change: zeyn::ui::QuantityChange) -> Result<zeyn::ui::QuantityState> {
    if change.target != "sku-7" {
        return Err(Error::NotFound);
    }
    Ok(zeyn::ui::QuantityState::new(change.quantity.min(7)).max(7))
}

#[params]
pub struct ItemParams {
    pub id: i64,
}

#[screen(route = "/cart/{id}", public)]
pub fn cart_line(p: ItemParams) -> Screen {
    Screen::new("Cart").body(
        Stack::new().add(
            QuantityStepper::of(set_quantity, format!("sku-{}", p.id))
                .range(1, 20)
                .value(2),
        ),
    )
}

#[entity(table = "goods")]
pub struct Good {
    #[key]
    pub id: GoodId,
    pub name: String,
    pub color: String,
    pub price: Money,
}

/// Sample goods: a color each of red, blue and green, prices rising with the id.
fn goods() -> Vec<Good> {
    (1..=30)
        .map(|i| Good {
            id: GoodId(i),
            name: format!("Good {i}"),
            color: ["red", "blue", "green"][(i % 3) as usize].to_string(),
            price: Money::tl(i * 1_000),
        })
        .collect()
}

#[action(public)]
pub async fn search_goods(q: DataQuery) -> Result<Page<Good>> {
    Ok(q.apply(goods()))
}

#[screen(route = "/goods", public)]
pub fn good_cards() -> Screen {
    Screen::new("Goods").body(
        CardGrid::of(search_goods)
            .card(|g: &Good| {
                Card::image("/g.webp", g.name.clone())
                    .price(Price::new(g.price).was(Money::tl(g.price.minor * 2)))
            })
            .paging(Paging::infinite(4)),
    )
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
                .screen(priced)
                .screen(ranged)
                .screen(cart_line)
                .screen(good_cards),
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

#[test]
fn a_range_slider_is_two_named_native_sliders_with_formatted_values() {
    let slider = price_slider(&Listing {
        min_price: Some(500),
        max_price: Some(9_000),
    });
    let en = html(&slider, "en");
    assert!(
        en.starts_with(
            r#"<div class="z-range" role="group" aria-labelledby="s-range--label" data-zeyn-range"#
        ),
        "{en}"
    );
    assert!(
        en.contains(r#"style="--z-lo:10.00%;--z-hi:100.00%""#),
        "{en}"
    );
    assert!(en.contains(
        r#"<input type="range" class="z-range-input z-range-low" id="s-range--low" name="min_price" min="0" max="5000" step="50" value="500" aria-label="Price, from" aria-valuetext="₺500.00""#
    ), "{en}");
    assert!(
        en.contains(r#"name="max_price" min="0" max="5000" step="50" value="5000" aria-label="Price, to" aria-valuetext="₺5,000.00""#),
        "a value out of the bounds is clamped: {en}"
    );
    assert!(
        !en.contains("<form"),
        "without drives it is no form of its own"
    );
    let tr = html(&slider, "tr");
    assert!(tr.contains(r#"aria-valuetext="500,00 ₺""#) && tr.contains("Price, başlangıç"));
    let swapped = html(
        &RangeSlider::new("Count", ["a", "b"], 10, 0).value(Some(8), Some(2)),
        "en",
    );
    assert!(
        swapped.contains(r#"name="a" min="0" max="10" step="1" value="2""#),
        "{swapped}"
    );
}

#[test]
fn a_date_range_slider_sends_iso_dates_through_hidden_inputs() {
    let first = Date::new(2026, 3, 1).unwrap();
    let last = Date::new(2026, 3, 31).unwrap();
    let slider = RangeSlider::dates("Trip", ["from", "to"], first, last)
        .date_value(Date::new(2026, 3, 10), None)
        .drives("trips");
    let out = html(&slider, "en");
    assert!(
        out.starts_with(r#"<form class="z-range-form" data-zeyn-drives="trips" novalidate>"#),
        "{out}"
    );
    assert!(
        out.contains(
            r#"<input type="hidden" class="z-range-date" name="from" value="2026-03-10">"#
        )
    );
    assert!(
        out.contains(r#"<input type="hidden" class="z-range-date" name="to" value="2026-03-31">"#)
    );
    assert!(
        !out.contains(r#"type="range" class="z-range-input z-range-low" id="s-range--low" name="#),
        "the sliders themselves send nothing"
    );
}

#[tokio::test]
async fn a_driving_range_slider_is_a_filter_form_and_loads_its_module_only_where_used() {
    let (status, page) = get("/range").await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains(
        r#"<form class="z-range-form" data-zeyn-drives="listing" data-zeyn-kinds="{&quot;max_price&quot;:&quot;number&quot;,&quot;min_price&quot;:&quot;number&quot;}" novalidate>"#
    ), "{page}");
    let linked = links(&page);
    for name in ["range.js", "topics.js", "commerce.css"] {
        assert!(linked.contains(&name.to_string()), "{name}");
    }
    let (_, price) = get("/price").await;
    assert!(!links(&price).contains(&"range.js".to_string()));
    let (status, js) = get("/_zeyn/range.js").await;
    assert_eq!(status, StatusCode::OK);
    assert!(js.contains("@module range"));
    let app = shop_app();
    assert!(
        app.check_page(&ranged::run(Listing::default()))
            .is_ok()
    );
    let bad = Screen::new("Bad").body(RangeSlider::new("Same", ["a", "a"], 1, 1));
    let problems = app.check_page(&bad).unwrap_err();
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn a_quantity_stepper_has_named_buttons_off_at_the_ends_of_its_range() {
    let stepper = QuantityStepper::of(set_quantity, "sku-<7>")
        .range(3, 1)
        .value(1);
    let en = html(&stepper, "en");
    assert!(
        en.starts_with(r#"<div class="z-qty" role="group" aria-labelledby="s-qty--label" data-zeyn-quantity data-text-limit="Only {n} available.""#),
        "{en}"
    );
    assert!(en.contains(r#"<button type="button" class="z-qty-step" data-step="-1" aria-label="Decrease" aria-controls="s-qty-" disabled>"#), "{en}");
    assert!(
        en.contains(r#"min="1" max="3" step="1" value="1" data-testid="s.quantity.sku-&lt;7&gt;""#),
        "{en}"
    );
    assert!(en.contains(r#"aria-label="Increase" aria-controls="s-qty-"><span"#));
    assert!(en.contains(r#"<span class="z-qty-status" role="status"></span>"#));
    assert!(html(&stepper, "tr").contains(r#"aria-label="Azalt""#));
}

#[tokio::test]
async fn a_quantity_change_runs_the_action_with_the_screens_target_clamped_through_the_write_checks()
 {
    let router = shop_app().router().unwrap();
    let call = |quantity: serde_json::Value| serde_json::json!({ "screen": "cart_line", "params": { "id": 7 }, "node": "0.0", "key": "quantity", "input": quantity });
    let (status, state) = data(
        &router,
        "cart.edit",
        true,
        call(serde_json::json!({ "quantity": 5 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{state}");
    assert_eq!(state, serde_json::json!({ "quantity": 5, "max": 7 }));
    let (_, state) = data(
        &router,
        "cart.edit",
        true,
        call(serde_json::json!({ "quantity": 500, "target": "sku-1" })),
    )
    .await;
    assert_eq!(
        state["quantity"], 7,
        "the server clamps to the range (20), the action to the stock, and the target comes from the screen: {state}"
    );
    let (status, _) = data(
        &router,
        "cart.edit",
        true,
        call(serde_json::json!({ "quantity": "many" })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a quantity that is no number never reaches the action"
    );
    let (status, _) = data(
        &router,
        "",
        true,
        call(serde_json::json!({ "quantity": 2 })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the action's permission");
    let (status, _) = data(
        &router,
        "cart.edit",
        false,
        call(serde_json::json!({ "quantity": 2 })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the CSRF check");
    let mut last = StatusCode::OK;
    for _ in 0..6 {
        last = data(
            &router,
            "cart.edit",
            true,
            call(serde_json::json!({ "quantity": 2 })),
        )
        .await
        .0;
    }
    assert_eq!(
        last,
        StatusCode::TOO_MANY_REQUESTS,
        "changes count against the write limit"
    );
    let (_, page) = get("/cart/7").await;
    assert!(links(&page).contains(&"quantity.js".to_string()));
    let (_, price) = get("/price").await;
    assert!(!links(&price).contains(&"quantity.js".to_string()));
}

#[tokio::test]
async fn a_card_shows_its_price_inside_its_body_with_no_script_of_its_own() {
    let (status, page) = get("/goods").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page.contains(r#"<span class="z-poster-title">Good 1</span><span class="z-price"><span class="z-sr-only">Was ₺20.00, now ₺10.00, 50% off</span>"#),
        "{page}"
    );
    let linked = links(&page);
    assert!(linked.contains(&"commerce.css".to_string()));
    for name in ["range.js", "quantity.js"] {
        assert!(!linked.contains(&name.to_string()), "{name}");
    }
}
