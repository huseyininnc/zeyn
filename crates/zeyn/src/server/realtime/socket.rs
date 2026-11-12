//! The page socket at `/_zeyn/socket`: one connection per page that carries topic
//! notifications, typed events for a user, a group or a tenant, and actions called by name. See
//! `docs/zeynsocket.md` for the wire protocol.

use super::{CLOSE_POLICY, Limits, Outbox, PROTOCOL, Socket, admit, typing_signal};
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

/// One open page socket: who it belongs to, its outgoing queue, its event counter and its
/// groups.
pub(super) struct Peer {
    pub(super) id: u64,
    pub(super) tenant: String,
    pub(super) user: Arc<str>,
    identity: Identity,
    tx: Outbox,
    seq: AtomicU64,
    /// Set when a message did not fit the queue; the connection then tells the browser to resync.
    lost: AtomicBool,
    groups: Mutex<Vec<String>>,
}

impl Peer {
    /// Queues a message numbered with the next sequence. `false` when the connection is gone.
    fn send_numbered(&self, body: impl FnOnce(u64) -> String) -> bool {
        let s = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        self.send(body(s))
    }

    fn send(&self, text: String) -> bool {
        match self
            .tx
            .try_send(Message::Text(text.into()))
        {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.lost.store(true, Ordering::Relaxed);
                true
            }
            Err(mpsc::error::TrySendError::Closed(_)) => false,
        }
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
    fn forget(&self, peer: &Peer, topics: &[&'static str]) {
        if let Ok(mut map) = self.0.topics.lock() {
            for t in topics {
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
}

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
    let (tx, rx) = mpsc::channel::<Message>(limits.queue);
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
        tx,
        seq: AtomicU64::new(0),
        lost: AtomicBool::new(false),
        groups: Mutex::new(Vec::new()),
    });
    if let Ok(mut conns) = socket.0.conns.lock() {
        conns.insert(peer.id, peer.clone());
    }
    ws.protocols([PROTOCOL])
        .max_message_size(limits.message_bytes)
        .on_upgrade(move |ws| async move {
            run(st, ctx, socket.clone(), peer.clone(), rx, ws, limits).await;
            socket.release_user(&user);
        })
}

async fn run(
    st: Arc<AppState>,
    ctx: Ctx,
    socket: Socket,
    peer: Arc<Peer>,
    mut rx: mpsc::Receiver<Message>,
    ws: axum::extract::ws::WebSocket,
    limits: Limits,
) {
    let (mut sink, mut stream) = ws.split();
    let mut topics: Vec<&'static str> = Vec::new();
    let mut beat = tokio::time::interval(limits.heartbeat);
    beat.tick().await;
    let mut heard = Instant::now();
    let mut window = (Instant::now(), 0u32);
    let conn = Conn {
        socket: socket.clone(),
        peer: peer.clone(),
    };
    let close = loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(m) => {
                    if sink.send(m).await.is_err() {
                        break None;
                    }
                    if peer.lost.swap(false, Ordering::Relaxed)
                        && sink.send(Message::Text(r#"{"resync":true}"#.into())).await.is_err()
                    {
                        break None;
                    }
                }
                None => break None,
            },
            incoming = stream.next() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    heard = Instant::now();
                    let Ok(frame) = serde_json::from_str::<Incoming>(&text) else {
                        break Some((CLOSE_POLICY, "bad frame"));
                    };
                    if let Some(names) = frame.sub {
                        subscribe(&socket, &peer, &ctx, &mut topics, &names);
                    } else if let Some(names) = frame.unsub {
                        unsubscribe(&socket, &peer, &mut topics, &names);
                    } else if let (Some(id), Some(action)) = (frame.c, frame.a) {
                        if window.0.elapsed().as_secs() >= 1 {
                            window = (Instant::now(), 0);
                        }
                        window.1 += 1;
                        let over = window.1 > limits.calls;
                        tokio::spawn(answer(
                            st.clone(),
                            ctx.clone(),
                            conn.clone(),
                            id,
                            action,
                            frame.d.unwrap_or(Json::Null),
                            over,
                        ));
                    } else if frame.t.is_some() {
                        if let Some((topic, room)) = typing_signal(&text, &topics) {
                            socket.typing(&peer.tenant, topic, &peer.user, &room);
                        }
                    } else {
                        break Some((CLOSE_POLICY, "bad frame"));
                    }
                }
                Some(Ok(Message::Binary(_))) => break Some((CLOSE_POLICY, "text only")),
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break None,
                Some(Ok(_)) => heard = Instant::now(),
            },
            _ = beat.tick() => {
                if heard.elapsed() > limits.heartbeat * 3 || sink.send(Message::Ping(Default::default())).await.is_err() {
                    break None;
                }
            }
        }
    };
    if let Some((code, reason)) = close {
        let _ = sink
            .send(Message::Close(Some(CloseFrame {
                code,
                reason: reason.into(),
            })))
            .await;
    }
    socket.forget(&peer, &topics);
}

/// Adds the topics the user may read and answers `{"denied":[..]}` with the others.
fn subscribe(
    socket: &Socket,
    peer: &Arc<Peer>,
    ctx: &Ctx,
    topics: &mut Vec<&'static str>,
    names: &[String],
) {
    let Some(config) = socket.config() else {
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

fn unsubscribe(socket: &Socket, peer: &Peer, topics: &mut Vec<&'static str>, names: &[String]) {
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
