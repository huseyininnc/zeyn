//! Master detail lines: rendering, rebuilding lines from submitted cells, per line validation
//! through `Valid<M>`, and protection of fields the client was not shown.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Component, I18n, Identity, Render, Rule};

#[entity(table = "order_lines")]
pub struct OrderLine {
    #[key]
    pub id: OrderLineId,
    #[rule(required, max_len = 40)]
    pub item: String,
    #[rule(min = 1, max = 99)]
    pub quantity: i64,
    #[rule(min = 0)]
    pub price: Money,
    #[readonly]
    pub note: String,
}

#[model]
pub struct Order {
    #[rule(required)]
    pub customer: String,
    #[rule(required)]
    pub lines: Vec<OrderLine>,
}

fn line(id: i64, item: &str, quantity: i64, kurus: i64) -> OrderLine {
    OrderLine {
        id: OrderLineId(id),
        item: item.into(),
        quantity,
        price: Money::tl(kurus),
        note: format!("note {id}"),
    }
}

#[allow(
    clippy::inconsistent_digit_grouping,
    reason = "money in minor units is written with the cents after the underscore"
)]
fn order() -> Order {
    Order {
        customer: "Acme".into(),
        lines: vec![line(10, "Tea", 2, 150_00), line(11, "Honey", 1, 99_95)],
    }
}

#[action(public)]
pub async fn save_order(order: Valid<Order>) -> Result<i64> {
    Ok(order.lines.len() as i64)
}

fn form(o: Order) -> zeyn::SubmitForm<Order, save_order> {
    Form::of(o)
        .field(Order::customer)
        .lines(Order::lines, |l| {
            l.column(OrderLine::item)
                .column(OrderLine::quantity)
                .column(OrderLine::price)
                .total(OrderLine::quantity, OrderLine::price)
        })
        .submit("Save", save_order)
}

fn values(pairs: &[(&str, &str)]) -> serde_json::Map<String, serde_json::Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), serde_json::json!(v)))
        .collect()
}

fn read(pairs: &[(&str, &str)]) -> Result<Order, zeyn::ValidationErrors> {
    form(order())
        .form()
        .read(&values(pairs), "tr", TimeZone::UTC)
        .map(|v| v.into_inner())
}

#[test]
fn lines_render_as_a_table_with_named_cells_and_server_totals() {
    let mut out = String::new();
    form(order()).render(&mut out, &Render::new("o", "tr", I18n::builtin()));
    assert!(out.contains(r#"<table class="z-lines-table" aria-labelledby="o-lines-label">"#));
    assert!(out.contains(r#"name="lines.0.item""#) && out.contains(r#"name="lines.1.price""#));
    assert!(out.contains(r#"name="lines.1._i" value="1""#));
    assert!(
        out.contains(r#"aria-label="Quantity, Satır 1""#)
            || out.contains(r#"aria-label="Miktar, Satır 1""#)
            || out.contains("Quantity, Satır 1")
    );
    assert!(out.contains(r#"value="150,00""#));
    assert!(
        out.contains(r#"data-zeyn-line-total>300,00</td>"#),
        "line total is quantity times price"
    );
    assert!(
        out.contains(r#"data-zeyn-lines-sum aria-live="polite">399,95</td>"#),
        "footer total of every line"
    );
    assert!(
        out.contains(r#"<template data-zeyn-line-template>"#)
            && out.contains(r#"aria-label="1. satırı sil""#)
    );
    assert!(
        !out.contains("note 10"),
        "columns not on the table are not sent to the browser"
    );
}

#[test]
fn lines_are_rebuilt_from_cells_keeping_hidden_fields_of_the_line_they_came_from() {
    let o = read(&[
        ("customer", "Acme"),
        ("lines", "rows"),
        ("lines.0._i", "1"),
        ("lines.0.item", "Honey"),
        ("lines.0.quantity", "3"),
        ("lines.0.price", "1.099,95"),
        ("lines.1._i", ""),
        ("lines.1.item", "Coffee"),
        ("lines.1.quantity", "1"),
        ("lines.1.price", "10"),
    ])
    .unwrap();
    assert_eq!(
        o.lines.len(),
        2,
        "the first loaded line was removed and a new one added"
    );
    assert_eq!(o.lines[0].id, OrderLineId(11), "a moved line keeps its key");
    assert_eq!(
        o.lines[0].note, "note 11",
        "read only fields come from the loaded line"
    );
    assert_eq!(o.lines[0].price, Money::tl(109_995));
    assert_eq!(o.lines[1].id, OrderLineId(0), "new lines start empty");
    assert_eq!(o.lines[1].note, "");
}

#[test]
fn clients_cannot_forge_keys_or_reuse_a_line_twice() {
    let o = read(&[
        ("customer", "Acme"),
        ("lines", "rows"),
        ("lines.0._i", "0"),
        ("lines.0.item", "Tea"),
        ("lines.0.quantity", "1"),
        ("lines.0.price", "1"),
        ("lines.0.id", "999"),
        ("lines.0.note", "forged"),
        ("lines.1._i", "0"),
        ("lines.1.item", "Copy"),
        ("lines.1.quantity", "1"),
        ("lines.1.price", "1"),
        ("lines.2._i", "77"),
        ("lines.2.item", "Out of range"),
        ("lines.2.quantity", "1"),
        ("lines.2.price", "1"),
    ])
    .unwrap();
    assert_eq!(o.lines[0].id, OrderLineId(10));
    assert_eq!(o.lines[0].note, "note 10");
    assert_eq!(
        o.lines[1].id,
        OrderLineId(0),
        "a second use of the same loaded line becomes a new line"
    );
    assert_eq!(o.lines[2].id, OrderLineId(0));
}

#[test]
fn every_line_is_validated_and_errors_name_the_line_and_column() {
    let errors = read(&[
        ("customer", "Acme"),
        ("lines", "rows"),
        ("lines.0._i", "0"),
        ("lines.0.item", ""),
        ("lines.0.quantity", "5"),
        ("lines.0.price", "1"),
        ("lines.1._i", "1"),
        ("lines.1.item", "Honey"),
        ("lines.1.quantity", "x"),
        ("lines.1.price", "1"),
    ])
    .unwrap_err();
    let found: Vec<(Rule, String)> = errors
        .errors
        .iter()
        .map(|e| (e.rule, e.message.english()))
        .collect();
    assert!(
        found.contains(&(
            Rule::Line {
                row: 1,
                field: "quantity"
            },
            "Line 2: Quantity has an invalid value.".to_string()
        )),
        "{found:?}"
    );
    assert!(
        found.contains(&(
            Rule::Line {
                row: 0,
                field: "item"
            },
            "Line 1: Item is required.".to_string()
        )),
        "{found:?}"
    );
    assert_eq!(
        found
            .iter()
            .filter(|(r, _)| *r
                == Rule::Line {
                    row: 1,
                    field: "quantity"
                })
            .count(),
        1,
        "an unreadable cell is reported once"
    );
    assert!(
        errors
            .errors
            .iter()
            .all(|e| e.field == "lines")
    );
}

#[test]
fn valid_checks_lines_even_without_a_form() {
    let mut bad = order();
    bad.lines[1].quantity = 500;
    let errors = Valid::new(bad).unwrap_err();
    assert_eq!(
        errors.errors[0].rule,
        Rule::Line {
            row: 1,
            field: "quantity"
        }
    );
    assert!(Valid::new(order()).is_ok());
}

#[test]
fn removing_every_line_empties_the_list_and_forms_without_cells_keep_the_loaded_lines() {
    let errors = read(&[("customer", "Acme"), ("lines", "rows")]).unwrap_err();
    assert_eq!(
        errors.errors[0].rule,
        Rule::Required,
        "the list is empty and required"
    );
    let kept = read(&[("customer", "Acme")]).unwrap();
    assert_eq!(kept.lines.len(), 2);
    let mut many = vec![
        ("customer".to_string(), "Acme".to_string()),
        ("lines".into(), "rows".into()),
    ];
    for i in 0..600 {
        many.push((format!("lines.{i}.item"), "x".into()));
    }
    let map: serde_json::Map<String, serde_json::Value> = many
        .into_iter()
        .map(|(k, v)| (k, serde_json::json!(v)))
        .collect();
    let errors = form(order())
        .form()
        .read(&map, "tr", TimeZone::UTC)
        .unwrap_err();
    assert!(
        errors
            .errors
            .iter()
            .any(|e| e.rule == Rule::MaxLen(zeyn::ui::MAX_LINES))
    );
}

#[screen(route = "/order", public)]
pub fn order_screen() -> Screen {
    Screen::new("Order").body(Panel::new("Order").add(form(order())))
}

#[tokio::test]
async fn the_form_endpoint_reports_line_errors_with_row_and_column() {
    const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let router = App::new("Test")
        .i18n(I18n::new("en"))
        .auth(|_: &axum::http::HeaderMap| Some(Identity::user("u")))
        .feature(Feature::new("o").screen(order_screen))
        .router()
        .unwrap();
    let body = serde_json::json!({
        "screen": "order_screen", "node": "0.0",
        "values": { "customer": "Acme", "lines": "rows", "lines.0._i": "0", "lines.0.item": "Tea", "lines.0.quantity": "0", "lines.0.price": "1" }
    });
    let req = Request::post("/_zeyn/form")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let json: serde_json::Value = serde_json::from_slice(
        &res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(json["errors"][0]["field"], "lines");
    assert_eq!(
        json["errors"][0]["rule"],
        serde_json::json!({ "rule": "line", "value": { "row": 0, "field": "quantity" } })
    );
}
