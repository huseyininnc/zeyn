//! Empty states of data components: the app's own title, text and action when there are no rows,
//! and another one when a filter or a search leaves none.

use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;

#[entity(table = "orders")]
pub struct Order {
    #[key]
    pub id: OrderId,
    pub name: String,
}

#[action(public)]
pub async fn no_orders(_q: DataQuery) -> Result<Page<Order>> {
    Ok(Page::new(Vec::new()))
}

#[screen(route = "/new-order", public)]
pub fn new_order() -> Screen {
    Screen::new("New order")
}

fn none() -> EmptyState {
    EmptyState::new("No orders yet")
        .text("Orders you take show up here.")
        .link("New order", new_order, ())
}

fn no_match() -> EmptyState {
    EmptyState::new("No orders match").text("Change the filters.")
}

#[screen(route = "/grid", public)]
pub fn grid() -> Screen {
    Screen::new("Grid").body(
        DataGrid::of(no_orders)
            .column(Order::name)
            .filter()
            .empty(none())
            .empty_filtered(no_match()),
    )
}

#[screen(route = "/cards", public)]
pub fn cards() -> Screen {
    Screen::new("Cards").body(
        CardGrid::of(no_orders)
            .card(|o: &Order| Card::image("/x.png", o.name.clone()))
            .empty(none())
            .empty_filtered(no_match()),
    )
}

#[screen(route = "/found", public)]
pub fn found() -> Screen {
    Screen::new("Found").body(
        List::of(no_orders)
            .search("zzz")
            .empty(none())
            .empty_filtered(no_match()),
    )
}

#[screen(route = "/plain", public)]
pub fn plain() -> Screen {
    Screen::new("Plain")
        .body(CardGrid::of(no_orders).card(|o: &Order| Card::image("/x.png", o.name.clone())))
}

async fn page(path: &str) -> String {
    let router = App::new("Empty")
        .feature(
            Feature::new("o")
                .screen(grid)
                .screen(cards)
                .screen(found)
                .screen(plain)
                .screen(new_order),
        )
        .router()
        .unwrap();
    let res = router
        .oneshot(
            Request::get(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    String::from_utf8(
        res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap()
}

#[tokio::test]
async fn a_grid_carries_both_states_for_the_browser_to_pick() {
    let html = page("/grid").await;
    assert!(
        html.contains(r#"<template data-zeyn-empty><div class="z-empty">"#),
        "{html}"
    );
    assert!(html.contains("No orders yet"));
    assert!(html.contains(r#"<template data-zeyn-empty-filtered><div class="z-empty">"#));
    assert!(html.contains("No orders match"));
}

#[tokio::test]
async fn card_grids_and_lists_show_the_state_for_their_search() {
    let html = page("/cards").await;
    assert!(
        html.contains(r#"<div class="z-cards-empty" data-zeyn-cards-empty><div class="z-empty">"#),
        "{html}"
    );
    assert!(html.contains("No orders yet") && !html.contains("No orders match"));
    let html = page("/found").await;
    assert!(
        html.contains("No orders match") && !html.contains("No orders yet"),
        "a search that finds nothing shows the filtered state"
    );
    let html = page("/plain").await;
    assert!(html.contains(r#"<div class="z-cards-empty" data-zeyn-cards-empty>"#));
    assert!(
        !html.contains("z-empty\""),
        "without a state of its own the built in text shows"
    );
}
