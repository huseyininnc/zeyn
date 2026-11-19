//! Drop down and split buttons, and toasts with tones: menu markup, commands left out for users
//! who may not run them, commands that run through the form endpoint with every check, and the
//! toast effect sent to the runtime.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{I18n, Identity, Render, ScreenDef};

#[action(permission = "orders.write")]
pub async fn save_draft(id: i64) -> Result<i64> {
    Ok(id)
}

#[action(permission = "orders.send")]
pub async fn send_order(id: i64) -> Result<i64> {
    Ok(id)
}

#[action(permission = "orders.delete")]
pub async fn delete_order(id: i64) -> Result<i64> {
    Ok(id)
}

#[screen(permission = "orders.read")]
pub fn order_card() -> Screen {
    Screen::window("Card")
}

#[screen(route = "/orders", permission = "orders.read")]
pub fn orders() -> Screen {
    orders_screen()
}

fn orders_screen() -> Screen {
    Screen::new("Orders").body(
        Stack::new()
            .add(
                MenuButton::new("More")
                    .item("Open card", Window::open(order_card, ()))
                    .separator()
                    .action_then("Send", send_order, 7, |_, ui| {
                        ui.notify(
                            Toast::success("Sent")
                                .action("Open card", Window::open(order_card, ())),
                        )
                    })
                    .separator()
                    .action("Delete", delete_order, 7)
                    .confirm("Delete the order?"),
            )
            .add(MenuButton::split("Save", save_draft, 7).action("Send", send_order, 7)),
    )
}

fn html(perms: &[&str]) -> String {
    let identity = Identity::user("u").with_permissions(
        perms
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>(),
    );
    let r = Render::new("orders", "en", I18n::builtin()).with_identity(&identity);
    orders_screen().render(
        &r,
        &<orders as ScreenDef>::META,
        &serde_json::Value::Null,
        &Default::default(),
    )
}

#[test]
fn a_menu_button_follows_the_apg_markup_and_hides_commands_the_user_may_not_run() {
    let all = html(&[
        "orders.read",
        "orders.write",
        "orders.send",
        "orders.delete",
    ]);
    assert!(
        all.contains(
            r#"aria-haspopup="menu" aria-expanded="false" aria-controls="orders-menu-0-0""#
        ),
        "{all}"
    );
    assert!(all.contains(
        r#"<ul class="z-menu-list" role="menu" id="orders-menu-0-0" aria-labelledby="orders-menu-0-0-button" hidden>"#
    ));
    assert!(
        all.contains(r#"role="menuitem" tabindex="-1" data-zeyn-effect="{&quot;effect&quot;:&quot;open_window&quot;"#)
    );
    assert!(
        all.contains(r#"<li class="z-node" data-zeyn-node="0.0.0" role="none">"#),
        "commands are nodes with a path"
    );
    assert!(all.contains(r#"data-zeyn-confirm="Delete the order?""#));
    assert_eq!(
        all.matches(r#"role="separator""#)
            .count(),
        2
    );

    let few = html(&["orders.read", "orders.write"]);
    assert!(
        !few.contains(">Send<") && !few.contains(">Delete<"),
        "{few}"
    );
    assert!(
        !few.contains(r#"role="separator""#),
        "separators next to removed commands go too"
    );
    assert!(few.contains(r#"<div class="z-split-button" role="group" aria-label="Save">"#));
    assert!(few.contains(r#"aria-label="More options for Save""#));
    assert!(few.contains(r#"class="z-button z-split-main" data-testid"#));
    let none = html(&["orders.read"]);
    assert!(
        none.contains(r#"class="z-button z-split-main" disabled"#),
        "a main action the user may not run is disabled"
    );
}

#[test]
fn toasts_carry_a_tone_and_an_action_effect() {
    let r = Render::new("orders", "tr", I18n::builtin());
    let e: Effect = Toast::danger("Kaydedilemedi")
        .action("Tekrar dene", Effect::CloseAndRefresh)
        .into();
    assert_eq!(
        e.to_json(&r),
        serde_json::json!({ "effect": "toast", "message": "Kaydedilemedi", "tone": "danger", "action": { "label": "Tekrar dene", "effect": { "effect": "close_and_refresh" } } })
    );
    let plain: Effect = Toast::info("ok").into();
    assert_eq!(plain.to_json(&r)["tone"], "info");
    assert!(
        plain
            .to_json(&r)
            .get("action")
            .is_none()
    );
    assert_eq!(zeyn::MAX_TOASTS, 3);
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

async fn run(perms: &str, node: &str) -> (StatusCode, serde_json::Value) {
    let app = App::new("Test")
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
            Feature::new("orders")
                .screen(orders)
                .screen(order_card),
        )
        .router()
        .unwrap();
    let req = Request::post("/_zeyn/form")
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-zeyn-csrf", TOKEN)
        .body(Body::from(
            serde_json::json!({ "screen": "orders", "node": node, "values": {} }).to_string(),
        ))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
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

#[tokio::test]
async fn menu_commands_run_through_the_form_endpoint_with_their_permission() {
    let (status, body) = run("orders.read,orders.send", "0.0.0").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["effects"][0]["tone"], "success");
    assert_eq!(
        body["effects"][0]["action"]["effect"]["screen"],
        "order_card"
    );
    let (status, _) = run("orders.read", "0.0.0").await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a hidden command is still refused on the server"
    );
    let (status, _) = run("orders.send", "0.0.0").await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the screen's access is checked too"
    );
    let (status, _) = run("orders.read,orders.write", "0.1.0").await;
    assert_eq!(status, StatusCode::OK, "the main action of a split button");
    let (status, _) = run("orders.read,orders.write", "0.0").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the menu button itself is no form"
    );
    let (status, _) = run("orders.read,orders.delete", "0.0.9").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
