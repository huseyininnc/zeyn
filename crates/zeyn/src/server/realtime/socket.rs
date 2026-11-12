//! The page socket at `/_zeyn/socket`: one connection per page that carries topic
//! notifications, typed events for a user, a group or a tenant, and actions called by name. See
//! `docs/zeynsocket.md` for the wire protocol.

use super::{CLOSE_POLICY, Limits, Outbox, PROTOCOL, Socket, admit, typing_signal};

pub(super) mod sse;
use crate::action::Error;
use crate::ctx::{Ctx, Identity, Inject};
use crate::server::{AppState, Call, CallKind, Failure};
use axum::extract::State;
use axum::extract::ws::{CloseFrame, Message, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::Value as Json;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;

/// A value the live socket sends to browsers: a `Serialize` type with a stable name. Derive it
/// with `#[derive(Event)]` and `#[event(name = "area.name")]`, and declare it with
/// [`ZeynSocket::event`](super::ZeynSocket::event) and the permission a receiver needs.
///
/// ```compile_fail
/// use zeyn::realtime::{Socket, Event};
/// #[derive(serde::Serialize)]
/// struct Plain { text: String }
/// fn send(socket: &Socket, ctx: &zeyn::Ctx) {
///     socket.to_tenant(ctx).emit(&Plain { text: "hi".into() });
/// }
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a socket event",
    label = "this type cannot be sent over the socket",
    note = "add `#[derive(Event)]` and `#[event(name = \"area.name\")]` to `{Self}`, and declare it with `ZeynSocket::new().event::<{Self}>(\"permission\")`"
)]
pub trait Event: serde::Serialize {
    /// The name browsers listen for (`zeyn.socket.on(name, ...)`).
    const NAME: &'static str;
}

/// One page socket: who it belongs to, its outgoing queue, the numbered messages it may still
/// replay, its topics and its groups. It outlives a dropped connection for
/// [`Limits::detached`], so the browser can resume where it was.
pub(super) struct Peer {
    pub(super) id: u64,
    pub(super) tenant: String,
    pub(super) user: Arc<str>,
    identity: Identity,
    /// The queue of the connection the peer is attached to; replaced when a dropped one resumes.
    tx: Mutex<Outbox>,
    replay: Mutex<Replay>,
    /// Set when a message did not fit the queue; the connection then tells the browser to resync.
    lost: AtomicBool,
    attached: AtomicBool,
    /// Bumped on every attach, so the sweep of an earlier drop leaves a resumed peer alone.
    epoch: AtomicU64,
    /// The secret half of the resume token.
    secret: String,
    groups: Mutex<Vec<String>>,
    topics: Mutex<Vec<&'static str>>,
    /// Frames and calls in the current second, for [`Limits::frames`] and [`Limits::calls`].
    rates: Mutex<Rates>,
}

/// Counters of one second.
struct Rates {
    since: Instant,
    frames: u32,
    calls: u32,
}

impl Peer {
    /// Counts a frame; `false` when the connection is over [`Limits::frames`] this second.
    fn take_frame(&self, max: u32) -> bool {
        let Ok(mut r) = self.rates.lock() else {
            return false;
        };
        if r.since.elapsed().as_secs() >= 1 {
            *r = Rates {
                since: Instant::now(),
                frames: 0,
                calls: 0,
            };
        }
        r.frames += 1;
        r.frames <= max
    }

    /// Counts a call; `false` when the connection is over [`Limits::calls`] this second.
    fn take_call(&self, max: u32) -> bool {
        self.rates
            .lock()
            .map(|mut r| {
                r.calls += 1;
                r.calls <= max
            })
            .unwrap_or(false)
    }
}

/// Numbered messages since the last acknowledgement, oldest first, capped by count and bytes.
struct Replay {
    seq: u64,
    held: VecDeque<(u64, Arc<str>)>,
    bytes: usize,
    max: usize,
    max_bytes: usize,
}

impl Replay {
    fn push(&mut self, seq: u64, text: Arc<str>) {
        self.bytes += text.len();
        self.held.push_back((seq, text));
        while self.held.len() > self.max || self.bytes > self.max_bytes {
            match self.held.pop_front() {
                Some((_, t)) => self.bytes -= t.len(),
                None => break,
            }
        }
    }

    /// Drops what the browser confirmed it received.
    fn ack(&mut self, seq: u64) {
        while self
            .held
            .front()
            .is_some_and(|(s, _)| *s <= seq)
        {
            if let Some((_, t)) = self.held.pop_front() {
                self.bytes -= t.len();
            }
        }
    }

    /// The messages after `seq`, or `None` when some of them are no longer held.
    fn since(&self, seq: u64) -> Option<Vec<Arc<str>>> {
        if seq >= self.seq {
            return Some(Vec::new());
        }
        let first = self.held.front()?.0;
        (first <= seq + 1).then(|| {
            self.held
                .iter()
                .filter(|(s, _)| *s > seq)
                .map(|(_, t)| t.clone())
                .collect()
        })
    }
}

impl Peer {
    /// Numbers a message, keeps it for a resume and queues it while a connection is attached.
    /// `false` only when the peer is poisoned.
    fn send_numbered(&self, body: impl FnOnce(u64) -> String) -> bool {
        let Ok(mut r) = self.replay.lock() else {
            return false;
        };
        r.seq += 1;
        let seq = r.seq;
        let text: Arc<str> = body(seq).into();
        r.push(seq, text.clone());
        if self.attached.load(Ordering::Acquire) {
            self.send(text.to_string());
        }
        true
    }

    fn send(&self, text: String) -> bool {
        let Ok(tx) = self.tx.lock() else {
            return false;
        };
        match tx.try_send(Message::Text(text.into())) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.lost.store(true, Ordering::Relaxed);
                true
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
    }

    /// The token the browser sends back to resume this peer: its id and its secret.
    fn token(&self) -> String {
        format!("{}.{}", self.id, self.secret)
    }

    pub(super) fn notify(&self, topic: &str) -> bool {
        let topic = serde_json::to_string(topic).unwrap_or_default();
        self.send_numbered(|s| format!(r#"{{"s":{s},"t":{topic}}}"#))
    }

    pub(super) fn signal(&self, text: String) -> bool {
        self.send(text)
    }
}

/// Who an event goes to. Made by [`Socket::to_user`], [`Socket::to_group`],
/// [`Socket::to_tenant`] and [`Socket::to_tenant_id`]; always inside one tenant.
pub struct Target<'a> {
    socket: &'a Socket,
    tenant: String,
    to: To,
}

enum To {
    User(String),
    Group(String),
    Tenant,
}

impl Socket {
    /// Every open page of one user in the caller's tenant.
    pub fn to_user(&self, ctx: &Ctx, user: impl Into<String>) -> Target<'_> {
        self.target(ctx, To::User(user.into()))
    }

    /// Every connection in a group of the caller's tenant. Connections join groups only in
    /// actions, with [`Conn::join`].
    pub fn to_group(&self, ctx: &Ctx, group: impl Into<String>) -> Target<'_> {
        self.target(ctx, To::Group(group.into()))
    }

    /// Every connection of the caller's tenant.
    pub fn to_tenant(&self, ctx: &Ctx) -> Target<'_> {
        self.target(ctx, To::Tenant)
    }

    /// Every connection of a tenant given by id (`None` without tenancy), for jobs and stores
    /// that run without a request.
    pub fn to_tenant_id(&self, tenant: Option<&str>) -> Target<'_> {
        Target {
            socket: self,
            tenant: tenant.unwrap_or("").to_string(),
            to: To::Tenant,
        }
    }

    fn target(&self, ctx: &Ctx, to: To) -> Target<'_> {
        Target {
            socket: self,
            tenant: ctx
                .tenant()
                .map(|t| t.id().to_string())
                .unwrap_or_default(),
            to,
        }
    }

    /// The connections a target names, without closed ones.
    fn peers(&self, tenant: &str, to: &To) -> Vec<Arc<Peer>> {
        let Ok(conns) = self.0.conns.lock() else {
            return Vec::new();
        };
        match to {
            To::User(user) => conns
                .values()
                .filter(|p| p.tenant == tenant && &*p.user == user)
                .cloned()
                .collect(),
            To::Tenant => conns
                .values()
                .filter(|p| p.tenant == tenant)
                .cloned()
                .collect(),
            To::Group(group) => self
                .0
                .groups
                .lock()
                .ok()
                .and_then(|g| {
                    g.get(&(tenant.to_string(), group.clone()))
                        .cloned()
                })
                .unwrap_or_default()
                .iter()
                .filter_map(|id| conns.get(id).cloned())
                .collect(),
        }
    }
}

impl Target<'_> {
    /// Sends an event to every connection of the target whose user holds the permission the
    /// event was declared with, and returns how many got it. The event is serialized once. An
    /// event not declared with `ZeynSocket::event`, or larger than [`Limits::event_bytes`], is
    /// an error.
    pub fn emit<E: Event>(&self, event: &E) -> crate::Result<usize> {
        let Some(config) = self.socket.config() else {
            return Ok(0);
        };
        let Some(permission) = config.events.get(E::NAME) else {
            return Err(Error::Internal(format!(
                "the socket event `{}` is not declared: add `.event::<{}>(\"permission\")` to `ZeynSocket::new()`",
                E::NAME,
                std::any::type_name::<E>()
            )));
        };
        let data = serde_json::to_string(event)
            .map_err(|e| Error::Internal(format!("socket event `{}`: {e}", E::NAME)))?;
        if data.len() > config.limits.event_bytes {
            return Err(Error::Internal(format!(
                "the socket event `{}` has {} bytes, more than the {} of `Limits::event_bytes`; send an id and let the browser load the rest",
                E::NAME,
                data.len(),
                config.limits.event_bytes
            )));
        }
        let name = serde_json::to_string(E::NAME).unwrap_or_default();
        let access = crate::action::Access::Permission(permission);
        let mut sent = 0;
        for peer in self
            .socket
            .peers(&self.tenant, &self.to)
        {
            if peer.identity.can(&access)
                && peer.send_numbered(|s| format!(r#"{{"s":{s},"e":{name},"d":{data}}}"#))
            {
                sent += 1;
            }
        }
        Ok(sent)
    }
}

/// The connection an action was called over. Actions take it as an argument to join or leave
/// groups; called over plain HTTP, an action that takes it fails with a validation error.
#[derive(Clone)]
pub struct Conn {
    socket: Socket,
    peer: Arc<Peer>,
}

impl Conn {
    /// Adds the connection to a group of its tenant. The action that calls it is where the app
    /// checks that the user may be in the group. Fails past [`Limits::groups`] groups.
    pub fn join(&self, group: impl Into<String>) -> crate::Result<()> {
        let group = group.into();
        let max = self
            .socket
            .config()
            .map_or(0, |c| c.limits.groups);
        {
            let mut mine = self
                .peer
                .groups
                .lock()
                .map_err(|_| Error::Internal("socket groups poisoned".into()))?;
            if mine.contains(&group) {
                return Ok(());
            }
            if mine.len() >= max {
                return Err(Error::Denied(crate::i18n::Text::key(
                    "zeyn.error.rate_limited",
                )));
            }
            mine.push(group.clone());
        }
        if let Ok(mut groups) = self.socket.0.groups.lock() {
            groups
                .entry((self.peer.tenant.clone(), group))
                .or_default()
                .push(self.peer.id);
        }
        Ok(())
    }

    /// Removes the connection from a group.
    pub fn leave(&self, group: &str) {
        if let Ok(mut mine) = self.peer.groups.lock() {
            mine.retain(|g| g != group);
        }
        self.socket
            .drop_from_group(&self.peer.tenant, group, self.peer.id);
    }

    /// The groups the connection is in.
    pub fn groups(&self) -> Vec<String> {
        self.peer
            .groups
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}

impl Inject for Conn {
    fn inject(ctx: &Ctx) -> crate::Result<Self> {
        ctx.conn::<Conn>()
            .cloned()
            .ok_or_else(|| {
                let mut errors = crate::model::ValidationErrors::default();
                errors.add_general(crate::i18n::Text::key("zeyn.error.socket_only"));
                Error::Validation(errors)
            })
    }

    fn dep() -> Option<crate::ctx::Dep> {
        None
    }
}

impl Socket {
    fn drop_from_group(&self, tenant: &str, group: &str, id: u64) {
        if let Ok(mut groups) = self.0.groups.lock() {
            let key = (tenant.to_string(), group.to_string());
            if let Some(ids) = groups.get_mut(&key) {
                ids.retain(|i| *i != id);
                if ids.is_empty() {
                    groups.remove(&key);
                }
            }
        }
    }

    /// Forgets a closed connection: its topics, groups and entry.
    fn forget(&self, peer: &Peer) {
        let topics = peer
            .topics
            .lock()
            .map(|t| t.clone())
            .unwrap_or_default();
        if let Ok(mut map) = self.0.topics.lock() {
            for t in &topics {
                let key = (peer.tenant.clone(), t.to_string());
                if let Some(subs) = map.get_mut(&key) {
                    subs.retain(|p| p.id != peer.id);
                    if subs.is_empty() {
                        map.remove(&key);
                    }
                }
            }
        }
        let groups = peer
            .groups
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        for g in &groups {
            self.drop_from_group(&peer.tenant, g, peer.id);
        }
        if let Ok(mut conns) = self.0.conns.lock() {
            conns.remove(&peer.id);
        }
    }
}

/// A frame from the browser. Unknown fields are ignored; a frame that is none of these closes the
/// socket with 1008.
#[derive(Deserialize)]
struct Incoming {
    #[serde(default)]
    sub: Option<Vec<String>>,
    #[serde(default)]
    unsub: Option<Vec<String>>,
    #[serde(default)]
    c: Option<u64>,
    #[serde(default)]
    a: Option<String>,
    #[serde(default)]
    d: Option<Json>,
    #[serde(default)]
    t: Option<String>,
    #[serde(default)]
    hello: Option<Hello>,
    #[serde(default)]
    ack: Option<u64>,
}

/// The first frame of a browser: the token and the last sequence it got, when it reconnects.
#[derive(Deserialize)]
struct Hello {
    #[serde(default)]
    resume: Option<String>,
    #[serde(default)]
    seq: u64,
}

/// Read buffer of a page socket. Frames are small and most sockets sit idle, so the 128 KiB
/// default would be most of a connection's memory; larger frames still arrive in pieces.
const READ_BUFFER: usize = 4 * 1024;

/// Most topics one connection listens to.
const MAX_TOPICS: usize = 16;

pub(super) async fn page_socket(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    ws: Result<WebSocketUpgrade, axum::extract::ws::rejection::WebSocketUpgradeRejection>,
) -> Response {
    let (ctx, socket, user) = match admit(&st, &headers).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let call = Call {
        kind: CallKind::Live,
        screen: "",
        action: None,
        via_agent: ctx.via_agent(),
    };
    if let Err(f) = st.admit_call(&ctx, &call) {
        socket.release_user(&user);
        return st.error_response(&ctx, f);
    }
    let ws = match ws {
        Ok(ws) => ws,
        Err(e) => {
            socket.release_user(&user);
            return e.into_response();
        }
    };
    let limits = socket
        .config()
        .expect("checked in admit")
        .limits;
    let (peer, rx) = match new_peer(&socket, &ctx, &limits) {
        Ok(p) => p,
        Err(f) => {
            socket.release_user(&user);
            return st.error_response(&ctx, f);
        }
    };
    ws.protocols([PROTOCOL])
        .max_message_size(limits.message_bytes)
        .read_buffer_size(READ_BUFFER)
        .write_buffer_size(0)
        .max_write_buffer_size(limits.message_bytes.max(1024 * 1024) * 2)
        .on_upgrade(move |ws| async move {
            run(st, ctx, socket.clone(), peer.clone(), rx, ws, limits).await;
            socket.release_user(&user);
        })
}

/// Makes the peer of a new connection and registers it.
fn new_peer(
    socket: &Socket,
    ctx: &Ctx,
    limits: &Limits,
) -> Result<(Arc<Peer>, mpsc::Receiver<Message>), Failure> {
    let (tx, rx) = mpsc::channel::<Message>(limits.queue);
    let mut secret = [0u8; 16];
    getrandom::getrandom(&mut secret)
        .map_err(|_| Failure::App(Error::Internal("no random source".into())))?;
    let peer = Arc::new(Peer {
        id: socket
            .0
            .next
            .fetch_add(1, Ordering::Relaxed),
        tenant: ctx
            .tenant()
            .map(|t| t.id().to_string())
            .unwrap_or_default(),
        user: ctx
            .identity()
            .user
            .clone()
            .unwrap_or_default()
            .into(),
        identity: ctx.identity().clone(),
        tx: Mutex::new(tx),
        replay: Mutex::new(Replay {
            seq: 0,
            held: VecDeque::new(),
            bytes: 0,
            max: limits.replay,
            max_bytes: limits.replay_bytes,
        }),
        lost: AtomicBool::new(false),
        attached: AtomicBool::new(true),
        epoch: AtomicU64::new(0),
        secret: secret
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
        groups: Mutex::new(Vec::new()),
        topics: Mutex::new(Vec::new()),
        rates: Mutex::new(Rates {
            since: Instant::now(),
            frames: 0,
            calls: 0,
        }),
    });
    if let Ok(mut conns) = socket.0.conns.lock() {
        conns.insert(peer.id, peer.clone());
    }
    Ok((peer, rx))
}

/// Handles a frame from the browser other than `hello`: topics, acknowledgements, calls and
/// typing signals. `Err` with the reason for a frame the connection must be closed for.
fn handle(
    st: &Arc<AppState>,
    ctx: &Ctx,
    socket: &Socket,
    peer: &Arc<Peer>,
    text: &str,
    frame: Incoming,
    limits: &Limits,
) -> Result<(), &'static str> {
    if let Some(names) = frame.sub {
        subscribe(socket, peer, ctx, &names);
    } else if let Some(names) = frame.unsub {
        unsubscribe(socket, peer, &names);
    } else if let Some(seq) = frame.ack {
        if let Ok(mut r) = peer.replay.lock() {
            r.ack(seq);
        }
    } else if let (Some(id), Some(action)) = (frame.c, frame.a) {
        let over = !peer.take_call(limits.calls);
        let conn = Conn {
            socket: socket.clone(),
            peer: peer.clone(),
        };
        tokio::spawn(answer(
            st.clone(),
            ctx.clone(),
            conn,
            id,
            action,
            frame.d.unwrap_or(Json::Null),
            over,
        ));
    } else if frame.t.is_some() {
        let topics = peer
            .topics
            .lock()
            .map(|t| t.clone())
            .unwrap_or_default();
        if let Some((topic, room)) = typing_signal(text, &topics) {
            socket.typing(&peer.tenant, topic, &peer.user, &room);
        }
    } else {
        return Err("bad frame");
    }
    Ok(())
}

/// How a connection ended: closed on purpose (forget the peer now), or dropped (keep it for a
/// resume).
enum Ending {
    Clean(Option<(u16, &'static str)>),
    Dropped,
}

async fn run(
    st: Arc<AppState>,
    ctx: Ctx,
    socket: Socket,
    mut peer: Arc<Peer>,
    mut rx: mpsc::Receiver<Message>,
    ws: axum::extract::ws::WebSocket,
    limits: Limits,
) {
    let (mut sink, mut stream) = ws.split();
    let mut beat = tokio::time::interval(limits.heartbeat);
    beat.tick().await;
    let mut heard = Instant::now();
    let mut first = true;
    let ending = loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(m) => {
                    if sink.send(m).await.is_err() {
                        break Ending::Dropped;
                    }
                    if peer.lost.swap(false, Ordering::Relaxed)
                        && sink.send(Message::Text(r#"{"resync":true}"#.into())).await.is_err()
                    {
                        break Ending::Dropped;
                    }
                }
                None => break Ending::Dropped,
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    heard = Instant::now();
                    if !peer.take_frame(limits.frames) {
                        break Ending::Clean(Some((CLOSE_POLICY, "too many frames")));
                    }
                    let Ok(mut frame) = serde_json::from_str::<Incoming>(&text) else {
                        break Ending::Clean(Some((CLOSE_POLICY, "bad frame")));
                    };
                    if let Some(hello) = frame.hello.take() {
                        if !first {
                            break Ending::Clean(Some((CLOSE_POLICY, "hello comes first")));
                        }
                        let (now, frames) = greet(&socket, peer, hello);
                        peer = now;
                        for f in frames {
                            if sink.send(Message::Text(f.into())).await.is_err() {
                                break;
                            }
                        }
                    } else if let Err(reason) = handle(&st, &ctx, &socket, &peer, &text, frame, &limits) {
                        break Ending::Clean(Some((CLOSE_POLICY, reason)));
                    }
                    first = false;
                }
                Some(Ok(Message::Binary(_))) => break Ending::Clean(Some((CLOSE_POLICY, "text only"))),
                Some(Ok(Message::Close(Some(f)))) if matches!(f.code, 1000 | 1001) => break Ending::Clean(None),
                Some(Err(e)) if too_long(&e) => break Ending::Clean(Some((super::CLOSE_TOO_BIG, "too big"))),
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break Ending::Dropped,
                Some(Ok(_)) => heard = Instant::now(),
            },
            _ = beat.tick() => {
                if heard.elapsed() > limits.heartbeat * 3
                    || sink.send(Message::Ping(Default::default())).await.is_err()
                    || sink.send(Message::Text(r#"{"p":1}"#.into())).await.is_err()
                {
                    break Ending::Dropped;
                }
            }
        }
    };
    match ending {
        Ending::Clean(close) => {
            if let Some((code, reason)) = close {
                let _ = sink
                    .send(Message::Close(Some(CloseFrame {
                        code,
                        reason: reason.into(),
                    })))
                    .await;
            }
            socket.forget(&peer);
        }
        Ending::Dropped => detach(socket, peer, limits.detached),
    }
}

/// Whether a read failed because the frame was over `Limits::message_bytes`. The WebSocket
/// library reports it only as a capacity error whose text says "too long".
fn too_long(e: &axum::Error) -> bool {
    e.to_string().contains("too long")
}

/// Keeps a dropped peer, numbering and holding its messages, until it resumes or
/// [`Limits::detached`] passes.
fn detach(socket: Socket, peer: Arc<Peer>, after: std::time::Duration) {
    peer.attached
        .store(false, Ordering::Release);
    let epoch = peer.epoch.load(Ordering::Acquire);
    tokio::spawn(async move {
        tokio::time::sleep(after).await;
        if !peer.attached.load(Ordering::Acquire) && peer.epoch.load(Ordering::Acquire) == epoch {
            socket.forget(&peer);
        }
    });
}

/// Answers a `hello`: resumes the dropped peer the token names when it belongs to the same user
/// and tenant, moving this connection's queue to it, and returns the peer to use from now on and
/// the frames to send first: `welcome`, then the replayed messages or `resync`.
fn greet(socket: &Socket, fresh: Arc<Peer>, hello: Hello) -> (Arc<Peer>, Vec<String>) {
    let tried = hello.resume.is_some();
    let old = hello
        .resume
        .as_deref()
        .and_then(|t| t.split_once('.'))
        .and_then(|(id, secret)| {
            let id: u64 = id.parse().ok()?;
            let old = socket
                .0
                .conns
                .lock()
                .ok()?
                .get(&id)
                .cloned()?;
            let same = old.tenant == fresh.tenant
                && old.user == fresh.user
                && !old.attached.load(Ordering::Acquire)
                && super::super::same_bytes(old.secret.as_bytes(), secret.as_bytes());
            same.then_some(old)
        });
    let Some(old) = old else {
        let seq = fresh.replay.lock().map_or(0, |r| r.seq);
        let welcome = serde_json::json!({ "welcome": { "resume": fresh.token(), "seq": seq, "resumed": false } });
        let mut frames = vec![welcome.to_string()];
        if tried {
            frames.push(r#"{"resync":true}"#.into());
        }
        return (fresh, frames);
    };
    let Ok(r) = old.replay.lock() else {
        return (fresh, Vec::new());
    };
    if let (Ok(mut to), Ok(from)) = (old.tx.lock(), fresh.tx.lock()) {
        *to = from.clone();
    }
    old.epoch.fetch_add(1, Ordering::AcqRel);
    old.attached
        .store(true, Ordering::Release);
    let replay = r.since(hello.seq);
    let welcome = |replayed: usize| {
        serde_json::json!({ "welcome": { "resume": old.token(), "seq": r.seq, "resumed": true, "replayed": replayed } })
            .to_string()
    };
    let frames = match replay {
        Some(items) => {
            let mut frames = vec![welcome(items.len())];
            frames.extend(items.iter().map(|t| t.to_string()));
            frames
        }
        None => vec![welcome(0), r#"{"resync":true}"#.into()],
    };
    drop(r);
    if let Ok(mut conns) = socket.0.conns.lock() {
        conns.remove(&fresh.id);
    }
    (old, frames)
}

/// Adds the topics the user may read and answers `{"denied":[..]}` with the others.
fn subscribe(socket: &Socket, peer: &Arc<Peer>, ctx: &Ctx, names: &[String]) {
    let Some(config) = socket.config() else {
        return;
    };
    let Ok(mut topics) = peer.topics.lock() else {
        return;
    };
    let mut denied = Vec::new();
    for name in names {
        let allowed = config
            .topics
            .get_key_value(name.as_str())
            .filter(|(_, p)| {
                ctx.identity()
                    .can(&crate::action::Access::Permission(p))
            });
        match allowed {
            Some((n, _)) if topics.contains(n) => {}
            Some((n, _)) if topics.len() < MAX_TOPICS => {
                topics.push(n);
                if let Ok(mut map) = socket.0.topics.lock() {
                    map.entry((peer.tenant.clone(), n.to_string()))
                        .or_default()
                        .push(peer.clone());
                }
            }
            _ => denied.push(name.as_str()),
        }
    }
    if !denied.is_empty() {
        peer.signal(serde_json::json!({ "denied": denied }).to_string());
    }
}

fn unsubscribe(socket: &Socket, peer: &Peer, names: &[String]) {
    let Ok(mut topics) = peer.topics.lock() else {
        return;
    };
    let gone: Vec<&'static str> = topics
        .iter()
        .copied()
        .filter(|t| names.iter().any(|n| n == t))
        .collect();
    topics.retain(|t| !gone.contains(t));
    if let Ok(mut map) = socket.0.topics.lock() {
        for t in gone {
            let key = (peer.tenant.clone(), t.to_string());
            if let Some(subs) = map.get_mut(&key) {
                subs.retain(|p| p.id != peer.id);
                if subs.is_empty() {
                    map.remove(&key);
                }
            }
        }
    }
}

/// Runs an action called over the socket through the same checks as an API call (permission,
/// guards, interceptors with call kind `Live`, validation, audit), plus the app's write limit
/// for actions that change data, and answers `{"r":id,"ok":..}` or `{"r":id,"err":problem}`.
async fn answer(
    st: Arc<AppState>,
    ctx: Ctx,
    conn: Conn,
    id: u64,
    action: String,
    input: Json,
    over: bool,
) {
    let peer = conn.peer.clone();
    let result = async {
        if over {
            return Err(Failure::Status(
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                crate::i18n::Text::key("zeyn.error.rate_limited"),
            ));
        }
        let Some((fi, ai)) = st.api_by_name(&action) else {
            return Err(Failure::App(Error::NotFound));
        };
        let entry = st.features[fi].apis()[ai];
        if !entry.reads
            && let Some(limit) = st.write_rate
            && st
                .rate_store
                .take(&format!("write:user:{}", peer.user), &limit)
                .await
                .is_err()
        {
            return Err(Failure::Status(
                axum::http::StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                crate::i18n::Text::key("zeyn.error.rate_limited"),
            ));
        }
        let ctx = ctx.with_conn(Arc::new(conn));
        st.run_api_as(&ctx, fi, ai, input, CallKind::Live)
            .await
    }
    .await;
    let text = match result {
        Ok(value) => serde_json::json!({ "r": id, "ok": value }).to_string(),
        Err(f) => serde_json::json!({ "r": id, "err": crate::server::problem_body(&ctx, &f).1 })
            .to_string(),
    };
    if text.len() > st_limit(&st) {
        let f = Failure::App(Error::Internal(format!(
            "the answer of `{action}` is larger than the socket's message limit"
        )));
        peer.signal(
            serde_json::json!({ "r": id, "err": crate::server::problem_body(&ctx, &f).1 })
                .to_string(),
        );
        return;
    }
    peer.signal(text);
}

/// Largest answer frame: the largest message a client may send, at least 1 MiB.
fn st_limit(st: &AppState) -> usize {
    st.realtime
        .as_ref()
        .and_then(|s| s.config())
        .map_or(0, |c| c.limits.message_bytes)
        .max(1024 * 1024)
}
