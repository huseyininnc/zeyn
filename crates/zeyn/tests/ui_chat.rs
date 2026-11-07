//! Chat components: the message thread with its history and streamed AI replies, and the
//! composer with its sends, mentions, attachments and drafts.

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::json;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::ui::{
    ChatSend, Composer, Delivery, LookupItem, LookupQuery, MAX_REPLY_CHARS, MessageThread, Reply,
    ReplyRequest, SendKey, ThreadQuery,
};
use zeyn::{I18n, Identity};

#[entity(table = "messages")]
pub struct Msg {
    #[key]
    pub id: MsgId,
    pub room: String,
    pub author: String,
    pub author_name: String,
    pub body: String,
    pub sent_at: DateTime,
    pub status: Delivery,
    pub files: Vec<File>,
    pub ai: bool,
}

#[derive(Clone, Default)]
pub struct Chat(Arc<Mutex<Vec<Msg>>>);

impl Service for Chat {}

pub const CHAT_FILES: FileRules = FileRules::new("chat");

const MINUTE: i64 = 60_000;

impl Chat {
    /// 120 messages in room `a` (one a minute, alternating senders, the last ones today) and one
    /// in room `b`.
    fn seeded() -> Self {
        let now = DateTime::now();
        let mut rows: Vec<Msg> = (0..120)
            .map(|i| Msg {
                id: MsgId(i + 1),
                room: "a".into(),
                author: if (i / 2) % 2 == 0 { "u" } else { "ada" }.into(),
                author_name: if (i / 2) % 2 == 0 {
                    "Me"
                } else {
                    "Ada <b>L</b>"
                }
                .into(),
                body: format!("message {}", i + 1),
                sent_at: now.add_ms(-(120 - i) * MINUTE),
                status: Delivery::Read,
                files: Vec::new(),
                ai: false,
            })
            .collect();
        rows[119].body = "<script>alert(1)</script>\nsecond line".into();
        rows[118].files = vec![
            File {
                id: "0123456789abcdef0123456789abcdef".into(),
                name: "photo <1>.png".into(),
                size: 2048,
                content_type: "image/png".into(),
            },
            File {
                id: "fedcba9876543210fedcba9876543210".into(),
                name: "terms.pdf".into(),
                size: 4096,
                content_type: "application/pdf".into(),
            },
            File {
                id: "../../etc/passwd".into(),
                name: "evil".into(),
                size: 1,
                content_type: "image/png".into(),
            },
        ];
        rows[0].sent_at = now.add_ms(-3 * 24 * 60 * MINUTE);
        rows.push(Msg {
            id: MsgId(500),
            room: "b".into(),
            author: "ada".into(),
            author_name: "Ada".into(),
            body: "secret of room b".into(),
            sent_at: now,
            status: Delivery::Sent,
            files: Vec::new(),
            ai: false,
        });
        Chat(Arc::new(Mutex::new(rows)))
    }
}

#[action(permission = "chat.read")]
pub async fn room_messages(q: ThreadQuery, chat: Chat) -> Result<Page<Msg>> {
    let rows: Vec<Msg> = chat
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|m| m.room == q.conversation)
        .cloned()
        .collect();
    Ok(q.query.apply(rows))
}

/// Answers with the user's text back, then a very long tail when asked for `long`.
#[action(permission = "chat.ai")]
pub async fn echo_reply(r: ReplyRequest) -> Result<Reply> {
    let (tx, reply) = Reply::channel("900");
    tokio::spawn(async move {
        if !tx
            .send(format!("You said **{}** in {}", r.text, r.conversation))
            .await
        {
            return;
        }
        if r.text == "long" {
            for _ in 0..50 {
                if !tx.send("x".repeat(1000)).await {
                    return;
                }
            }
        }
    });
    Ok(reply)
}

#[screen(route = "/plain", permission = "chat.read")]
pub fn plain() -> Screen {
    Screen::new("Plain").body(Paragraph::new("Only text"))
}

#[screen(route = "/room", permission = "chat.read")]
pub fn room() -> Screen {
    Screen::new("Room").body(
        Card::new().add(
            MessageThread::of(room_messages)
                .conversation("a")
                .author(Msg::author)
                .name(Msg::author_name)
                .text(Msg::body)
                .when(Msg::sent_at)
                .status(Msg::status)
                .attachments(Msg::files, CHAT_FILES)
                .ai(Msg::ai)
                .reply(echo_reply),
        ),
    )
}

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ALL: &str = "chat.read,chat.write,chat.ai";

fn app_with(chat: Chat, writes: u32) -> axum::Router {
    app_sent(chat, Sent::default(), writes)
}

fn app_sent(chat: Chat, sent: Sent, writes: u32) -> axum::Router {
    App::new("Chat")
        .i18n(I18n::new("en"))
        .service(chat)
        .service(sent)
        .client_prefs(|p| p.key(CHAT_DRAFTS))
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
        .write_limit(zeyn::RateLimit::per_ip(
            writes,
            std::time::Duration::from_secs(60),
        ))
        .feature(
            Feature::new("chat")
                .screen(plain)
                .screen(room)
                .screen(write)
                .files(CHAT_FILES, "chat.write", zeyn::MemoryStore::default()),
        )
        .router()
        .unwrap()
}

fn app(chat: Chat) -> axum::Router {
    app_with(chat, 1000)
}

async fn body(res: axum::response::Response) -> (StatusCode, String) {
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

async fn get_as(router: &axum::Router, path: &str, perms: &str) -> (StatusCode, String) {
    let res = router
        .clone()
        .oneshot(
            Request::get(path)
                .header("x-perms", perms)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    body(res).await
}

#[derive(PartialEq, Clone, Copy)]
enum Csrf {
    Sent,
    Missing,
}

/// Posts JSON to `path`, with the CSRF header unless `csrf` is `Csrf::Missing`.
async fn post(
    router: &axum::Router,
    path: &str,
    perms: &str,
    csrf: Csrf,
    call: serde_json::Value,
) -> (StatusCode, String) {
    let mut req = Request::post(path)
        .header("x-perms", perms)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={TOKEN}"))
        .header("x-forwarded-for", "203.0.113.9");
    if csrf == Csrf::Sent {
        req = req.header("x-zeyn-csrf", TOKEN);
    }
    let res = router
        .clone()
        .oneshot(
            req.body(Body::from(call.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    body(res).await
}

fn grid_call(node: &str, offset: u64, limit: u32) -> serde_json::Value {
    json!({ "screen": "room", "params": null, "node": node, "query": { "page": { "type": "offset", "offset": offset, "limit": limit }, "with_total": true } })
}

fn reply_call(node: &str, key: &str, text: &str) -> serde_json::Value {
    json!({ "screen": "room", "params": null, "node": node, "key": key, "input": { "text": text } })
}

#[tokio::test]
async fn the_thread_module_loads_only_on_pages_with_a_thread() {
    let router = app(Chat::seeded());
    let (_, bare) = get_as(&router, "/plain", ALL).await;
    for name in ["thread.js", "thread.css", "stream.js"] {
        assert!(!bare.contains(&format!("/_zeyn/{name}")), "{name}");
    }
    let (status, page) = get_as(&router, "/room", ALL).await;
    assert_eq!(status, StatusCode::OK);
    for name in ["thread.js", "thread.css", "stream.js"] {
        assert!(page.contains(&format!("/_zeyn/{name}?v=")), "{name}");
    }
    let (status, js) = get_as(&router, "/_zeyn/thread.js", "").await;
    assert_eq!(status, StatusCode::OK);
    assert!(js.contains("stream.js"));
    let (status, js) = get_as(&router, "/_zeyn/stream.js", "").await;
    assert_eq!(status, StatusCode::OK);
    assert!(js.contains("export function writer") && !js.contains("innerHTML"));
}

#[tokio::test]
async fn bubbles_group_by_sender_and_day_and_escape_text_and_names() {
    let (status, page) = get_as(&app(Chat::seeded()), "/room", ALL).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        page.matches("<article class=\"z-thread-msg")
            .count(),
        zeyn::ui::THREAD_PAGE as usize
    );
    assert!(page.contains(r#"data-room="a""#) && page.contains("data-zeyn-thread-reply"));
    assert!(page.contains(r#"aria-posinset="120" aria-setsize="120""#));
    assert!(page.contains(r#"aria-posinset="71" aria-setsize="120""#));
    assert!(!page.contains(r#"aria-posinset="70""#));
    assert!(page.contains(r#"role="feed""#));
    assert!(
        page.contains("z-thread-msg z-thread-own") && page.contains("z-thread-msg z-thread-other")
    );
    assert!(
        page.contains("z-thread-cont"),
        "consecutive messages of one sender group"
    );
    assert!(page.contains(">Today</span></h3>"));
    assert_eq!(
        page.matches(r#"class="z-thread-day""#)
            .count(),
        1
    );
    assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt;<br>second line"));
    assert!(!page.contains("<script>alert(1)"));
    assert!(page.contains("Ada &lt;b&gt;L&lt;/b&gt;") && !page.contains("Ada <b>L</b>"));
    assert!(page.contains(r#"<span class="z-sr-only">Read</span>"#));
    assert!(
        !page.contains("secret of room b"),
        "only the thread's conversation"
    );
    assert!(page.contains("photo &lt;1&gt;.png") && !page.contains("photo <1>"));
    assert!(page.contains(r#"href="/_zeyn/files/chat/fedcba9876543210fedcba9876543210""#));
    assert!(page.contains("/_zeyn/files/chat/0123456789abcdef0123456789abcdef"));
    assert!(
        !page.contains("etc/passwd"),
        "file ids of the wrong shape are left out"
    );
    assert!(page.contains(r#"loading="lazy""#), "images load lazily");
}

#[tokio::test]
async fn older_pages_load_through_the_grid_endpoint_with_a_nested_path_and_a_capped_size() {
    let router = app(Chat::seeded());
    let (status, body) = post(
        &router,
        "/_zeyn/grid",
        ALL,
        Csrf::Sent,
        grid_call("0.0", 50, 500),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["count"], 50, "the server keeps the page size");
    assert_eq!(v["total"], 120);
    let rows = v["rows"].as_str().unwrap();
    assert!(rows.contains(r#"aria-posinset="70""#) && rows.contains(r#"aria-posinset="21""#));
    assert!(!rows.contains(r#"aria-posinset="71""#));
    let (_, last) = post(
        &router,
        "/_zeyn/grid",
        ALL,
        Csrf::Sent,
        grid_call("0.0", 100, 50),
    )
    .await;
    let v: serde_json::Value = serde_json::from_str(&last).unwrap();
    assert_eq!(v["count"], 20);
    assert!(
        v["rows"]
            .as_str()
            .unwrap()
            .contains(r#"aria-posinset="1""#)
    );
    let (status, _) = post(
        &router,
        "/_zeyn/grid",
        "chat.write",
        Csrf::Sent,
        grid_call("0.0", 0, 50),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the history needs the action's permission"
    );
    let (status, _) = post(
        &router,
        "/_zeyn/grid",
        ALL,
        Csrf::Missing,
        grid_call("0.0", 0, 50),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = post(
        &router,
        "/_zeyn/grid",
        ALL,
        Csrf::Sent,
        grid_call("0", 0, 50),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "the card is not a thread");
}

/// Text of the `text` events of a streamed reply.
fn streamed(body: &str) -> String {
    body.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|e| e["t"] == "text")
        .filter_map(|e| e["v"].as_str().map(String::from))
        .collect()
}

#[tokio::test]
async fn ai_replies_stream_as_json_lines_with_csrf_the_permission_and_a_cap() {
    let router = app(Chat::seeded());
    let (status, body) = post(
        &router,
        "/_zeyn/stream",
        ALL,
        Csrf::Sent,
        reply_call("0.0", "reply", "hi <b>"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let lines: Vec<serde_json::Value> = body
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines[0], json!({ "t": "start", "key": "900" }));
    assert_eq!(lines.last().unwrap(), &json!({ "t": "done" }));
    assert_eq!(
        streamed(&body),
        "You said **hi <b>** in a",
        "the conversation comes from the server"
    );
    let (status, _) = post(
        &router,
        "/_zeyn/stream",
        ALL,
        Csrf::Missing,
        reply_call("0.0", "reply", "hi"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = post(
        &router,
        "/_zeyn/stream",
        "chat.read",
        Csrf::Sent,
        reply_call("0.0", "reply", "hi"),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "the reply needs its own permission"
    );
    let (status, _) = post(
        &router,
        "/_zeyn/stream",
        ALL,
        Csrf::Sent,
        reply_call("0.0", "reply", "   "),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(
        &router,
        "/_zeyn/stream",
        ALL,
        Csrf::Sent,
        reply_call("0.0", "other", "hi"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(
        &router,
        "/_zeyn/stream",
        ALL,
        Csrf::Sent,
        reply_call("0", "reply", "hi"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = post(
        &router,
        "/_zeyn/stream",
        ALL,
        Csrf::Sent,
        reply_call("0.0", "reply", "long"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(streamed(&body).chars().count(), MAX_REPLY_CHARS);
    assert!(body.ends_with("{\"t\":\"done\"}\n"));
    let (_, reader) = get_as(&router, "/room", "chat.read").await;
    assert!(
        !reader.contains("data-zeyn-thread-reply"),
        "no reply without its permission"
    );
}

#[tokio::test]
async fn streamed_replies_count_against_the_write_limit() {
    let router = app_with(Chat::seeded(), 1);
    let (status, _) = post(
        &router,
        "/_zeyn/stream",
        ALL,
        Csrf::Sent,
        reply_call("0.0", "reply", "hi"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = post(
        &router,
        "/_zeyn/stream",
        ALL,
        Csrf::Sent,
        reply_call("0.0", "reply", "hi"),
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[derive(Clone, Default)]
pub struct Sent(Arc<Mutex<Vec<ChatSend>>>);

impl Service for Sent {}

#[action(permission = "chat.write")]
pub async fn send_message(m: ChatSend, sent: Sent) -> Result<()> {
    sent.0.lock().unwrap().push(m);
    Ok(())
}

#[action(permission = "chat.read")]
pub async fn search_people(q: LookupQuery) -> Result<Page<LookupItem>> {
    let people: Vec<LookupItem> = (1..=30)
        .map(|i| LookupItem::new(i, format!("Person <{i}>")))
        .collect();
    Ok(Page::new(people).total(30 + u64::from(q.limit)))
}

pub const CHAT_DRAFTS: zeyn::ClientPref<zeyn::Draft> = zeyn::ClientPref::draft(
    "chat-draft",
    std::time::Duration::from_secs(86_400),
    zeyn::Purpose::Preference,
);

#[screen(route = "/write", permission = "chat.read")]
pub fn write() -> Screen {
    Screen::new("Write").body(
        Stack::new().add(
            Composer::of(send_message)
                .conversation("a")
                .max_chars(20)
                .send_key(SendKey::CtrlEnter)
                .attachments(CHAT_FILES)
                .mentions(search_people)
                .remember(CHAT_DRAFTS),
        ),
    )
}

fn send_call(input: serde_json::Value) -> serde_json::Value {
    json!({ "screen": "write", "params": null, "node": "0.0", "key": "send", "input": input })
}

fn file(id: &str) -> serde_json::Value {
    json!({ "id": id, "name": "a.png", "size": 10, "type": "image/png" })
}

#[tokio::test]
async fn the_composer_renders_its_limits_and_keys_and_loads_only_where_used() {
    let router = app(Chat::seeded());
    let (_, bare) = get_as(&router, "/plain", ALL).await;
    assert!(!bare.contains("/_zeyn/composer.js") && !bare.contains("/_zeyn/files.js"));
    let (status, page) = get_as(&router, "/write", ALL).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("/_zeyn/composer.js?v=") && page.contains("/_zeyn/composer.css?v="));
    assert!(
        page.contains("/_zeyn/files.js?v="),
        "attachments bring the upload module"
    );
    assert!(page.contains(r#"data-send-key="ctrl-enter""#) && page.contains(r#"data-max="20""#));
    assert!(page.contains(r#"data-room="a""#) && page.contains("data-mentions"));
    assert!(
        page.contains(r#"data-zeyn-remember="chat-draft""#)
            && page.contains(r#"data-zeyn-remember-max="4000""#)
    );
    assert!(
        page.contains(r#"aria-disabled="true""#),
        "nothing to send yet"
    );
    assert!(page.contains(r#"role="listbox""#) && page.contains(r#"aria-autocomplete="list""#));
    let (_, reader) = get_as(&router, "/write", "chat.read").await;
    assert!(
        !reader.contains("data-zeyn-composer"),
        "no composer without the send permission"
    );
    assert!(!reader.contains("/_zeyn/composer.js"));
}

#[tokio::test]
async fn sending_needs_csrf_the_permission_and_a_message_that_fits() {
    let sent = Sent::default();
    let router = app_sent(Chat::seeded(), sent.clone(), 1000);
    let ok = send_call(
        json!({ "text": "  hi <b>  ", "mentions": [3, 3, 1], "attachments": [file("0123456789abcdef0123456789abcdef")] }),
    );
    let (status, _) = post(&router, "/_zeyn/data", ALL, Csrf::Missing, ok.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = post(&router, "/_zeyn/data", "chat.read", Csrf::Sent, ok.clone()).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = post(&router, "/_zeyn/data", ALL, Csrf::Sent, ok).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let got = sent
        .0
        .lock()
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    assert_eq!(
        got.conversation, "a",
        "the conversation comes from the server"
    );
    assert_eq!(got.text, "hi <b>");
    assert_eq!(got.mentions, vec![1, 3]);
    assert_eq!(got.attachments.len(), 1);
    for bad in [
        json!({ "text": "   " }),
        json!({ "text": "x".repeat(21) }),
        json!({ "text": "hi", "attachments": [file("../../etc/passwd")] }),
        json!({ "text": "hi", "attachments": (0..11).map(|_| file("0123456789abcdef0123456789abcdef")).collect::<Vec<_>>() }),
        json!({ "text": "hi", "mentions": (0..21).collect::<Vec<i64>>() }),
        json!({ "text": "hi", "conversation": "b" }),
    ] {
        let before = sent.0.lock().unwrap().len();
        let (status, _) = post(
            &router,
            "/_zeyn/data",
            ALL,
            Csrf::Sent,
            send_call(bad.clone()),
        )
        .await;
        if bad.get("conversation").is_some() {
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                sent.0
                    .lock()
                    .unwrap()
                    .last()
                    .unwrap()
                    .conversation,
                "a"
            );
        } else {
            assert_eq!(status, StatusCode::NOT_FOUND, "{bad}");
            assert_eq!(sent.0.lock().unwrap().len(), before);
        }
    }
    let (status, _) = post(&router, "/_zeyn/data", ALL, Csrf::Sent, json!({ "screen": "write", "params": null, "node": "0", "key": "send", "input": { "text": "hi" } })).await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the stack is not the composer"
    );
}

#[tokio::test]
async fn mention_searches_are_bounded_and_need_the_lookup_permission() {
    let router = app(Chat::seeded());
    let call = |text: &str| json!({ "screen": "write", "params": null, "node": "0.0", "key": "mentions", "input": { "text": text } });
    let (status, body) = post(&router, "/_zeyn/data", ALL, Csrf::Sent, call("pe")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        v["items"].as_array().unwrap().len(),
        zeyn::ui::MAX_MENTIONS as usize
    );
    assert_eq!(v["items"][0]["label"], "Person <1>");
    let (status, _) = post(
        &router,
        "/_zeyn/data",
        ALL,
        Csrf::Sent,
        call(&"x".repeat(51)),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(&router, "/_zeyn/data", "chat.write", Csrf::Sent, call("pe")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn sends_count_against_the_write_limit() {
    let router = app_sent(Chat::seeded(), Sent::default(), 2);
    for expected in [
        StatusCode::OK,
        StatusCode::OK,
        StatusCode::TOO_MANY_REQUESTS,
    ] {
        let (status, _) = post(
            &router,
            "/_zeyn/data",
            ALL,
            Csrf::Sent,
            send_call(json!({ "text": "hi" })),
        )
        .await;
        assert_eq!(status, expected);
    }
}
