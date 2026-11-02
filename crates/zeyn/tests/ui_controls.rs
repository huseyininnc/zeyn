//! Form controls and lookups: markup with native fallbacks, plain submitted values, server
//! parsing and the access checks of lookup searches.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Component, I18n, Identity, Render};

#[entity(table = "customers")]
pub struct Customer {
    #[key]
    pub id: CustomerId,
    pub name: String,
}

#[choice]
pub enum Size {
    Small,
    Large,
}

#[model]
pub struct Order {
    #[rule(required)]
    pub customer: Option<CustomerId>,
    pub tags: Vec<i64>,
    #[rule(min = 1, max = 5000)]
    pub quantity: i64,
    pub price: Money,
    pub size: Size,
    pub other: Option<Size>,
    pub urgent: bool,
    pub gift: bool,
    #[rule(max_len = 400)]
    pub note: String,
}

fn customers() -> Vec<LookupItem> {
    ["Ayşe Yılmaz", "Ilgaz Inşaat", "İnci Gıda", "Mert Kaya"]
        .iter()
        .enumerate()
        .map(|(i, n)| LookupItem::new(i as i64 + 1, *n).hint(format!("C-{i}")))
        .collect()
}

#[action(permission = "customers.read")]
pub async fn find_customers(q: LookupQuery) -> Result<Page<LookupItem>> {
    Ok(q.apply(customers()))
}

#[action(public)]
pub async fn save_order(order: Valid<Order>) -> Result<Order> {
    Ok(order.into_inner())
}

fn form(order: Order) -> zeyn::SubmitForm<Order, save_order> {
    Form::of(order)
        .lookup(Order::customer, find_customers)
        .lookup_many(Order::tags, find_customers)
        .field(Order::quantity)
        .field(Order::price)
        .field(Order::size)
        .field(Order::other)
        .control(Order::other, Control::Segmented)
        .field(Order::urgent)
        .control(Order::urgent, Control::Switch)
        .field(Order::gift)
        .field(Order::note)
        .submit("Save", save_order)
}

#[screen(route = "/orders", public)]
pub fn orders() -> Screen {
    Screen::new("Orders").body(Panel::new("Order").add(form(Order {
        customer: Some(CustomerId(2)),
        tags: vec![1, 3],
        quantity: 1250,
        ..Order::default()
    })))
}

fn html(locale: &str) -> String {
    let mut out = String::new();
    let order = Order {
        customer: Some(CustomerId(2)),
        tags: vec![1, 3],
        quantity: 1250,
        price: Money::tl(123456),
        size: Size::Large,
        ..Order::default()
    };
    form(order).render(&mut out, &Render::new("o", locale, I18n::builtin()));
    out
}

#[test]
fn every_field_kind_gets_a_control_with_a_native_element_underneath() {
    let tr = html("tr");
    assert!(
        tr.contains(r#"<div class="z-select" data-text-none="Seçilmedi""#),
        "choices render a native select inside the select control"
    );
    assert!(tr.contains(r#"<select id="o-size" name="size""#));
    assert!(tr.contains(r#"<option value="Large" selected>"#));
    assert!(tr.contains(r#"<fieldset class="z-radios z-segmented" aria-describedby="o-other-error">"#));
    assert!(
        tr.contains(r#"<input type="radio" id="o-other" name="other" value="Small""#),
        "the first radio carries the field id for error messages"
    );
    assert!(tr.contains(r#"type="checkbox" role="switch" class="z-switch""#));
    assert!(tr.contains(r#"type="checkbox" class="z-checkbox""#));
    assert!(
        tr.contains(r#"role="spinbutton" inputmode="numeric""#) && tr.contains(r#"value="1.250""#),
        "whole numbers are grouped in the user's language"
    );
    assert!(tr.contains(r#"data-min="1" aria-valuemin="1" data-max="5000" aria-valuemax="5000""#));
    assert!(tr.contains(r#"inputmode="decimal""#) && tr.contains(r#"value="1.234,56""#));
    assert!(
        tr.contains(r#"<textarea rows="3" class="z-textarea" data-zeyn-autosize"#) && tr.contains(r#"maxlength="400""#)
    );
    assert!(tr.contains(r#"aria-describedby="o-note-count o-note-error""#) && tr.contains("0 / 400 karakter"));
    assert!(
        tr.contains(r#"data-zeyn-lookup="customer" data-multiple="false""#)
            && tr.contains(r#"name="customer""#)
            && tr.contains(r#"value="2""#)
    );
    assert!(tr.contains(r#"data-zeyn-lookup="tags" data-multiple="true""#) && tr.contains(r#"value="1,3""#));
    assert!(tr.contains(r#"<label for="o-customer" id="o-customer-label">"#));
    for native in [r#"type="number""#, r#"type="date""#] {
        assert!(!tr.contains(native));
    }
}

#[test]
fn submitted_plain_values_are_read_on_the_server() {
    let f = form(Order::default());
    let values = serde_json::json!({
        "customer": "3", "tags": "1, 4", "quantity": "1.250", "price": "1.234,50", "size": "Small",
        "other": "", "urgent": true, "gift": false, "note": "ok"
    });
    let order = f.form().read(values.as_object().unwrap(), "tr", TimeZone::UTC).unwrap();
    assert_eq!(order.customer, Some(CustomerId(3)));
    assert_eq!(order.tags, vec![1, 4]);
    assert_eq!(order.quantity, 1250);
    assert_eq!(order.price, Money::tl(123450));
    assert!(order.urgent);

    let bad = serde_json::json!({ "customer": "", "tags": "1,x", "quantity": "9.999", "size": "Huge" });
    let errors = f.form().read(bad.as_object().unwrap(), "tr", TimeZone::UTC).unwrap_err();
    let fields: Vec<&str> = errors.errors.iter().map(|e| e.field).collect();
    assert_eq!(fields, ["tags", "size", "customer", "quantity"]);
}

#[test]
fn lookups_search_by_text_or_ids_and_cap_the_page() {
    let q = |text: &str| LookupQuery { text: text.into(), limit: 10, ..Default::default() };
    let labels = |p: Page<LookupItem>| p.items.into_iter().map(|i| i.label).collect::<Vec<_>>();
    assert_eq!(labels(q("ilgaz").apply(customers())), ["Ilgaz Inşaat"]);
    assert_eq!(labels(q("inci").apply(customers())), ["İnci Gıda"], "Turkish İ matches i");
    assert_eq!(labels(q("C-3").apply(customers())), ["Mert Kaya"], "hints are searched too");
    let by_ids = LookupQuery { ids: vec![4, 1], ..Default::default() }.apply(customers());
    assert_eq!(by_ids.items.len(), 2);
    let big = LookupQuery { limit: 10_000, ..Default::default() };
    assert_eq!(big.limit(), zeyn::ui::MAX_LOOKUP_PAGE);
    let paged = LookupQuery { limit: 3, ..Default::default() }.apply(customers());
    assert_eq!((paged.items.len(), paged.total, paged.next.map(|c| c.0)), (3, Some(4), Some("3".to_string())));
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn app() -> axum::Router {
    App::new("Test")
        .i18n(I18n::new("en"))
        .auth(|h: &HeaderMap| {
            let perms: Vec<String> = h.get("x-perms")?.to_str().ok()?.split(',').map(String::from).collect();
            Some(Identity::user("u").with_permissions(perms))
        })
        .feature(Feature::new("orders").screen(orders))
        .router()
        .unwrap()
}

async fn data(perms: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let req = Request::post("/_zeyn/data")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
async fn lookup_searches_go_through_the_actions_permission() {
    let call = |key: &str, input: serde_json::Value| serde_json::json!({ "screen": "orders", "node": "0.0", "key": key, "input": input });
    let (status, body) =
        data("customers.read", call("customer", serde_json::json!({ "text": "kaya", "limit": 5 }))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["items"][0]["label"], "Mert Kaya");
    assert_eq!(body["total"], 1);

    let (status, _) = data("", call("customer", serde_json::json!({ "text": "kaya" }))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "the lookup action's permission is checked");
    let (status, _) = data("customers.read", call("quantity", serde_json::json!({}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "only lookup fields are data sources");
    let (status, _) = data("customers.read", call("customer", serde_json::json!({ "text": 5 }))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "input that does not fit the action is refused");
    let (status, _) =
        data("customers.read", serde_json::json!({ "screen": "orders", "node": "0", "key": "customer", "input": {} }))
            .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "the panel itself has no data source");
}
