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
use zeyn::realtime::{
    Conn, Event, FRAME_UPDATE, Limits, MemoryDocStore, RoomKey, Socket as LiveSocket, ZeynSocket,
};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[screen(route = "/", permission = "notes.read")]
pub fn home() -> Screen {
    Screen::new("Home")
}

/// Sent to a room's group when a note is saved; any reader may get it.
#[derive(serde::Serialize, Event)]
#[event(name = "notes.saved")]
pub struct Saved {
    pub room: String,
}

/// Sent to the whole tenant; only writers may get it.
#[derive(serde::Serialize, Event)]
#[event(name = "notes.audit")]
pub struct Audited {
    pub by: String,
}

/// Never declared with `ZeynSocket::event`.
#[derive(serde::Serialize, Event)]
#[event(name = "notes.undeclared")]
pub struct Undeclared {}

#[params]
pub struct RoomName {
    pub name: String,
}

#[action(permission = "notes.read")]
pub async fn join_room(r: RoomName, conn: Conn) -> Result<()> {
    conn.join(r.name)
}

#[action(permission = "notes.write")]
pub async fn save_note(r: RoomName, socket: LiveSocket, ctx: Ctx) -> Result<()> {
    socket
        .to_group(&ctx, r.name.clone())
        .emit(&Saved { room: r.name })?;
    socket.to_tenant(&ctx).emit(&Audited {
        by: ctx
            .identity()
            .user
            .clone()
            .unwrap_or_default(),
    })?;
    Ok(())
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

async fn start(limits: Limits) -> (SocketAddr, LiveSocket, MemoryDocStore) {
    let (router, hub, store) = app(limits);
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

fn app(limits: Limits) -> (axum::Router, LiveSocket, MemoryDocStore) {
    let store = MemoryDocStore::default();
    let live = ZeynSocket::new()
        .documents("notes", "notes.read", "notes.write", store.clone())
        .topic("orders", "notes.read")
        .topic("secret", "notes.write")
        .event::<Saved>("notes.read")
        .event::<Audited>("notes.write")
        .limits(limits);
    let hub = live.socket();
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
            let identity = Identity::user(user).with_permissions(perms.iter().copied());
            Some(
                match h
                    .get("x-tenant")
                    .and_then(|v| v.to_str().ok())
                {
                    Some(t) => identity.with_tenant(t),
                    None => identity,
                },
            )
        })
        .feature(
            Feature::new("notes")
                .screen(home)
                .api(join_room)
                .api(save_note),
        )
        .interceptor(Suspended)
        .tenancy(zeyn::HeaderTenant("x-tenant"))
        .socket(live);
    (app.router().unwrap(), hub, store)
}

async fn open(
    addr: SocketAddr,
    path: &str,
    user: &str,
    write: bool,
    origin: &str,
) -> Result<Socket, WsError> {
    open_with(addr, path, user, write, origin, Some(TOKEN)).await
}

/// Opens a socket as the browser runtime does: the CSRF token travels in
/// `Sec-WebSocket-Protocol` next to `zeyn`, never in the address.
async fn open_with(
    addr: SocketAddr,
    path: &str,
    user: &str,
    write: bool,
    origin: &str,
    csrf: Option<&str>,
) -> Result<Socket, WsError> {
    let mut req = format!("ws://{addr}{path}")
        .into_client_request()
        .unwrap();
    let h = req.headers_mut();
    h.insert("origin", HeaderValue::from_str(origin).unwrap());
    if let Some(token) = csrf {
        h.insert(
            "sec-websocket-protocol",
            HeaderValue::from_str(&format!("zeyn, csrf.{token}")).unwrap(),
        );
    }
    h.insert(
        "cookie",
        HeaderValue::from_str(&format!("zeyn_csrf={TOKEN}")).unwrap(),
    );
    let (user, tenant) = user
        .split_once('@')
        .unwrap_or((user, ""));
    h.insert("x-user", HeaderValue::from_str(user).unwrap());
    if !tenant.is_empty() {
        h.insert("x-tenant", HeaderValue::from_str(tenant).unwrap());
    }
    if write {
        h.insert("x-write", HeaderValue::from_static("1"));
    }
    tokio_tungstenite::connect_async(req)
        .await
        .map(|(s, _)| s)
}

fn doc_path(doc: &str) -> String {
    format!("/_zeyn/live/doc/notes/{doc}")
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
            open_with(
                addr,
                "/_zeyn/live/doc/notes/a",
                "ayse",
                true,
                &here,
                Some("wrong")
            )
            .await
            .unwrap_err()
        ),
        403
    );
    assert_eq!(
        status(
            open(addr, "/_zeyn/live/doc/other/a", "ayse", true, &here)
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
            open_with(
                addr,
                &format!("/_zeyn/live/doc/notes/a?csrf={TOKEN}"),
                "ayse",
                true,
                &here,
                None
            )
            .await
            .unwrap_err()
        ),
        403,
        "a token in the address, where logs and history keep it, is not accepted"
    );
    let (_, answer) = tokio_tungstenite::connect_async({
        let mut req = format!("ws://{addr}{}", doc_path("a"))
            .into_client_request()
            .unwrap();
        let h = req.headers_mut();
        h.insert("origin", HeaderValue::from_str(&here).unwrap());
        h.insert(
            "cookie",
            HeaderValue::from_str(&format!("zeyn_csrf={TOKEN}")).unwrap(),
        );
        h.insert("x-user", HeaderValue::from_static("ayse"));
        h.insert(
            "sec-websocket-protocol",
            HeaderValue::from_str(&format!("zeyn, csrf.{TOKEN}")).unwrap(),
        );
        req
    })
    .await
    .unwrap();
    assert_eq!(
        answer
            .headers()
            .get("sec-websocket-protocol")
            .unwrap(),
        "zeyn",
        "the server picks `zeyn` and never echoes the token"
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
    let topics = PAGE;
    assert_eq!(
        status(
            open(addr, topics, "blocked", false, &here)
                .await
                .unwrap_err()
        ),
        403
    );
    assert!(
        open(addr, topics, "ayse", false, &here)
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

/// The page socket every page shares.
const PAGE: &str = "/_zeyn/socket";

/// Sends one JSON frame.
async fn send_json(s: &mut Socket, v: serde_json::Value) {
    s.send(Message::Text(v.to_string().into()))
        .await
        .unwrap();
}

/// The next JSON frame within five seconds.
async fn next_json(s: &mut Socket) -> serde_json::Value {
    let text = next_text(s, 5000)
        .await
        .expect("a frame");
    serde_json::from_str(&text).unwrap()
}

/// Waits until the socket holds `n` topic subscriptions.
async fn until_subscribers(hub: &LiveSocket, n: usize) {
    let mut waited = 0;
    while hub.stats().subscribers < n && waited < 100 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        waited += 1;
    }
}

#[tokio::test]
async fn topics_reach_subscribers_of_the_same_tenant() {
    let (addr, hub, _) = start(Limits::default()).await;
    let here = format!("http://{addr}");
    let mut s = open(addr, PAGE, "ayse", false, &here)
        .await
        .unwrap();
    send_json(
        &mut s,
        serde_json::json!({ "sub": ["orders", "secret", "nope"] }),
    )
    .await;
    assert_eq!(
        next_json(&mut s).await,
        serde_json::json!({ "denied": ["secret", "nope"] }),
        "topics without permission or undeclared are refused, the socket stays"
    );
    until_subscribers(&hub, 1).await;
    hub.notify_tenant(Some("other"), "orders");
    hub.notify_tenant(None, "orders");
    assert_eq!(
        next_json(&mut s).await,
        serde_json::json!({ "s": 1, "t": "orders" })
    );
    send_json(&mut s, serde_json::json!({ "unsub": ["orders"] })).await;
    let mut waited = 0;
    while hub.stats().subscribers > 0 && waited < 100 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        waited += 1;
    }
    assert_eq!(hub.stats().subscribers, 0);
}

#[tokio::test]
async fn actions_run_over_the_socket_and_events_reach_groups_with_permission() {
    let (addr, hub, _) = start(Limits::default()).await;
    let here = format!("http://{addr}");
    let mut reader = open(addr, PAGE, "ayse", false, &here)
        .await
        .unwrap();
    let mut writer = open(addr, PAGE, "bora", true, &here)
        .await
        .unwrap();
    let mut outsider = open(addr, PAGE, "cem", false, &here)
        .await
        .unwrap();
    for s in [&mut reader, &mut writer] {
        send_json(
            s,
            serde_json::json!({ "c": 1, "a": "join_room", "d": { "name": "r1" } }),
        )
        .await;
        assert_eq!(
            next_json(s).await,
            serde_json::json!({ "r": 1, "ok": null })
        );
    }
    assert_eq!(hub.stats().groups, 1);
    send_json(
        &mut reader,
        serde_json::json!({ "c": 2, "a": "save_note", "d": { "name": "r1" } }),
    )
    .await;
    let refused = next_json(&mut reader).await;
    assert_eq!(refused["r"], 2);
    assert_eq!(refused["err"]["status"], 403, "{refused}");
    send_json(
        &mut reader,
        serde_json::json!({ "c": 3, "a": "no_such_action", "d": null }),
    )
    .await;
    assert_eq!(next_json(&mut reader).await["err"]["status"], 404);

    send_json(
        &mut writer,
        serde_json::json!({ "c": 9, "a": "save_note", "d": { "name": "r1" } }),
    )
    .await;
    let mut got = Vec::new();
    for _ in 0..3 {
        got.push(next_json(&mut writer).await);
    }
    assert!(
        got.contains(&serde_json::json!({ "s": 1, "e": "notes.saved", "d": { "room": "r1" } })),
        "{got:?}"
    );
    assert!(
        got.contains(&serde_json::json!({ "s": 2, "e": "notes.audit", "d": { "by": "bora" } })),
        "{got:?}"
    );
    assert!(
        got.contains(&serde_json::json!({ "r": 9, "ok": null })),
        "{got:?}"
    );
    assert_eq!(
        next_json(&mut reader).await,
        serde_json::json!({ "s": 1, "e": "notes.saved", "d": { "room": "r1" } })
    );
    assert!(
        next_text(&mut reader, 300)
            .await
            .is_none(),
        "a reader never gets an event that needs another permission"
    );
    assert!(
        next_text(&mut outsider, 300)
            .await
            .is_none(),
        "a connection outside the group gets nothing"
    );
    assert!(
        hub.to_tenant_id(None)
            .emit(&Undeclared {})
            .is_err(),
        "an event not declared with ZeynSocket::event is refused"
    );
    for mut s in [reader, writer] {
        s.close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame {
            code: CloseCode::Normal,
            reason: "".into(),
        }))
        .await
        .unwrap();
    }
    let mut waited = 0;
    while hub.stats().groups > 0 && waited < 100 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        waited += 1;
    }
    assert_eq!(
        hub.stats().groups,
        0,
        "closed connections leave their groups"
    );
}

#[tokio::test]
async fn an_action_that_takes_the_connection_refuses_plain_http() {
    let (addr, _, _) = start(Limits::default()).await;
    let res = reqwest_post(addr, "/api/v1/notes/join_room", r#"{"name":"r1"}"#).await;
    assert!(res.starts_with("422"), "{res}");
    assert!(res.contains("live socket"), "{res}");
}

/// A minimal HTTP POST over a TCP stream; returns the status line code and the body.
async fn reqwest_post(addr: SocketAddr, path: &str, body: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(addr)
        .await
        .unwrap();
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {addr}\r\nx-user: ayse\r\nAuthorization: Bearer t\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(req.as_bytes())
        .await
        .unwrap();
    let mut out = String::new();
    stream
        .read_to_string(&mut out)
        .await
        .unwrap();
    let code = out
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_string();
    format!("{code} {out}")
}

#[tokio::test]
async fn calls_over_the_limit_per_second_answer_429() {
    let limits = Limits {
        calls: 2,
        ..Limits::default()
    };
    let (addr, _, _) = start(limits).await;
    let here = format!("http://{addr}");
    let mut s = open(addr, PAGE, "ayse", false, &here)
        .await
        .unwrap();
    for c in 1..=3 {
        send_json(
            &mut s,
            serde_json::json!({ "c": c, "a": "join_room", "d": { "name": format!("g{c}") } }),
        )
        .await;
    }
    let mut statuses = Vec::new();
    for _ in 0..3 {
        let v = next_json(&mut s).await;
        statuses.push(
            v["err"]["status"]
                .as_u64()
                .unwrap_or(200),
        );
    }
    statuses.sort();
    assert_eq!(statuses, [200, 200, 429]);
}

#[tokio::test]
async fn a_bad_frame_closes_the_socket_with_policy() {
    let (addr, _, _) = start(Limits::default()).await;
    let here = format!("http://{addr}");
    let mut s = open(addr, PAGE, "ayse", false, &here)
        .await
        .unwrap();
    s.send(Message::Text("not json".into()))
        .await
        .unwrap();
    let code = loop {
        match tokio::time::timeout(Duration::from_secs(5), s.next())
            .await
            .unwrap()
        {
            Some(Ok(Message::Close(frame))) => break frame.map(|f| f.code),
            Some(Ok(_)) => continue,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(code, Some(CloseCode::Policy));
}

/// Opens a page socket, says hello (resuming `token` at `seq` when given) and subscribes to
/// `orders`; returns the socket and the `welcome` frame.
async fn hello(
    addr: SocketAddr,
    user: &str,
    resume: Option<(&str, u64)>,
) -> (Socket, serde_json::Value) {
    let here = format!("http://{addr}");
    let mut s = open(addr, PAGE, user, false, &here)
        .await
        .unwrap();
    let hello = match resume {
        Some((token, seq)) => serde_json::json!({ "hello": { "resume": token, "seq": seq } }),
        None => serde_json::json!({ "hello": {} }),
    };
    send_json(&mut s, hello).await;
    let welcome = next_json(&mut s).await;
    send_json(&mut s, serde_json::json!({ "sub": ["orders"] })).await;
    (s, welcome)
}

/// Waits until the socket holds exactly `n` page connections.
async fn until_connections(hub: &LiveSocket, n: usize) {
    let mut waited = 0;
    while hub.stats().connections != n && waited < 150 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        waited += 1;
    }
}

#[tokio::test]
async fn a_dropped_socket_resumes_and_gets_what_it_missed() {
    let (addr, hub, _) = start(Limits::default()).await;
    let (mut s, welcome) = hello(addr, "ayse", None).await;
    assert_eq!(welcome["welcome"]["resumed"], false);
    let token = welcome["welcome"]["resume"]
        .as_str()
        .unwrap()
        .to_string();
    until_subscribers(&hub, 1).await;
    hub.notify_tenant(None, "orders");
    assert_eq!(
        next_json(&mut s).await,
        serde_json::json!({ "s": 1, "t": "orders" })
    );
    drop(s);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        hub.stats().connections,
        1,
        "a dropped socket is kept for a resume"
    );
    hub.notify_tenant(None, "orders");
    hub.notify_tenant(None, "orders");

    let (mut again, welcome) = hello(addr, "ayse", Some((&token, 1))).await;
    assert_eq!(welcome["welcome"]["resumed"], true, "{welcome}");
    assert_eq!(welcome["welcome"]["replayed"], 2);
    assert_eq!(
        next_json(&mut again).await,
        serde_json::json!({ "s": 2, "t": "orders" })
    );
    assert_eq!(
        next_json(&mut again).await,
        serde_json::json!({ "s": 3, "t": "orders" })
    );
    until_connections(&hub, 1).await;
    assert_eq!(
        hub.stats().connections,
        1,
        "the resumed peer replaces the new one"
    );
    assert_eq!(hub.stats().subscribers, 1, "and keeps its topics once");
    hub.notify_tenant(None, "orders");
    assert_eq!(
        next_json(&mut again).await,
        serde_json::json!({ "s": 4, "t": "orders" })
    );
}

#[tokio::test]
async fn a_resume_past_the_buffer_or_of_another_user_gets_resync() {
    let limits = Limits {
        replay: 1,
        ..Limits::default()
    };
    let (addr, hub, _) = start(limits).await;
    let (s, welcome) = hello(addr, "ayse", None).await;
    let token = welcome["welcome"]["resume"]
        .as_str()
        .unwrap()
        .to_string();
    until_subscribers(&hub, 1).await;
    drop(s);
    tokio::time::sleep(Duration::from_millis(200)).await;
    for _ in 0..3 {
        hub.notify_tenant(None, "orders");
    }
    let (mut other, welcome) = hello(addr, "bora", Some((&token, 0))).await;
    assert_eq!(
        welcome["welcome"]["resumed"], false,
        "a token of another user resumes nothing"
    );
    assert_eq!(
        next_json(&mut other).await,
        serde_json::json!({ "resync": true })
    );

    let (mut late, welcome) = hello(addr, "ayse", Some((&token, 0))).await;
    assert_eq!(welcome["welcome"]["resumed"], true);
    assert_eq!(welcome["welcome"]["seq"], 3);
    assert_eq!(
        next_json(&mut late).await,
        serde_json::json!({ "resync": true }),
        "two of the three missed messages are gone"
    );
}

#[tokio::test]
async fn detached_sockets_expire_and_clean_closes_are_forgotten_at_once() {
    let limits = Limits {
        detached: Duration::from_millis(100),
        ..Limits::default()
    };
    let (addr, hub, _) = start(limits).await;
    let (s, welcome) = hello(addr, "ayse", None).await;
    let token = welcome["welcome"]["resume"]
        .as_str()
        .unwrap()
        .to_string();
    drop(s);
    until_connections(&hub, 0).await;
    assert_eq!(hub.stats().connections, 0, "the dropped socket expired");
    let (mut s, welcome) = hello(addr, "ayse", Some((&token, 0))).await;
    assert_eq!(welcome["welcome"]["resumed"], false);
    assert_eq!(
        next_json(&mut s).await,
        serde_json::json!({ "resync": true })
    );

    let (addr, hub, _) = start(Limits::default()).await;
    let (mut s, _) = hello(addr, "ayse", None).await;
    until_connections(&hub, 1).await;
    s.close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame {
        code: CloseCode::Normal,
        reason: "".into(),
    }))
    .await
    .unwrap();
    until_connections(&hub, 0).await;
    assert_eq!(
        hub.stats().connections,
        0,
        "a page that closes on purpose is not kept for a minute"
    );
}

#[tokio::test]
async fn topics_groups_events_and_resumes_stay_inside_their_tenant() {
    let (addr, hub, _) = start(Limits::default()).await;
    let here = format!("http://{addr}");
    let (mut a, welcome_a) = hello(addr, "ayse@acme", None).await;
    let (mut b, _) = hello(addr, "ayse@globex", None).await;
    let mut writer_a = open(addr, PAGE, "bora@acme", true, &here)
        .await
        .unwrap();
    let mut writer_b = open(addr, PAGE, "bora@globex", true, &here)
        .await
        .unwrap();
    for s in [&mut a, &mut b, &mut writer_a, &mut writer_b] {
        send_json(
            s,
            serde_json::json!({ "c": 1, "a": "join_room", "d": { "name": "r1" } }),
        )
        .await;
        assert_eq!(next_json(s).await["r"], 1);
    }
    assert_eq!(
        hub.stats().groups,
        2,
        "the same group name is one group per tenant"
    );
    until_subscribers(&hub, 2).await;

    hub.notify_tenant(Some("acme"), "orders");
    assert_eq!(next_json(&mut a).await["t"], "orders");
    assert!(
        next_text(&mut b, 300).await.is_none(),
        "a topic of acme never reaches globex"
    );

    send_json(
        &mut writer_a,
        serde_json::json!({ "c": 2, "a": "save_note", "d": { "name": "r1" } }),
    )
    .await;
    assert_eq!(next_json(&mut a).await["e"], "notes.saved");
    let mut got = Vec::new();
    for _ in 0..3 {
        got.push(next_json(&mut writer_a).await);
    }
    assert!(
        got.iter()
            .any(|v| v["e"] == "notes.audit"),
        "{got:?}"
    );
    assert!(
        next_text(&mut b, 300).await.is_none(),
        "a group event of acme never reaches globex"
    );
    assert!(
        next_text(&mut writer_b, 300)
            .await
            .is_none(),
        "a tenant event of acme never reaches globex"
    );

    let token = welcome_a["welcome"]["resume"]
        .as_str()
        .unwrap()
        .to_string();
    drop(a);
    tokio::time::sleep(Duration::from_millis(200)).await;
    let (_, welcome) = hello(addr, "ayse@globex", Some((&token, 0))).await;
    assert_eq!(
        welcome["welcome"]["resumed"], false,
        "the same user id in another tenant cannot resume the connection"
    );
}

#[tokio::test]
async fn a_flood_of_frames_or_one_too_large_closes_the_socket() {
    let limits = Limits {
        frames: 5,
        message_bytes: 1024,
        ..Limits::default()
    };
    let (addr, _, _) = start(limits).await;
    let here = format!("http://{addr}");
    let close_code = |mut s: Socket| async move {
        loop {
            match tokio::time::timeout(Duration::from_secs(5), s.next())
                .await
                .unwrap()
            {
                Some(Ok(Message::Close(frame))) => break frame.map(|f| f.code),
                Some(Ok(_)) => continue,
                Some(Err(_)) | None => break None,
            }
        }
    };
    let mut s = open(addr, PAGE, "ayse", false, &here)
        .await
        .unwrap();
    for _ in 0..10 {
        let _ = s
            .send(Message::Text(r#"{"ack":0}"#.into()))
            .await;
    }
    assert_eq!(close_code(s).await, Some(CloseCode::Policy));
    let mut s = open(addr, PAGE, "ayse", false, &here)
        .await
        .unwrap();
    let big = format!(r#"{{"sub":["{}"]}}"#, "x".repeat(2000));
    let _ = s.send(Message::Text(big.into())).await;
    assert_eq!(close_code(s).await, Some(CloseCode::Size));
}

/// A request of the fallback transport: the user's header, and the CSRF cookie and header
/// unless `csrf` is false.
fn sse_request(
    method: &str,
    path: &str,
    user: &str,
    csrf: bool,
    body: &str,
) -> axum::http::Request<axum::body::Body> {
    let mut b = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header("x-user", user)
        .header("content-type", "application/json")
        .header("cookie", format!("zeyn_csrf={TOKEN}"));
    if csrf {
        b = b.header("x-zeyn-csrf", TOKEN);
    }
    b.body(axum::body::Body::from(body.to_string()))
        .unwrap()
}

/// Reads the next `data:` line of an event stream body within five seconds.
async fn next_data(body: &mut axum::body::Body, held: &mut String) -> serde_json::Value {
    use http_body_util::BodyExt;
    loop {
        if let Some(end) = held.find("\n\n") {
            let event: String = held.drain(..end + 2).collect();
            if let Some(data) = event
                .lines()
                .find_map(|l| l.strip_prefix("data: "))
            {
                return serde_json::from_str(data).unwrap();
            }
            continue;
        }
        let frame = tokio::time::timeout(Duration::from_secs(5), body.frame())
            .await
            .expect("an event in time")
            .expect("the stream is open")
            .unwrap();
        if let Ok(bytes) = frame.into_data() {
            held.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
}

/// Opens the fallback transport for `user` and returns the event stream, the resume token and
/// what is left of the stream's first read.
async fn sse_open(
    router: &axum::Router,
    user: &str,
    open: &str,
) -> (axum::body::Body, String, String, serde_json::Value) {
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let res = router
        .clone()
        .oneshot(sse_request("POST", "/_zeyn/socket/open", user, true, open))
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let opened: serde_json::Value = serde_json::from_slice(
        &res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let ticket = opened["ticket"].as_str().unwrap();
    let res = router
        .clone()
        .oneshot(sse_request(
            "GET",
            &format!("/_zeyn/socket/events?ticket={ticket}"),
            user,
            false,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert!(
        res.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    let mut body = res.into_body();
    let mut held = String::new();
    let welcome = next_data(&mut body, &mut held).await;
    let token = welcome["welcome"]["resume"]
        .as_str()
        .unwrap()
        .to_string();
    (body, token, held, welcome)
}

/// Sends one frame up the fallback transport and returns the status.
async fn sse_send(router: &axum::Router, user: &str, token: &str, frame: serde_json::Value) -> u16 {
    use tower::ServiceExt;
    let mut req = sse_request("POST", "/_zeyn/socket/send", user, true, &frame.to_string());
    req.headers_mut()
        .insert("x-zeyn-socket", HeaderValue::from_str(token).unwrap());
    router
        .clone()
        .oneshot(req)
        .await
        .unwrap()
        .status()
        .as_u16()
}

#[tokio::test]
async fn the_fallback_transport_carries_topics_calls_and_resume_over_events_and_posts() {
    let (router, hub, _) = app(Limits::default());
    let (mut body, token, mut held, welcome) = sse_open(&router, "ayse", "").await;
    assert_eq!(welcome["welcome"]["resumed"], false);
    assert_eq!(
        sse_send(
            &router,
            "ayse",
            &token,
            serde_json::json!({ "sub": ["orders"] })
        )
        .await,
        204
    );
    until_subscribers(&hub, 1).await;
    hub.notify_tenant(None, "orders");
    assert_eq!(
        next_data(&mut body, &mut held).await,
        serde_json::json!({ "s": 1, "t": "orders" })
    );
    assert_eq!(
        sse_send(
            &router,
            "ayse",
            &token,
            serde_json::json!({ "c": 4, "a": "join_room", "d": { "name": "r1" } })
        )
        .await,
        204
    );
    assert_eq!(
        next_data(&mut body, &mut held).await,
        serde_json::json!({ "r": 4, "ok": null })
    );

    drop(body);
    tokio::time::sleep(Duration::from_millis(100)).await;
    hub.notify_tenant(None, "orders");
    let open = serde_json::json!({ "resume": token, "seq": 1 }).to_string();
    let (mut body, _, mut held, welcome) = sse_open(&router, "ayse", &open).await;
    assert_eq!(welcome["welcome"]["resumed"], true, "{welcome}");
    assert_eq!(
        next_data(&mut body, &mut held).await,
        serde_json::json!({ "s": 2, "t": "orders" })
    );
}

#[tokio::test]
async fn the_fallback_transport_checks_csrf_tickets_and_tokens() {
    use tower::ServiceExt;
    let (router, _, _) = app(Limits::default());
    let status = |req: axum::http::Request<axum::body::Body>| {
        let router = router.clone();
        async move {
            router
                .oneshot(req)
                .await
                .unwrap()
                .status()
                .as_u16()
        }
    };
    assert_eq!(
        status(sse_request("POST", "/_zeyn/socket/open", "ayse", false, "")).await,
        403,
        "opening needs the CSRF token"
    );
    let (_body, token, _, _) = sse_open(&router, "ayse", "").await;
    assert_eq!(
        sse_send(
            &router,
            "bora",
            &token,
            serde_json::json!({ "sub": ["orders"] })
        )
        .await,
        404,
        "another user cannot send on the connection"
    );
    assert_eq!(
        sse_send(
            &router,
            "ayse",
            "0.00",
            serde_json::json!({ "sub": ["orders"] })
        )
        .await,
        404,
        "a made up token names nothing"
    );
    let mut no_csrf = sse_request(
        "POST",
        "/_zeyn/socket/send",
        "ayse",
        false,
        r#"{"sub":["orders"]}"#,
    );
    no_csrf
        .headers_mut()
        .insert("x-zeyn-socket", HeaderValue::from_str(&token).unwrap());
    assert_eq!(status(no_csrf).await, 403, "sending needs the CSRF token");

    use http_body_util::BodyExt;
    let res = router
        .clone()
        .oneshot(sse_request("POST", "/_zeyn/socket/open", "ayse", true, ""))
        .await
        .unwrap();
    let opened: serde_json::Value = serde_json::from_slice(
        &res.into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let path = format!(
        "/_zeyn/socket/events?ticket={}",
        opened["ticket"].as_str().unwrap()
    );
    assert_eq!(
        status(sse_request("GET", &path, "bora", false, "")).await,
        403,
        "a ticket works only for the user who opened it"
    );
    assert_eq!(
        status(sse_request("GET", &path, "ayse", false, "")).await,
        404,
        "and only once"
    );
}

/// The next text frame within `ms`, or `None`.
async fn next_text(s: &mut Socket, ms: u64) -> Option<String> {
    loop {
        match tokio::time::timeout(Duration::from_millis(ms), s.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => return Some(t.to_string()),
            Ok(Some(Ok(_))) => continue,
            _ => return None,
        }
    }
}

#[tokio::test]
async fn typing_signals_reach_other_users_only_throttled_and_without_user_ids() {
    let (addr, hub, _) = start(Limits::default()).await;
    let here = format!("http://{addr}");
    let mut ada = open(addr, PAGE, "ada", false, &here)
        .await
        .unwrap();
    let mut ada2 = open(addr, PAGE, "ada", false, &here)
        .await
        .unwrap();
    let mut grace = open(addr, PAGE, "grace", false, &here)
        .await
        .unwrap();
    for s in [&mut ada, &mut ada2, &mut grace] {
        send_json(s, serde_json::json!({ "sub": ["orders"] })).await;
    }
    until_subscribers(&hub, 3).await;
    let signal = r#"{"t":"typing","topic":"orders","room":"r-1"}"#;
    ada.send(Message::Text(signal.into()))
        .await
        .unwrap();
    let got = next_text(&mut grace, 5000)
        .await
        .expect("grace hears ada typing");
    let v: serde_json::Value = serde_json::from_str(&got).unwrap();
    assert_eq!(v["t"], "orders");
    assert_eq!(v["typing"]["room"], "r-1");
    assert_eq!(v["typing"]["who"], hub.person_key("", "ada"));
    assert!(!got.contains("ada"), "the user id never travels: {got}");
    assert!(
        next_text(&mut ada2, 300)
            .await
            .is_none(),
        "never back to the same user"
    );
    assert!(next_text(&mut ada, 100).await.is_none());
    ada.send(Message::Text(signal.into()))
        .await
        .unwrap();
    assert!(
        next_text(&mut grace, 400)
            .await
            .is_none(),
        "one signal per user every few seconds"
    );
    grace
        .send(Message::Text(
            r#"{"t":"typing","topic":"secret","room":"r-1"}"#.into(),
        ))
        .await
        .unwrap();
    assert!(
        next_text(&mut ada, 400).await.is_none(),
        "topics the socket did not subscribe to are ignored"
    );
    assert_ne!(hub.person_key("", "ada"), hub.person_key("t2", "ada"));
}
