//! ZeynSocket (feature `realtime`): the page socket that carries topics, typed events and
//! actions (see `docs/zeynsocket.md`), and WebSocket rooms that sync Y-CRDT documents (`yrs`, see
//! `docs/realtime.md`).
//!
//! Requests stay stateless by default; this is the one place where the server keeps state per
//! connection, so every piece of it has a limit: connections per user and per room, rooms in
//! memory, bytes per message and per document, and a bounded queue per client. A client that
//! cannot keep up is closed, never buffered without end. Rooms without clients are persisted
//! through [`DocStore`] and dropped after [`Limits::idle`].

use super::{AppState, Call, CallKind, Failure, cookie};
use crate::action::Error;
use crate::ctx::{BoxFuture, Ctx, Service};
use axum::Router;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;

#[cfg(feature = "redis")]
mod bus;
#[cfg(feature = "socket-client")]
mod client;
mod socket;
#[cfg(feature = "socket-client")]
pub use client::TestSocket;
use socket::Peer;
pub use socket::{Conn, Event, Target};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use yrs::updates::decoder::Decode;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update};
pub use zeyn_macros::Event;

const JS: &str = include_str!("../../assets/live.js");
const LOADER: &str = "/_zeyn/live.js";

/// Attribute of an element that refreshes on a topic; the quote keeps user data from faking it.
pub(super) const MARKER: &str = "data-zeyn-live=\"";
/// Attribute of a node that reacts to a socket event ([`crate::ui::OnEvents`]).
const EVENT_MARKER: &str = "data-zeyn-on=\"";

/// First byte of a binary frame: a Y update (v1 encoding). Both directions.
pub const FRAME_UPDATE: u8 = 1;

/// WebSocket close code sent to a client that fell behind (its queue was full).
pub const CLOSE_SLOW: u16 = 1013;
/// Close code for a frame the server cannot accept (bad update, write without permission).
pub const CLOSE_POLICY: u16 = 1008;
/// Close code when a message or the document is over its size limit.
pub const CLOSE_TOO_BIG: u16 = 1009;

/// Bounds of the realtime channel. Every number caps memory or work on the server.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Open sockets per signed in user (all rooms and topics).
    pub per_user: usize,
    /// Clients in one room.
    pub per_room: usize,
    /// Documents held in memory at once; more are refused with `503`.
    pub rooms: usize,
    /// Messages waiting for one client. When it is full the client is closed with 1013.
    pub queue: usize,
    /// Largest message a client may send.
    pub message_bytes: usize,
    /// Largest document, counted as the bytes of its stored state plus the updates applied since.
    pub doc_bytes: usize,
    /// How long a room without clients stays in memory before it is persisted and dropped.
    pub idle: Duration,
    /// Ping interval; a client silent for three intervals is closed.
    pub heartbeat: Duration,
    /// How often changed documents are persisted while clients are connected.
    pub persist_every: Duration,
    /// Groups one page socket may be in; [`Conn::join`] fails past it.
    pub groups: usize,
    /// Actions one page socket may call per second; more answer `429`.
    pub calls: u32,
    /// Frames of any kind one page socket may send per second; more close it with 1008.
    pub frames: u32,
    /// Largest event, as JSON; [`Target::emit`] fails past it.
    pub event_bytes: usize,
    /// Numbered messages a page socket keeps for a resume; a resume past them gets a resync.
    pub replay: usize,
    /// Bytes of those messages per page socket.
    pub replay_bytes: usize,
    /// How long a dropped page socket keeps its topics, groups and messages for a resume.
    pub detached: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            per_user: 8,
            per_room: 32,
            rooms: 256,
            queue: 32,
            message_bytes: 64 * 1024,
            doc_bytes: 1024 * 1024,
            idle: Duration::from_secs(30),
            heartbeat: Duration::from_secs(20),
            persist_every: Duration::from_secs(2),
            groups: 32,
            calls: 20,
            frames: 100,
            event_bytes: 16 * 1024,
            replay: 64,
            replay_bytes: 64 * 1024,
            detached: Duration::from_secs(60),
        }
    }
}

/// A room: one document of one kind, inside one tenant (empty without tenancy).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RoomKey {
    pub tenant: String,
    pub kind: String,
    pub doc: String,
}

/// Where documents live between sessions. `load` runs when a room opens, `save` while it has
/// changes and when it is evicted. The state is a Y update (v1) of the whole document.
pub trait DocStore: Send + Sync + 'static {
    fn load(&self, key: RoomKey) -> BoxFuture<crate::Result<Option<Vec<u8>>>>;
    fn save(&self, key: RoomKey, state: Vec<u8>) -> BoxFuture<crate::Result<()>>;
}

/// Keeps documents in process memory, for tests and demos. Unbounded: use a database in
/// production.
#[derive(Clone, Default)]
pub struct MemoryDocStore(Arc<Mutex<HashMap<RoomKey, Vec<u8>>>>);

impl MemoryDocStore {
    /// The stored state of a room.
    pub fn get(&self, key: &RoomKey) -> Option<Vec<u8>> {
        self.0.lock().ok()?.get(key).cloned()
    }
}

impl DocStore for MemoryDocStore {
    fn load(&self, key: RoomKey) -> BoxFuture<crate::Result<Option<Vec<u8>>>> {
        let state = self.get(&key);
        Box::pin(async move { Ok(state) })
    }

    fn save(&self, key: RoomKey, state: Vec<u8>) -> BoxFuture<crate::Result<()>> {
        if let Ok(mut m) = self.0.lock() {
            m.insert(key, state);
        }
        Box::pin(async { Ok(()) })
    }
}

struct DocKind {
    read: &'static str,
    write: &'static str,
    store: Arc<dyn DocStore>,
}

struct Config {
    kinds: HashMap<&'static str, DocKind>,
    topics: HashMap<&'static str, &'static str>,
    events: HashMap<&'static str, &'static str>,
    limits: Limits,
    origins: Vec<String>,
}

/// Settings of the live socket, given to `App::socket`.
///
/// ```
/// use zeyn::realtime::{MemoryDocStore, ZeynSocket};
/// let live = ZeynSocket::new()
///     .documents("diagram", "diagram.read", "diagram.write", MemoryDocStore::default())
///     .topic("orders", "sales.order.read");
/// let socket = live.socket();
/// assert_eq!(socket.stats().rooms, 0);
/// ```
pub struct ZeynSocket {
    socket: Socket,
    config: Config,
}

impl Default for ZeynSocket {
    fn default() -> Self {
        Self::new()
    }
}

impl ZeynSocket {
    pub fn new() -> Self {
        ZeynSocket {
            socket: Socket::default(),
            config: Config {
                kinds: HashMap::new(),
                topics: HashMap::new(),
                events: HashMap::new(),
                limits: Limits::default(),
                origins: Vec::new(),
            },
        }
    }

    /// Collaborative documents of `kind` at `/_zeyn/live/doc/{kind}/{doc}`. Joining needs `read`;
    /// sending changes needs `write`.
    pub fn documents(
        mut self,
        kind: &'static str,
        read: &'static str,
        write: &'static str,
        store: impl DocStore,
    ) -> Self {
        self.config.kinds.insert(
            kind,
            DocKind {
                read,
                write,
                store: Arc::new(store),
            },
        );
        self
    }

    /// A change topic screens may subscribe to (`ui::Live`), for users with `permission`.
    pub fn topic(mut self, name: &'static str, permission: &'static str) -> Self {
        self.config
            .topics
            .insert(name, permission);
        self
    }

    /// An event the server may send, for users with `permission`. A receiver without it never
    /// gets the event, whatever group it is in.
    pub fn event<E: Event>(mut self, permission: &'static str) -> Self {
        self.config
            .events
            .insert(E::NAME, permission);
        self
    }

    pub fn limits(mut self, limits: Limits) -> Self {
        self.config.limits = limits;
        self
    }

    /// Accepts sockets from another origin too (`https://app.example.com`), such as a public
    /// host name behind a proxy that rewrites `Host`. The page's own host is always accepted.
    pub fn allow_origin(mut self, origin: impl Into<String>) -> Self {
        self.config.origins.push(
            origin
                .into()
                .trim_end_matches('/')
                .to_string(),
        );
        self
    }

    /// The socket, to send events and notify topics from jobs or stores. `App::socket` also
    /// registers it as a service, so actions can take `socket: Socket`.
    pub fn socket(&self) -> Socket {
        self.socket.clone()
    }

    pub(crate) fn install(self) -> Socket {
        let _ = self.socket.0.config.set(self.config);
        self.socket
    }
}

type Outbox = mpsc::Sender<Message>;

struct Room {
    doc: Doc,
    clients: Mutex<Vec<(u64, Outbox)>>,
    idle_since: Mutex<Option<Instant>>,
    dirty: AtomicBool,
    bytes: AtomicUsize,
    store: Arc<dyn DocStore>,
}

/// A name inside a tenant: `(tenant, topic)` or `(tenant, group)`.
type TenantKey = (String, String);

/// Most users whose last typing signal the hub remembers; older entries make room first.
pub const MAX_TYPING_USERS: usize = 4096;

#[derive(Default)]
struct Inner {
    config: OnceLock<Config>,
    rooms: Mutex<HashMap<RoomKey, Arc<Room>>>,
    users: Mutex<HashMap<String, usize>>,
    topics: Mutex<HashMap<TenantKey, Vec<Arc<Peer>>>>,
    /// Open page sockets by id.
    conns: Mutex<HashMap<u64, Arc<Peer>>>,
    /// Connection ids by tenant and group.
    groups: Mutex<HashMap<TenantKey, Vec<u64>>>,
    /// Fallback transports opened with `POST` and waiting for their event stream.
    tickets: Mutex<HashMap<String, socket::sse::Ticket>>,
    /// Other instances of the app, through Redis.
    #[cfg(feature = "redis")]
    bus: OnceLock<bus::Bus>,
    next: AtomicU64,
    sweeping: AtomicBool,
    /// When each user (by tenant and user id) last sent a typing signal.
    typing: Mutex<HashMap<(String, String), Instant>>,
    /// Secret of the person keys typing signals carry, made when first needed.
    salt: OnceLock<u64>,
}

/// The live socket: open connections, their topics and groups, and document rooms. Actions take
/// it as a service to send events and notify topics.
#[derive(Clone, Default)]
pub struct Socket(Arc<Inner>);

impl Service for Socket {}

/// What the socket holds right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SocketStats {
    pub rooms: usize,
    /// Clients of document rooms.
    pub clients: usize,
    /// Topic subscriptions of page sockets.
    pub subscribers: usize,
    /// Open page sockets.
    pub connections: usize,
    /// Groups with at least one connection.
    pub groups: usize,
    /// Sum of the documents' sizes as counted against [`Limits::doc_bytes`].
    pub doc_bytes: usize,
}

impl Socket {
    /// Tells every screen of the caller's tenant that subscribed to `topic` to refresh.
    pub fn notify(&self, ctx: &Ctx, topic: &str) {
        self.notify_tenant(ctx.tenant().map(|t| t.id()), topic);
    }

    /// Same as [`Socket::notify`] for a tenant given by id (`None` without tenancy). A
    /// subscriber whose queue is full is told to resync instead.
    pub fn notify_tenant(&self, tenant: Option<&str>, topic: &str) {
        self.notify_local(tenant, topic);
        #[cfg(feature = "redis")]
        self.publish(
            tenant.unwrap_or(""),
            bus::What::Topic {
                topic: topic.to_string(),
            },
        );
    }

    /// Notifies the subscribers of this instance only.
    fn notify_local(&self, tenant: Option<&str>, topic: &str) {
        let key = (tenant.unwrap_or("").to_string(), topic.to_string());
        if let Ok(mut topics) = self.0.topics.lock()
            && let Some(subs) = topics.get_mut(&key)
        {
            subs.retain(|p| p.notify(topic));
        }
    }

    /// The key a typing signal carries for a user: a keyed hash that only this hub can make, so
    /// pages can tell people apart without seeing their user ids.
    pub fn person_key(&self, tenant: &str, user: &str) -> String {
        let salt = *self.0.salt.get_or_init(|| {
            let mut bytes = [0u8; 8];
            getrandom::getrandom(&mut bytes).expect("the operating system random source must work");
            u64::from_le_bytes(bytes)
        });
        let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ salt;
        for b in tenant
            .bytes()
            .chain([0])
            .chain(user.bytes())
        {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h ^= salt.rotate_left(29);
        h = h.wrapping_mul(0x0100_0000_01b3);
        format!("{h:016x}")
    }

    /// Whether a user may send a typing signal now: at most one every
    /// [`TYPING_EVERY_SECONDS`](crate::ui::TYPING_EVERY_SECONDS), remembered for at most
    /// [`MAX_TYPING_USERS`] users.
    fn typing_allowed(&self, tenant: &str, user: &str) -> bool {
        let every = Duration::from_secs(crate::ui::TYPING_EVERY_SECONDS);
        let Ok(mut map) = self.0.typing.lock() else {
            return false;
        };
        let key = (tenant.to_string(), user.to_string());
        let now = Instant::now();
        if map
            .get(&key)
            .is_some_and(|at| now.duration_since(*at) < every)
        {
            return false;
        }
        if map.len() >= MAX_TYPING_USERS && !map.contains_key(&key) {
            map.retain(|_, at| now.duration_since(*at) < every);
            if map.len() >= MAX_TYPING_USERS {
                return false;
            }
        }
        map.insert(key, now);
        true
    }

    /// Passes a typing signal of `user` in conversation `room` to the other users subscribed to
    /// `topic` in the tenant. Signals faster than one every three seconds per user are dropped.
    fn typing(&self, tenant: &str, topic: &str, user: &str, room: &str) {
        if !self.typing_allowed(tenant, user) {
            return;
        }
        self.typing_local(tenant, topic, user, room);
        #[cfg(feature = "redis")]
        self.publish(
            tenant,
            bus::What::Typing {
                topic: topic.to_string(),
                user: user.to_string(),
                room: room.to_string(),
            },
        );
    }

    /// Passes a typing signal to the subscribers of this instance only.
    fn typing_local(&self, tenant: &str, topic: &str, user: &str, room: &str) {
        let text = serde_json::json!({
            "t": topic,
            "typing": { "room": room, "who": self.person_key(tenant, user) },
        })
        .to_string();
        if let Ok(topics) = self.0.topics.lock()
            && let Some(subs) = topics.get(&(tenant.to_string(), topic.to_string()))
        {
            for p in subs {
                if &*p.user != user {
                    p.signal(text.clone());
                }
            }
        }
    }

    pub fn stats(&self) -> SocketStats {
        let rooms = self
            .0
            .rooms
            .lock()
            .map(|r| r.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        SocketStats {
            connections: self
                .0
                .conns
                .lock()
                .map(|c| c.len())
                .unwrap_or(0),
            groups: self
                .0
                .groups
                .lock()
                .map(|g| g.len())
                .unwrap_or(0),
            rooms: rooms.len(),
            clients: rooms
                .iter()
                .map(|r| {
                    r.clients
                        .lock()
                        .map(|c| c.len())
                        .unwrap_or(0)
                })
                .sum(),
            subscribers: self
                .0
                .topics
                .lock()
                .map(|t| t.values().map(Vec::len).sum())
                .unwrap_or(0),
            doc_bytes: rooms
                .iter()
                .map(|r| r.bytes.load(Ordering::Relaxed))
                .sum(),
        }
    }

    /// Persists every changed document now; rooms stay open. Also runs on shutdown paths and in
    /// tests.
    pub async fn flush(&self) {
        let rooms: Vec<(RoomKey, Arc<Room>)> = self
            .0
            .rooms
            .lock()
            .map(|r| {
                r.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();
        for (key, room) in rooms {
            persist(&key, &room).await;
        }
    }

    fn config(&self) -> Option<&Config> {
        self.0.config.get()
    }

    fn take_user(&self, user: &str, max: usize) -> bool {
        let Ok(mut users) = self.0.users.lock() else {
            return false;
        };
        let n = users
            .entry(user.to_string())
            .or_default();
        if *n >= max {
            return false;
        }
        *n += 1;
        true
    }

    fn release_user(&self, user: &str) {
        if let Ok(mut users) = self.0.users.lock()
            && let Some(n) = users.get_mut(user)
        {
            *n -= 1;
            if *n == 0 {
                users.remove(user);
            }
        }
    }

    /// The room, loading its document when it is not in memory. `None` when the room limit is
    /// reached or the store fails.
    async fn open(&self, key: &RoomKey, kind: &DocKind, limits: &Limits) -> Option<Arc<Room>> {
        if let Some(room) = self.0.rooms.lock().ok()?.get(key) {
            return Some(room.clone());
        }
        if self.0.rooms.lock().ok()?.len() >= limits.rooms {
            return None;
        }
        let state = kind
            .store
            .load(key.clone())
            .await
            .ok()?;
        let doc = Doc::new();
        let mut bytes = 0;
        if let Some(state) = state {
            bytes = state.len();
            let update = Update::decode_v1(&state).ok()?;
            doc.transact_mut()
                .apply_update(update)
                .ok()?;
        }
        let room = Arc::new(Room {
            doc,
            clients: Mutex::new(Vec::new()),
            idle_since: Mutex::new(None),
            dirty: AtomicBool::new(false),
            bytes: AtomicUsize::new(bytes),
            store: kind.store.clone(),
        });
        let mut rooms = self.0.rooms.lock().ok()?;
        if rooms.len() >= limits.rooms && !rooms.contains_key(key) {
            return None;
        }
        Some(
            rooms
                .entry(key.clone())
                .or_insert(room)
                .clone(),
        )
    }

    /// Persists changed rooms and evicts idle ones, every `persist_every`, for the life of the
    /// process. Started by the first connection.
    fn start_sweeper(&self, limits: Limits) {
        if self
            .0
            .sweeping
            .swap(true, Ordering::AcqRel)
        {
            return;
        }
        let hub = Arc::downgrade(&self.0);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(limits.persist_every);
            loop {
                tick.tick().await;
                let Some(inner) = hub.upgrade() else { return };
                Socket(inner).sweep(limits.idle).await;
            }
        });
    }

    async fn sweep(&self, idle: Duration) {
        let rooms: Vec<(RoomKey, Arc<Room>)> = self
            .0
            .rooms
            .lock()
            .map(|r| {
                r.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            })
            .unwrap_or_default();
        for (key, room) in rooms {
            persist(&key, &room).await;
            let expired = room
                .idle_since
                .lock()
                .ok()
                .and_then(|s| *s)
                .is_some_and(|since| since.elapsed() >= idle);
            if expired && let Ok(mut map) = self.0.rooms.lock() {
                let empty = room
                    .clients
                    .lock()
                    .map(|c| c.is_empty())
                    .unwrap_or(true);
                if empty && !room.dirty.load(Ordering::Acquire) {
                    map.remove(&key);
                }
            }
        }
    }
}

/// Saves a room's document when it changed since the last save.
async fn persist(key: &RoomKey, room: &Room) {
    if !room.dirty.swap(false, Ordering::AcqRel) {
        return;
    }
    let state = room
        .doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    room.bytes
        .store(state.len(), Ordering::Relaxed);
    if let Err(e) = room
        .store
        .save(key.clone(), state)
        .await
    {
        room.dirty
            .store(true, Ordering::Release);
        tracing::warn!("realtime: saving {}/{} failed: {e}", key.kind, key.doc);
    }
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route(
            LOADER,
            get(|| async { super::asset(JS, "text/javascript; charset=utf-8") }),
        )
        .route("/_zeyn/live/doc/{kind}/{doc}", get(doc_socket))
        .route("/_zeyn/socket", get(socket::page_socket))
        .route("/_zeyn/socket/open", axum::routing::post(socket::sse::open))
        .route("/_zeyn/socket/events", get(socket::sse::events))
        .route("/_zeyn/socket/send", axum::routing::post(socket::sse::send))
}

pub(super) fn is_asset(path: &str) -> bool {
    path == LOADER
}

/// Adds the topic script to a full page whose screen subscribes to a topic.
pub(super) fn head(st: &AppState, screen: &str, page: &mut String) {
    if holds(st, screen) {
        let tag = format!(
            r#"<script type="module" src="{}{LOADER}{}"></script></head>"#,
            crate::render::escape(&st.prefix),
            super::ops::asset_version()
        );
        *page = page.replacen("</head>", &tag, 1);
    }
}

/// Whether a screen loaded by `nav.js` subscribes to a topic (`x-zeyn-live: 1`).
pub(super) fn holds(st: &AppState, screen: &str) -> bool {
    st.realtime.is_some() && (screen.contains(MARKER) || screen.contains(EVENT_MARKER))
}

/// Document ids: 1 to 64 letters, digits, `-`, `_` or `.`.
fn valid_doc(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// `Origin` must name the host the request came to, or an origin the app allows. Browsers always
/// send it on WebSocket handshakes, so a missing one is refused.
fn origin_allowed(headers: &HeaderMap, allowed: &[String]) -> bool {
    let Some(origin) = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    if allowed.iter().any(|a| a == origin) {
        return true;
    }
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let bare = origin
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or("");
    !host.is_empty() && bare.eq_ignore_ascii_case(host)
}

/// The subprotocol the browser runtime asks for and the server answers with.
const PROTOCOL: &str = "zeyn";

/// The CSRF token of a handshake. Browsers cannot set headers on WebSockets, so the runtime
/// offers it as a subprotocol next to [`PROTOCOL`] (`Sec-WebSocket-Protocol: zeyn, csrf.TOKEN`),
/// which keeps it out of the address, server logs and history. The answer names only `zeyn`.
fn csrf_protocol(headers: &HeaderMap) -> &str {
    headers
        .get_all(header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .find_map(|p| p.trim().strip_prefix("csrf."))
        .unwrap_or("")
}

/// The checks of every handshake: the channel is on, origin, CSRF token (see
/// [`csrf_protocol`]), a signed in user and the per user limit. Returns the context and the user
/// key, which the caller must release.
#[allow(
    clippy::result_large_err,
    reason = "the error is the HTTP response, returned to the client at once and never stored; boxing it would allocate on every refusal"
)]
async fn admit(st: &AppState, headers: &HeaderMap) -> Result<(Ctx, Socket, String), Response> {
    admit_with(st, headers, false).await
}

/// [`admit`] for the `POST` that opens the fallback transport: the CSRF token comes in the
/// `x-zeyn-csrf` header like any `POST`, and the browser's same origin rules stand in for the
/// `Origin` check.
#[allow(
    clippy::result_large_err,
    reason = "the error is the HTTP response, returned to the client at once and never stored; boxing it would allocate on every refusal"
)]
async fn admit_post(st: &AppState, headers: &HeaderMap) -> Result<(Ctx, Socket, String), Response> {
    admit_with(st, headers, true).await
}

#[allow(
    clippy::result_large_err,
    reason = "the error is the HTTP response, returned to the client at once and never stored; boxing it would allocate on every refusal"
)]
async fn admit_with(
    st: &AppState,
    headers: &HeaderMap,
    post: bool,
) -> Result<(Ctx, Socket, String), Response> {
    let ctx = st.licensed(st.ctx(headers)).await;
    let Some(hub) = st.realtime.clone() else {
        return Err(st.error_response(&ctx, Failure::App(Error::NotFound)));
    };
    let Some(config) = hub.config() else {
        return Err(st.error_response(&ctx, Failure::App(Error::NotFound)));
    };
    let checked = if post {
        st.check_csrf(headers).is_ok()
    } else {
        let csrf = csrf_protocol(headers);
        cookie(headers, "zeyn_csrf").is_some_and(|c| super::csrf_matches(c, csrf))
            && origin_allowed(headers, &config.origins)
    };
    if !checked {
        return Err(st.error_response(
            &ctx,
            Failure::App(Error::Denied(crate::i18n::Text::key("zeyn.error.csrf"))),
        ));
    }
    let Some(user) = ctx.identity().user.clone() else {
        return Err(st.error_response(&ctx, Failure::App(Error::Forbidden)));
    };
    let user = format!(
        "{}/{user}",
        ctx.tenant()
            .map(|t| t.id())
            .unwrap_or("")
    );
    if !hub.take_user(&user, config.limits.per_user) {
        let mut r = st.error_response(
            &ctx,
            Failure::App(Error::Denied(crate::i18n::Text::key(
                "zeyn.error.rate_limited",
            ))),
        );
        *r.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        return Err(r);
    }
    Ok((ctx, hub, user))
}

fn busy() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::RETRY_AFTER, "5")],
        "busy",
    )
        .into_response()
}

async fn doc_socket(
    State(st): State<Arc<AppState>>,
    Path((kind, doc)): Path<(String, String)>,
    headers: HeaderMap,
    ws: Result<WebSocketUpgrade, axum::extract::ws::rejection::WebSocketUpgradeRejection>,
) -> Response {
    let (ctx, hub, user) = match admit(&st, &headers).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let config = hub.config().expect("checked in admit");
    let refuse = |hub: &Socket, failure: Failure| {
        hub.release_user(&user);
        st.error_response(&ctx, failure)
    };
    let Some((&kind_name, kind_def)) = config
        .kinds
        .get_key_value(kind.as_str())
        .filter(|_| valid_doc(&doc))
    else {
        return refuse(&hub, Failure::App(Error::NotFound));
    };
    if !ctx
        .identity()
        .can(&crate::action::Access::Permission(kind_def.read))
    {
        return refuse(&hub, Failure::App(Error::Forbidden));
    }
    let call = Call {
        kind: CallKind::Live,
        screen: kind_name,
        action: None,
        via_agent: ctx.via_agent(),
    };
    if let Err(f) = st.admit_call(&ctx, &call) {
        return refuse(&hub, f);
    }
    let ws = match ws {
        Ok(ws) => ws,
        Err(e) => {
            hub.release_user(&user);
            return e.into_response();
        }
    };
    let limits = config.limits;
    let key = RoomKey {
        tenant: ctx
            .tenant()
            .map(|t| t.id().to_string())
            .unwrap_or_default(),
        kind,
        doc,
    };
    let Some(room) = hub.open(&key, kind_def, &limits).await else {
        hub.release_user(&user);
        return busy();
    };
    let (tx, rx) = mpsc::channel(limits.queue);
    let id = hub
        .0
        .next
        .fetch_add(1, Ordering::Relaxed);
    {
        let rooms = hub.0.rooms.lock();
        let current = rooms
            .as_ref()
            .ok()
            .and_then(|r| r.get(&key))
            .is_some_and(|r| Arc::ptr_eq(r, &room));
        let clients = room.clients.lock();
        let (true, Ok(mut clients)) = (current, clients) else {
            hub.release_user(&user);
            return busy();
        };
        if clients.len() >= limits.per_room {
            hub.release_user(&user);
            return busy();
        }
        clients.push((id, tx));
        if let Ok(mut idle) = room.idle_since.lock() {
            *idle = None;
        }
    }
    hub.start_sweeper(limits);
    let can_write = ctx
        .identity()
        .can(&crate::action::Access::Permission(kind_def.write));
    ws.protocols([PROTOCOL])
        .max_message_size(limits.message_bytes)
        .on_upgrade(move |socket| async move {
            run_doc(socket, &room, id, rx, can_write, &limits).await;
            if let Ok(mut clients) = room.clients.lock() {
                clients.retain(|(c, _)| *c != id);
                if clients.is_empty()
                    && let Ok(mut idle) = room.idle_since.lock()
                {
                    *idle = Some(Instant::now());
                }
            }
            hub.release_user(&user);
        })
}

fn frame(update: &[u8]) -> Message {
    let mut out = Vec::with_capacity(update.len() + 1);
    out.push(FRAME_UPDATE);
    out.extend_from_slice(update);
    Message::Binary(out.into())
}

fn close(code: u16, reason: &'static str) -> Message {
    Message::Close(Some(CloseFrame {
        code,
        reason: reason.into(),
    }))
}

/// One document client: sends the whole document, then relays updates both ways until the
/// client leaves, falls silent, sends something invalid or falls behind.
async fn run_doc(
    socket: WebSocket,
    room: &Room,
    id: u64,
    mut rx: mpsc::Receiver<Message>,
    can_write: bool,
    limits: &Limits,
) {
    let (mut sink, mut stream) = socket.split();
    let state = room
        .doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    if sink.send(frame(&state)).await.is_err() {
        return;
    }
    drop(state);
    let mut beat = tokio::time::interval(limits.heartbeat);
    beat.tick().await;
    let mut heard = Instant::now();
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(m) => if sink.send(m).await.is_err() { return },
                None => {
                    let _ = sink.send(close(CLOSE_SLOW, "too slow")).await;
                    return;
                }
            },
            incoming = stream.next() => {
                heard = Instant::now();
                match incoming {
                    Some(Ok(Message::Binary(data))) => {
                        if let Err(code) = apply(room, id, &data, can_write, limits) {
                            let _ = sink.send(close(code, "refused")).await;
                            return;
                        }
                    }
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                    Some(Ok(Message::Text(_))) => {
                        let _ = sink.send(close(CLOSE_POLICY, "binary only")).await;
                        return;
                    }
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return,
                }
            }
            _ = beat.tick() => {
                if heard.elapsed() > limits.heartbeat * 3 || sink.send(Message::Ping(Default::default())).await.is_err() {
                    return;
                }
            }
        }
    }
}

/// Applies a client's update and passes it to the other clients. A client whose queue is full
/// is dropped from the room; its socket then closes with 1013.
fn apply(room: &Room, from: u64, data: &[u8], can_write: bool, limits: &Limits) -> Result<(), u16> {
    let (Some(&FRAME_UPDATE), true) = (data.first(), can_write) else {
        return Err(CLOSE_POLICY);
    };
    let body = &data[1..];
    if room.bytes.load(Ordering::Relaxed) + body.len() > limits.doc_bytes {
        return Err(CLOSE_TOO_BIG);
    }
    let update = Update::decode_v1(body).map_err(|_| CLOSE_POLICY)?;
    room.doc
        .transact_mut()
        .apply_update(update)
        .map_err(|_| CLOSE_POLICY)?;
    room.bytes
        .fetch_add(body.len(), Ordering::Relaxed);
    room.dirty
        .store(true, Ordering::Release);
    let message = Message::Binary(axum::body::Bytes::copy_from_slice(data));
    if let Ok(mut clients) = room.clients.lock() {
        clients.retain(|(id, tx)| *id == from || tx.try_send(message.clone()).is_ok());
    }
    Ok(())
}

/// Reads a typing signal a browser sent on a topic socket: `{"t":"typing","topic":..,"room":..}`
/// on a topic this socket subscribed to, the room at most 200 characters without control
/// characters. Anything else is ignored.
fn typing_signal(text: &str, topics: &[&'static str]) -> Option<(&'static str, String)> {
    #[derive(Deserialize)]
    struct Signal {
        t: String,
        topic: String,
        room: String,
    }
    let s: Signal = serde_json::from_str(text).ok()?;
    let topic = topics.iter().find(|t| **t == s.topic)?;
    let fits = s.t == "typing"
        && s.room.chars().count() <= crate::ui::MAX_CONVERSATION
        && !s.room.chars().any(char::is_control);
    fits.then_some((topic, s.room))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_must_match_the_host_or_the_allow_list() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "app.test:3000".parse().unwrap());
        assert!(!origin_allowed(&h, &[]));
        h.insert(header::ORIGIN, "http://app.test:3000".parse().unwrap());
        assert!(origin_allowed(&h, &[]));
        h.insert(header::ORIGIN, "https://evil.test".parse().unwrap());
        assert!(!origin_allowed(&h, &[]));
        assert!(origin_allowed(&h, &["https://evil.test".into()]));
    }

    #[test]
    fn a_client_with_a_full_queue_is_dropped_from_the_room() {
        let room = Room {
            doc: Doc::new(),
            clients: Mutex::new(Vec::new()),
            idle_since: Mutex::new(None),
            dirty: AtomicBool::new(false),
            bytes: AtomicUsize::new(0),
            store: Arc::new(MemoryDocStore::default()),
        };
        let (slow, _slow_rx) = mpsc::channel(1);
        let (fast, mut fast_rx) = mpsc::channel(8);
        slow.try_send(Message::Ping(Default::default()))
            .unwrap();
        room.clients
            .lock()
            .unwrap()
            .extend([(1, slow), (2, fast)]);
        let other = Doc::new();
        let body = other.get_or_insert_text("t");
        yrs::Text::insert(&body, &mut other.transact_mut(), 0, "hi");
        let update = other
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        let frame = [vec![FRAME_UPDATE], update].concat();
        assert!(apply(&room, 3, &frame, true, &Limits::default()).is_ok());
        assert_eq!(
            room.clients
                .lock()
                .unwrap()
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            [2]
        );
        assert!(fast_rx.try_recv().is_ok());
        assert!(room.dirty.load(Ordering::Acquire));
        assert_eq!(
            apply(&room, 3, &frame, false, &Limits::default()),
            Err(CLOSE_POLICY)
        );
        assert_eq!(
            apply(
                &room,
                3,
                &[FRAME_UPDATE, 0xff, 0xff],
                true,
                &Limits::default()
            ),
            Err(CLOSE_POLICY)
        );
    }

    #[test]
    fn document_ids_are_plain() {
        assert!(valid_doc("main") && valid_doc("order-7.v2"));
        assert!(!valid_doc("") && !valid_doc("a/b") && !valid_doc(&"x".repeat(65)));
    }
}
