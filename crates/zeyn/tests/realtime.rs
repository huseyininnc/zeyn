use axum::http::{HeaderMap, HeaderValue};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::time::Duration;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, Text, Transact, Update};
use zeyn::prelude::*;
use zeyn::realtime::{FRAME_UPDATE, Hub, Limits, MemoryDocStore, Realtime, RoomKey};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[screen(route = "/", permission = "notes.read")]
pub fn home() -> Screen {
    Screen::new("Home")
}

/// Refuses live channels to the user `blocked`, as a suspended account check would.
struct Suspended;

impl zeyn::Interceptor for Suspended {
    fn before(&self, ctx: &Ctx, call: &zeyn::Call) -> Result<()> {
        if call.kind == zeyn::CallKind::Live && ctx.identity().user.as_deref() == Some("blocked") {
            return Err(Error::Forbidden);
        }
        Ok(())
    }
}

async fn start(limits: Limits) -> (SocketAddr, Hub, MemoryDocStore) {
    let store = MemoryDocStore::default();
    let realtime = Realtime::new()
        .documents("notes", "notes.read", "notes.write", store.clone())
        .topic("orders", "notes.read")
        .limits(limits);
    let hub = realtime.hub();
    let app = App::new("Test")
        .auth(|h: &HeaderMap| {
            let user = h
                .get("x-user")?
                .to_str()
                .ok()?
                .to_string();
            let perms: &[&str] = if h.contains_key("x-write") {
                &["notes.read", "notes.write"]
            } else {
                &["notes.read"]
            };
            Some(Identity::user(user).with_permissions(perms.iter().copied()))
        })
        .feature(Feature::new("notes").screen(home))
        .interceptor(Suspended)
        .realtime(realtime);
    let router = app.router().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap()
    });
    (addr, hub, store)
}

async fn open(
    addr: SocketAddr,
    path: &str,
    user: &str,
    write: bool,
    origin: &str,
) -> Result<Socket, WsError> {
    let mut req = format!("ws://{addr}{path}")
        .into_client_request()
        .unwrap();
    let h = req.headers_mut();
    h.insert("origin", HeaderValue::from_str(origin).unwrap());
    h.insert(
        "cookie",
        HeaderValue::from_str(&format!("zeyn_csrf={TOKEN}")).unwrap(),
    );
    h.insert("x-user", HeaderValue::from_str(user).unwrap());
    if write {
        h.insert("x-write", HeaderValue::from_static("1"));
    }
    tokio_tungstenite::connect_async(req)
        .await
        .map(|(s, _)| s)
}

fn doc_path(doc: &str) -> String {
    format!("/_zeyn/live/doc/notes/{doc}?csrf={TOKEN}")
}

async fn next_binary(s: &mut Socket) -> Vec<u8> {
    loop {
        match tokio::time::timeout(Duration::from_secs(5), s.next())
            .await
            .expect("a message in time")
        {
            Some(Ok(Message::Binary(b))) => return b.to_vec(),
            Some(Ok(_)) => continue,
            other => panic!("socket ended: {other:?}"),
        }
    }
}

fn status(e: WsError) -> u16 {
    match e {
        WsError::Http(r) => r.status().as_u16(),
        other => panic!("expected an HTTP refusal, got {other:?}"),
    }
}

/// An update that appends `text` to the document's `body`, as `doc` sees it.
fn edit(doc: &Doc, text: &str) -> Vec<u8> {
    let body = doc.get_or_insert_text("body");
    let before = doc.transact().state_vector();
    {
        let mut txn = doc.transact_mut();
        let len = body.get_string(&txn).len() as u32;
        body.insert(&mut txn, len, text);
    }
    let update = doc
        .transact()
        .encode_state_as_update_v1(&before);
    [vec![FRAME_UPDATE], update].concat()
}

fn read(doc: &Doc, frame: &[u8]) -> String {
    assert_eq!(frame[0], FRAME_UPDATE);
    doc.transact_mut()
        .apply_update(Update::decode_v1(&frame[1..]).unwrap())
        .unwrap();
    let body = doc.get_or_insert_text("body");
    body.get_string(&doc.transact())
}

#[tokio::test]
async fn handshakes_need_origin_token_user_and_a_declared_document() {
    let (addr, _, _) = start(Limits::default()).await;
    let here = format!("http://{addr}");
    assert_eq!(
        status(
            open(addr, &doc_path("a"), "ayse", true, "http://evil.test")
                .await
                .unwrap_err()
        ),
        403
    );
    assert_eq!(
        status(
            open(
                addr,
                "/_zeyn/live/doc/notes/a?csrf=wrong",
                "ayse",
                true,
                &here
            )
            .await
            .unwrap_err()
        ),
        403
    );
    assert_eq!(
        status(
            open(
                addr,
                &format!("/_zeyn/live/doc/other/a?csrf={TOKEN}"),
                "ayse",
                true,
                &here
            )
            .await
            .unwrap_err()
        ),
        404
    );
    assert_eq!(
        status(
            open(addr, &doc_path("a/b"), "ayse", true, &here)
                .await
                .unwrap_err()
        ),
        404
    );
    assert_eq!(
        status(
            open(
                addr,
                &format!("/_zeyn/live/topics?t=secret&csrf={TOKEN}"),
                "ayse",
                true,
                &here
            )
            .await
            .unwrap_err()
        ),
        403
    );
    assert!(
        open(addr, &doc_path("a"), "ayse", true, &here)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn live_channels_run_the_interceptors() {
    let (addr, _, _) = start(Limits::default()).await;
    let here = format!("http://{addr}");
    assert_eq!(
        status(
            open(addr, &doc_path("a"), "blocked", true, &here)
                .await
                .unwrap_err()
        ),
        403
    );
    let topics = format!("/_zeyn/live/topics?t=orders&csrf={TOKEN}");
    assert_eq!(
        status(
            open(addr, &topics, "blocked", false, &here)
                .await
                .unwrap_err()
        ),
        403
    );
    assert!(
        open(addr, &topics, "ayse", false, &here)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn two_clients_edit_one_document_and_readers_cannot_write() {
    let (addr, hub, _) = start(Limits::default()).await;
    let here = format!("http://{addr}");
    let mut a = open(addr, &doc_path("shared"), "ayse", true, &here)
        .await
        .unwrap();
    let mut b = open(addr, &doc_path("shared"), "bora", true, &here)
        .await
        .unwrap();
    let (da, db) = (Doc::new(), Doc::new());
    read(&da, &next_binary(&mut a).await);
    read(&db, &next_binary(&mut b).await);

    a.send(Message::Binary(edit(&da, "hello").into()))
        .await
        .unwrap();
    assert_eq!(read(&db, &next_binary(&mut b).await), "hello");
    b.send(Message::Binary(edit(&db, " world").into()))
        .await
        .unwrap();
    assert_eq!(read(&da, &next_binary(&mut a).await), "hello world");
    assert_eq!(hub.stats().clients, 2);

    let mut reader = open(addr, &doc_path("shared"), "cem", false, &here)
        .await
        .unwrap();
    let late = Doc::new();
    assert_eq!(read(&late, &next_binary(&mut reader).await), "hello world");
    reader
        .send(Message::Binary(edit(&late, "!").into()))
        .await
        .unwrap();
    let closed = loop {
        match tokio::time::timeout(Duration::from_secs(5), reader.next())
            .await
            .unwrap()
        {
            Some(Ok(Message::Close(frame))) => break frame.map(|f| f.code),
            Some(Ok(_)) => continue,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(closed, Some(CloseCode::Policy));
}

#[tokio::test]
async fn idle_documents_are_persisted_and_evicted_and_come_back() {
    let limits = Limits {
        idle: Duration::from_millis(100),
        persist_every: Duration::from_millis(50),
        ..Limits::default()
    };
    let (addr, hub, store) = start(limits).await;
    let here = format!("http://{addr}");
    let mut a = open(addr, &doc_path("memo"), "ayse", true, &here)
        .await
        .unwrap();
    let doc = Doc::new();
    read(&doc, &next_binary(&mut a).await);
    a.send(Message::Binary(edit(&doc, "kept").into()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    a.close(None).await.unwrap();
    drop(a);
    let key = RoomKey {
        tenant: String::new(),
        kind: "notes".into(),
        doc: "memo".into(),
    };
    let mut waited = 0;
    while hub.stats().rooms > 0 && waited < 50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        waited += 1;
    }
    assert_eq!(
        hub.stats(),
        Default::default(),
        "room evicted, nothing held"
    );
    let saved = Doc::new();
    saved
        .transact_mut()
        .apply_update(Update::decode_v1(&store.get(&key).unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        saved
            .get_or_insert_text("body")
            .get_string(&saved.transact()),
        "kept"
    );

    let mut again = open(addr, &doc_path("memo"), "ayse", true, &here)
        .await
        .unwrap();
    assert_eq!(read(&Doc::new(), &next_binary(&mut again).await), "kept");
}

#[tokio::test]
async fn limits_refuse_extra_connections_and_large_documents() {
    let limits = Limits {
        per_user: 1,
        per_room: 1,
        doc_bytes: 64,
        ..Limits::default()
    };
    let (addr, _, _) = start(limits).await;
    let here = format!("http://{addr}");
    let mut a = open(addr, &doc_path("x"), "ayse", true, &here)
        .await
        .unwrap();
    assert_eq!(
        status(
            open(addr, &doc_path("y"), "ayse", true, &here)
                .await
                .unwrap_err()
        ),
        429
    );
    assert_eq!(
        status(
            open(addr, &doc_path("x"), "bora", true, &here)
                .await
                .unwrap_err()
        ),
        503
    );
    let doc = Doc::new();
    read(&doc, &next_binary(&mut a).await);
    a.send(Message::Binary(edit(&doc, &"z".repeat(200)).into()))
        .await
        .unwrap();
    let code = loop {
        match tokio::time::timeout(Duration::from_secs(5), a.next())
            .await
            .unwrap()
        {
            Some(Ok(Message::Close(frame))) => break frame.map(|f| f.code),
            Some(Ok(_)) => continue,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(code, Some(CloseCode::Size));
}

#[tokio::test]
async fn topics_reach_subscribers_of_the_same_tenant() {
    let (addr, hub, _) = start(Limits::default()).await;
    let here = format!("http://{addr}");
    let mut s = open(
        addr,
        &format!("/_zeyn/live/topics?t=orders&csrf={TOKEN}"),
        "ayse",
        false,
        &here,
    )
    .await
    .unwrap();
    let mut waited = 0;
    while hub.stats().subscribers == 0 && waited < 50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        waited += 1;
    }
    hub.notify_tenant(Some("other"), "orders");
    hub.notify_tenant(None, "orders");
    let text = loop {
        match tokio::time::timeout(Duration::from_secs(5), s.next())
            .await
            .unwrap()
        {
            Some(Ok(Message::Text(t))) => break t.to_string(),
            Some(Ok(_)) => continue,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(text, r#"{"topic":"orders"}"#);
}
