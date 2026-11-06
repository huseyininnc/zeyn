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

#[params]
pub struct ShirtParams {
    pub color: Option<String>,
    pub size: Option<String>,
}

/// The shirt's picker: XL comes only in sand.
fn shirt_picker(p: &ShirtParams) -> VariantPicker {
    let sizes = VariantGroup::chips("size", "Size")
        .option("m", "M")
        .option("xl", "XL")
        .current(p.size.as_deref());
    let sizes = match p.color.as_deref() {
        Some("navy") => sizes.unavailable("xl", "Not in this color"),
        _ => sizes,
    };
    VariantPicker::new()
        .group(
            VariantGroup::swatches("color", "Color")
                .swatch("sand", "Sand", "#d8c3a5")
                .swatch("navy", "Navy", "--z-info")
                .swatch("bad", "<Bad>", "red;background:url(x)")
                .current(p.color.as_deref()),
        )
        .group(sizes)
        .drives("variant")
}

#[screen(route = "/shirt", public)]
pub fn shirt(p: ShirtParams) -> Screen {
    let price = if p.size.as_deref() == Some("xl") {
        34_900
    } else {
        29_900
    };
    Screen::new("Shirt").body(
        Stack::new()
            .add(shirt_picker(&p))
            .add(Price::new(Money::tl(price)).listens("variant")),
    )
}

fn gallery() -> ProductGallery {
    (1..=4).fold(ProductGallery::new("Linen shirt"), |g, i| {
        let g = g.image(
            Image::new(format!("/p/{i}.webp"))
                .alt(format!("View {i}"))
                .size(800, 800)
                .lazy(),
        );
        match i {
            3 => g.thumb("javascript:alert(1)"),
            _ => g.thumb(format!("/p/{i}-t.webp")),
        }
    })
}

#[screen(route = "/gallery", public)]
pub fn galleried() -> Screen {
    Screen::new("Gallery").body(gallery())
}

#[params]
#[derive(Default)]
pub struct GoodsParams {
    #[serde(default)]
    pub color: Vec<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
}

/// The facet panel of the goods: color counts with the price range applied.
fn good_facets(p: &GoodsParams) -> FacetPanel {
    let low = p.min_price.unwrap_or(0) * 100;
    let high = p.max_price.unwrap_or(300) * 100;
    let counted = |c: &str| {
        goods()
            .iter()
            .filter(|g| g.color == c && (low..=high).contains(&g.price.minor))
            .count() as u64
    };
    FacetPanel::of(goods_listing, p)
        .facet(
            Facet::new("color", "Color")
                .option("red", "Red", counted("red"))
                .option("blue", "<Blue>", counted("blue"))
                .option("pink", "Pink", 0)
                .chosen(p.color.clone()),
        )
        .range(
            RangeSlider::new("Price", ["min_price", "max_price"], 0, 300)
                .money(Currency::Try)
                .value(p.min_price, p.max_price),
        )
        .drives("goods")
}

#[screen(route = "/listing", public)]
pub fn goods_listing(p: GoodsParams) -> Screen {
    Screen::new("Goods")
        .body(good_facets(&p))
        .body(
            CardGrid::of(search_goods)
                .card(|g: &Good| Card::image("/g.webp", g.name.clone()))
                .scope_any(Good::color, p.color.clone())
                .scope_range(
                    Good::price,
                    p.min_price.map(|m| Money::tl(m * 100)),
                    p.max_price.map(|m| Money::tl(m * 100)),
                )
                .paging(Paging::infinite(50))
                .listens("goods"),
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
                .screen(good_cards)
                .screen(shirt)
                .screen(galleried)
                .screen(goods_listing),
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

/// Renders nodes of a screen again through `POST /_zeyn/nodes`, as a topic refresh does.
async fn refresh(
    screen: &str,
    params: serde_json::Value,
    nodes: &[&str],
) -> (StatusCode, serde_json::Value) {
    let body = serde_json::json!({ "screen": screen, "params": params, "nodes": nodes });
    let res = shop_app()
        .router()
        .unwrap()
        .oneshot(
            Request::post("/_zeyn/nodes")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
                .header("x-zeyn-csrf", TOKEN)
                .body(Body::from(body.to_string()))
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

#[test]
fn a_variant_picker_is_radio_groups_with_reasons_for_options_that_cannot_be_had() {
    let p = ShirtParams {
        color: Some("navy".into()),
        size: Some("m".into()),
    };
    let out = html(&shirt_picker(&p), "en");
    assert!(
        out.starts_with(r#"<form class="z-variants" data-zeyn-drives="variant" data-zeyn-listens="variant" novalidate><fieldset class="z-variant-group z-variant-swatches"><legend class="z-variant-legend"><span class="z-variant-name">Color</span><span class="z-variant-chosen">Navy</span></legend>"#),
        "{out}"
    );
    assert!(out.contains(r#"<input type="radio" class="z-variant-input" id="s-variants--color-1" name="color" value="navy" data-testid="s.variant.color.navy" checked><span class="z-variant-mark" style="--z-swatch:var(--z-info)" aria-hidden="true"></span><span class="z-sr-only">Navy</span>"#), "{out}");
    assert!(out.contains(r#"value="bad" data-testid="s.variant.color.bad"><span class="z-variant-mark" aria-hidden="true"></span><span class="z-sr-only">&lt;Bad&gt;</span>"#), "a color that is no color draws an empty swatch: {out}");
    assert!(!out.contains("url(x)"));
    assert!(out.contains(r#"<label class="z-variant z-variant-off" title="XL: Not in this color"><input type="radio" class="z-variant-input" id="s-variants--size-1" name="size" value="xl" data-testid="s.variant.size.xl" disabled aria-describedby="s-variants--size-1-why"><span class="z-variant-mark">XL</span><span class="z-sr-only" id="s-variants--size-1-why">Not in this color</span></label>"#), "{out}");
    let app = shop_app();
    let problems = app
        .check_page(&shirt::run(p))
        .unwrap_err();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("swatch `bad` of `color` has no color"),
        "{problems:?}"
    );
    let lonely = Screen::new("Lonely")
        .body(VariantPicker::new().group(VariantGroup::chips("size", "Size").option("m", "M")));
    assert!(app.check_page(&lonely).unwrap_err()[0].contains("refreshes nothing"));
}

#[tokio::test]
async fn a_variant_choice_refreshes_the_picker_and_its_listeners_from_the_new_params() {
    let (status, page) = get("/shirt").await;
    assert_eq!(status, StatusCode::OK);
    let linked = links(&page);
    assert!(
        linked.contains(&"topics.js".to_string()) && linked.contains(&"commerce.css".to_string())
    );
    for name in ["range.js", "quantity.js"] {
        assert!(!linked.contains(&name.to_string()), "{name}");
    }
    let (status, body) = refresh(
        "shirt",
        serde_json::json!({ "color": "navy", "size": "xl" }),
        &["0.0", "0.1"],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let picker = body["nodes"]["0.0"].as_str().unwrap();
    assert!(
        picker.contains(r#"value="xl" data-testid="s.variant.size.xl" disabled"#)
            || picker
                .contains(r#"value="xl" data-testid="shirt.variant.size.xl" checked disabled"#),
        "{picker}"
    );
    assert!(
        body["nodes"]["0.1"]
            .as_str()
            .unwrap()
            .contains("₺349.00"),
        "{body}"
    );
}

#[test]
fn a_gallery_puts_only_its_first_image_in_the_page_and_loads_it_first() {
    let out = html(&gallery(), "en");
    assert!(
        out.starts_with(r#"<section class="z-gallery" id="s-gallery-" aria-roledescription="carousel" aria-label="Linen shirt" data-zeyn-gallery"#),
        "{out}"
    );
    assert_eq!(
        out.matches(r#"<img class="z-image""#)
            .count(),
        4
    );
    assert_eq!(
        out.matches("<template>").count(),
        3,
        "three images wait in templates"
    );
    let first = out.find("<template>").unwrap();
    assert!(out[..first].contains(r#"src="/p/1.webp" alt="View 1" width="800" height="800" decoding="async" loading="eager" fetchpriority="high""#), "{out}");
    assert!(
        !out[..first].contains(r#"loading="lazy""#),
        "the first image is never lazy"
    );
    assert!(out.contains(r#"<div class="z-gallery-slide" role="group" aria-roledescription="slide" aria-label="2 of 4" data-index="1"><template>"#));
    assert!(out.contains(r#"<button type="button" class="z-gallery-thumb" data-index="0" aria-label="Show photo 1 of 4" aria-current="true"><img src="/p/1-t.webp" alt="" width="64" height="64" loading="lazy""#), "{out}");
    assert!(
        out.contains(r#"class="z-gallery-thumb z-gallery-thumb-dot" data-index="2""#)
            && !out.contains("javascript"),
        "an unsafe thumbnail becomes a dot"
    );
    assert!(out.contains(r#"<button type="button" class="z-gallery-open" aria-haspopup="dialog""#));
    let one = html(
        &ProductGallery::new("One").image(
            Image::new("/a.webp")
                .alt("A")
                .size(10, 10),
        ),
        "en",
    );
    assert!(
        !one.contains("z-gallery-thumbs"),
        "one image needs no thumbnails"
    );
}

#[tokio::test]
async fn the_gallery_module_loads_only_with_a_gallery() {
    let (status, page) = get("/gallery").await;
    assert_eq!(status, StatusCode::OK);
    let linked = links(&page);
    assert!(
        linked.contains(&"gallery.js".to_string()) && linked.contains(&"commerce.css".to_string())
    );
    assert!(
        !linked.contains(&"media.js".to_string()),
        "plain images need no media script"
    );
    let (_, other) = get("/shirt").await;
    assert!(!links(&other).contains(&"gallery.js".to_string()));
}

#[test]
fn a_facet_panel_counts_options_and_lists_the_chosen_ones_as_links_without_them() {
    let p = GoodsParams {
        color: vec!["red".into(), "blue".into()],
        min_price: Some(50),
        max_price: None,
    };
    let out = html(&good_facets(&p), "en");
    assert!(out.starts_with(r#"<form class="z-facets" aria-label="Filters" data-zeyn-drives="goods" data-zeyn-listens="goods" data-zeyn-kinds="{&quot;color&quot;:&quot;list&quot;,&quot;max_price&quot;:&quot;number&quot;,&quot;min_price&quot;:&quot;number&quot;}" novalidate>"#), "{out}");
    assert!(out.contains(r#"name="color" value="red" aria-label="Red, 9 results" data-testid="s.facet.color.red" checked><span class="z-facet-text" aria-hidden="true">Red</span><span class="z-facet-count" aria-hidden="true">9</span>"#), "{out}");
    assert!(out.contains(r#"value="pink" aria-label="Pink, 0 results" data-testid="s.facet.color.pink" disabled>"#), "an empty option is off");
    assert!(out.contains("&lt;Blue&gt;") && !out.contains("<Blue>"));
    assert!(out.contains(r#"<a class="z-facet-chip" href="/_zeyn/route/goods_listing?params=%7B%22color%22%3A%5B%22blue%22%5D%2C%22max_price%22%3Anull%2C%22min_price%22%3A50%7D" data-zeyn-params="{&quot;color&quot;:[&quot;blue&quot;],&quot;max_price&quot;:null,&quot;min_price&quot;:50}" data-zeyn-base-href aria-label="Remove filter Red"><span>Red</span>"#), "{out}");
    assert!(
        out.contains(r#"aria-label="Remove filter Price"><span>Price: ₺50.00 to ₺300.00</span>"#),
        "{out}"
    );
    assert!(out.contains(r#"<a class="z-facets-clear" href="/_zeyn/route/goods_listing?params=%7B%22color%22%3A%5B%5D%2C%22max_price%22%3Anull%2C%22min_price%22%3Anull%7D""#), "{out}");
    let none = html(&good_facets(&GoodsParams::default()), "en");
    assert!(
        !none.contains("z-facets-chosen"),
        "nothing chosen, no chips"
    );
    let app = shop_app();
    assert!(
        app.check_page(&goods_listing::run(GoodsParams::default()))
            .is_ok()
    );
    let stray = Screen::new("Stray").body(
        FacetPanel::of(goods_listing, &GoodsParams::default())
            .facet(Facet::new("size", "Size").option("m", "M", 1))
            .drives("goods"),
    );
    assert!(
        app.check_page(&stray).unwrap_err()[0]
            .contains("facet `size` is not a field of the screen's params")
    );
}

#[tokio::test]
async fn a_facet_change_refreshes_the_cards_with_the_server_side_scope_in_one_request() {
    let (status, body) = refresh(
        "goods_listing",
        serde_json::json!({ "color": ["red"], "min_price": 100, "max_price": 200 }),
        &["0", "1"],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let cards = body["nodes"]["1"].as_str().unwrap();
    let names: Vec<&str> = cards
        .split(r#"<span class="z-poster-title">"#)
        .skip(1)
        .filter_map(|s| s.split('<').next())
        .collect();
    assert_eq!(
        names,
        ["Good 12", "Good 15", "Good 18"],
        "red goods from ₺100 to ₺200"
    );
    let panel = body["nodes"]["0"].as_str().unwrap();
    assert!(
        panel.contains(r#"aria-label="Red, 3 results""#),
        "the counts follow: {panel}"
    );
    let (_, page) = get("/listing").await;
    let linked = links(&page);
    for name in ["topics.js", "range.js", "commerce.css"] {
        assert!(linked.contains(&name.to_string()), "{name}");
    }
}

#[test]
fn a_review_summary_computes_its_average_and_shares_from_the_counts() {
    let summary = zeyn::ui::ReviewSummary::new()
        .count(5, 1_200)
        .count(4, 300)
        .count(2, 0)
        .count(9, 50);
    let en = html(&summary, "en");
    assert!(en.contains(r#"<span class="z-reviews-average" aria-hidden="true">4.8</span><span class="z-stars" role="img" aria-label="4.8 out of 5 stars" style="--z-stars:96%"></span><span class="z-reviews-count">1,500 reviews</span>"#), "{en}");
    assert!(en.contains(r#"<li class="z-reviews-row"><span class="z-reviews-star">5 stars</span><span class="z-reviews-bar" aria-hidden="true"><span class="z-reviews-fill" style="--z-share:80%"></span></span><span class="z-reviews-share">80%</span></li>"#), "{en}");
    assert!(
        en.find("5 stars").unwrap() < en.find("1 stars").unwrap(),
        "the bars run from 5 to 1"
    );
    let tr = html(&summary, "tr");
    assert!(
        tr.contains(">4,8<") && tr.contains("1.500 değerlendirme") && tr.contains(">%80<"),
        "{tr}"
    );
    let none = html(&zeyn::ui::ReviewSummary::new(), "en");
    assert!(
        none.contains(r#"aria-label="0.0 out of 5 stars" style="--z-stars:0%""#)
            && none.contains("0 reviews")
    );
    assert!(
        !en.contains("<script") && !en.contains("data-zeyn-"),
        "no script"
    );
}

#[model]
pub struct Score {
    #[rule(required, min = 1, max = 5, scale = 1)]
    pub stars: Option<Decimal>,
}

#[action(public)]
pub async fn save_score(score: Valid<Score>) -> Result<()> {
    let _ = score;
    Ok(())
}

#[test]
fn a_decimal_rating_takes_half_stars_and_the_server_turns_away_values_between_them() {
    let form = Form::of(Score {
        stars: Decimal::parse("3.5"),
    })
    .field(Score::stars)
    .control(Score::stars, Control::Rating)
    .submit("Save", save_score);
    let tr = html(&form, "tr");
    assert_eq!(tr.matches(r#"type="radio""#).count(), 10, "{tr}");
    assert!(tr.contains(r#"<fieldset class="z-rating z-rating-halves""#));
    assert!(tr.contains(r#"<label class="z-rating-star z-rating-half" for="s-stars-3_5"><input type="radio" class="z-rating-input" id="s-stars-3_5" name="stars" value="3,5" data-testid="s.field.stars.3_5" checked required>"#), "{tr}");
    assert!(
        tr.contains("5 üzerinden 3,5") || tr.contains("3,5 / 5") || tr.contains(">3,5"),
        "{tr}"
    );
    let en = html(&form, "en");
    assert!(en.contains(r#"class="z-rating-star z-rating-half z-rating-half-end" for="s-stars-4"><input type="radio" class="z-rating-input" id="s-stars-4" name="stars" value="4""#), "{en}");
    let read = |v: serde_json::Value| {
        form.form()
            .read(v.as_object().unwrap(), "en", TimeZone::UTC)
    };
    assert_eq!(
        read(serde_json::json!({ "stars": "4.5" }))
            .unwrap()
            .stars,
        Decimal::parse("4.5")
    );
    let errors = read(serde_json::json!({ "stars": "4.3" })).unwrap_err();
    assert!(
        errors
            .errors
            .iter()
            .any(|e| e.field == "stars"),
        "a value between the halves is turned away"
    );
}

#[test]
fn a_countdown_writes_the_time_left_by_the_server_clock_and_the_end_state_once_over() {
    let left = ((2 * 24 + 3) * 3_600 + 14 * 60 + 9) * 1_000 + 600;
    let end = DateTime::now().add_ms(left);
    let timer = zeyn::ui::Countdown::until(end)
        .label("Sale ends in")
        .ended(Badge::new("Ended"));
    let out = html(&timer, "en");
    assert!(
        out.starts_with(&format!(
            r#"<div class="z-countdown" data-zeyn-countdown data-end="{}" data-now=""#,
            end.unix_ms()
        )),
        "{out}"
    );
    assert!(out.contains(r#"<span class="z-countdown-label">Sale ends in</span><span class="z-countdown-time" aria-hidden="true"><span class="z-countdown-d">2</span>d <span class="z-countdown-h">03</span>:<span class="z-countdown-m">14</span>:"#), "{out}");
    assert!(out.contains(r#"<span class="z-sr-only z-countdown-words" aria-live="polite" aria-atomic="true">2 d 3 h 14 min left</span>"#), "{out}");
    assert!(
        out.contains(r#"data-text-left="{d} d {h} h {m} min left""#),
        "{out}"
    );
    assert!(
        out.contains(r#"<template class="z-countdown-end"><span class="z-badge"#),
        "{out}"
    );
    let over = html(
        &zeyn::ui::Countdown::until(DateTime::now().add_ms(-1_000)).ended(Badge::new("Ended")),
        "tr",
    );
    assert!(
        over.starts_with(r#"<div class="z-countdown z-countdown-over"><span class="z-badge"#),
        "{over}"
    );
    assert!(
        !over.contains("data-zeyn-countdown"),
        "a finished countdown needs no script"
    );
    assert!(html(&timer, "tr").contains("2 g 3 sa 14 dk kaldı"));
}

#[screen(route = "/sale", public)]
pub fn sale() -> Screen {
    Screen::new("Sale").body(zeyn::ui::Countdown::until(DateTime::now().add_ms(60_000)))
}

#[tokio::test]
async fn the_countdown_module_loads_only_with_a_running_countdown() {
    let res = App::new("Test")
        .i18n(I18n::new("en"))
        .feature(
            Feature::new("shop")
                .screen(sale)
                .screen(plain),
        )
        .router()
        .unwrap();
    for (path, wanted) in [("/sale", true), ("/plain", false)] {
        let page = res
            .clone()
            .oneshot(
                Request::get(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let page = String::from_utf8_lossy(&page).into_owned();
        assert_eq!(
            links(&page).contains(&"countdown.js".to_string()),
            wanted,
            "{path}"
        );
    }
}
