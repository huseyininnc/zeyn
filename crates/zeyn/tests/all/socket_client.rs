//! The Rust client of the page socket and screens that react to socket events.

use axum::body::Body;
use axum::http::{HeaderMap, Request};
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::realtime::{Conn, Event, Socket, TestSocket, ZeynSocket};
use zeyn::ui::{OnEvent, OnEvents};

#[derive(Serialize, Deserialize, Event, Debug, PartialEq)]
#[event(name = "chat.message")]
pub struct MessageSent {
    pub room: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Event)]
#[event(name = "orders.shipped")]
pub struct Shipped {
    pub id: u32,
}

#[params]
pub struct Say {
    pub room: String,
    pub text: String,
}

#[action(permission = "chat.read")]
pub async fn join(s: Say, conn: Conn) -> Result<()> {
    conn.join(format!("room:{}", s.room))
}

#[action(permission = "chat.read")]
pub async fn say(s: Say, socket: Socket, ctx: Ctx) -> Result<()> {
    socket
        .to_group(&ctx, format!("room:{}", s.room))
        .emit(&MessageSent {
            room: s.room,
            text: s.text,
        })?;
    Ok(())
}

#[screen(route = "/", permission = "chat.read")]
pub fn home() -> Screen {
    Screen::new("Chat").body(
        Stack::new()
            .add(Alert::info("Messages").on_event::<MessageSent>(OnEvent::Refresh))
            .add(
                Alert::info("Orders")
                    .on_event::<Shipped>(OnEvent::Toast(Text::from("Order <shipped>"))),
            ),
    )
}

fn app() -> (App, Socket) {
    let live = ZeynSocket::new()
        .topic("orders", "chat.read")
        .event::<MessageSent>("chat.read")
        .event::<Shipped>("chat.read");
    let socket = live.socket();
    let app = App::new("Chat")
        .auth(|h: &HeaderMap| {
            Some(Identity::user(h.get("x-user")?.to_str().ok()?).with_permissions(["chat.read"]))
        })
        .feature(
            Feature::new("chat")
                .screen(home)
                .api(join)
                .api(say),
        )
        .socket(live);
    (app, socket)
}

#[tokio::test]
async fn a_rust_client_calls_actions_and_reads_typed_events() {
    let (app, socket) = app();
    let addr = TestSocket::serve(app.router().unwrap()).await;
    let mut ada = TestSocket::connect(addr, &[("x-user", "ada")])
        .await
        .unwrap();
    let mut grace = TestSocket::connect(addr, &[("x-user", "grace")])
        .await
        .unwrap();
    let room = serde_json::json!({ "room": "1", "text": "" });
    ada.call("join", &room).await.unwrap();
    grace.call("join", &room).await.unwrap();
    grace
        .call("say", serde_json::json!({ "room": "1", "text": "hi" }))
        .await
        .unwrap();
    let got: MessageSent = ada.next_event().await.unwrap();
    assert_eq!(
        got,
        MessageSent {
            room: "1".into(),
            text: "hi".into()
        }
    );
    assert!(ada.call("nope", ()).await.is_err());
    ada.subscribe(&["orders"])
        .await
        .unwrap();
    let mut waited = 0;
    while socket.stats().subscribers == 0 && waited < 100 {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        waited += 1;
    }
    socket.notify_tenant(None, "orders");
    assert_eq!(ada.next_topic().await.unwrap(), "orders");
}

#[tokio::test]
async fn nodes_that_react_to_events_link_the_socket_script() {
    let router = app().0.router().unwrap();
    let res = router
        .oneshot(
            Request::get("/")
                .header("x-user", "ada")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = String::from_utf8(
        res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains(r#"data-zeyn-on="chat.message""#), "{html}");
    assert!(html.contains(r#"data-zeyn-listens="chat.message""#));
    assert!(html.contains(r#"data-zeyn-on-toast="Order &lt;shipped&gt;""#));
    assert!(
        html.contains("/_zeyn/live.js"),
        "the page links the socket script"
    );
}
