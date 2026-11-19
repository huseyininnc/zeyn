//! The fallback transport of the page socket, for networks whose proxies block WebSocket:
//! Server-Sent Events down and `POST` up, with the same frames, sequences and resume. The browser
//! opens it with `POST /_zeyn/socket/open` (CSRF token, optional resume), gets a single use ticket
//! for `GET /_zeyn/socket/events?ticket=..`, and sends each frame with `POST /_zeyn/socket/send`
//! and the resume token in `x-zeyn-socket`.

use super::super::{Socket, admit_post};
use super::{Hello, Incoming, Limits, Peer, detach, greet, handle, new_peer};
use crate::action::Error;
use crate::server::{AppState, Call, CallKind, Failure};
use axum::body::Body;
use axum::extract::ws::Message;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event as SseEvent, Sse};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// How long a ticket waits for its event stream.
const TICKET_TTL: Duration = Duration::from_secs(30);
/// Most tickets waiting at once.
const MAX_TICKETS: usize = 1024;
/// Largest body of the `POST` that opens the transport.
const OPEN_BYTES: usize = 1024;

/// An opened fallback transport waiting for its event stream: the peer, its queue, the frames to
/// send first and the per user slot it holds.
pub(crate) struct Ticket {
    peer: Arc<Peer>,
    rx: mpsc::Receiver<Message>,
    first: Vec<String>,
    made: Instant,
    user: String,
}

#[derive(Deserialize, Default)]
struct Open {
    #[serde(default)]
    resume: Option<String>,
    #[serde(default)]
    seq: u64,
}

fn bad_request(st: &AppState, ctx: &crate::Ctx) -> Response {
    let mut errors = crate::model::ValidationErrors::default();
    errors.add_general(crate::i18n::Text::key("zeyn.error.bad_request"));
    let mut r = st.error_response(ctx, Failure::App(Error::Validation(errors)));
    *r.status_mut() = StatusCode::BAD_REQUEST;
    r
}

/// `POST /_zeyn/socket/open`: the checks of a WebSocket handshake with the CSRF token in its
/// header, then a peer (new or resumed) and a ticket for its event stream.
pub(in crate::server) async fn open(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let (ctx, socket, user) = match admit_post(&st, &headers).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let refuse = |r: Response| {
        socket.release_user(&user);
        r
    };
    let call = Call {
        kind: CallKind::Live,
        screen: "",
        action: None,
        via_agent: ctx.via_agent(),
    };
    if let Err(f) = st.admit_call(&ctx, &call) {
        return refuse(st.error_response(&ctx, f));
    }
    #[cfg(feature = "redis")]
    socket.listen();
    let Ok(raw) = axum::body::to_bytes(body, OPEN_BYTES).await else {
        return refuse(bad_request(&st, &ctx));
    };
    let wanted: Open = if raw.is_empty() {
        Open::default()
    } else {
        match serde_json::from_slice(&raw) {
            Ok(o) => o,
            Err(_) => return refuse(bad_request(&st, &ctx)),
        }
    };
    let limits = socket
        .config()
        .expect("checked in admit")
        .limits;
    let (fresh, rx) = match new_peer(&socket, &ctx, &limits) {
        Ok(p) => p,
        Err(f) => return refuse(st.error_response(&ctx, f)),
    };
    let (peer, first) = greet(
        &socket,
        fresh,
        Hello {
            resume: wanted.resume,
            seq: wanted.seq,
        },
    );
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() {
        socket.forget(&peer);
        return refuse(st.error_response(
            &ctx,
            Failure::App(Error::Internal("no random source".into())),
        ));
    }
    let ticket: String = bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let stale = {
        let Ok(mut tickets) = socket.0.tickets.lock() else {
            socket.forget(&peer);
            return refuse(super::super::busy());
        };
        let stale: Vec<String> = tickets
            .iter()
            .filter(|(_, t)| t.made.elapsed() > TICKET_TTL)
            .map(|(k, _)| k.clone())
            .collect();
        let stale: Vec<Ticket> = stale
            .iter()
            .filter_map(|k| tickets.remove(k))
            .collect();
        if tickets.len() >= MAX_TICKETS {
            drop(tickets);
            for t in stale {
                expire(&socket, t);
            }
            socket.forget(&peer);
            return refuse(super::super::busy());
        }
        tickets.insert(
            ticket.clone(),
            Ticket {
                peer,
                rx,
                first,
                made: Instant::now(),
                user,
            },
        );
        stale
    };
    for t in stale {
        expire(&socket, t);
    }
    let mut r = axum::Json(serde_json::json!({ "ticket": ticket })).into_response();
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

/// Drops a ticket nobody picked up: its peer and its per user slot.
fn expire(socket: &Socket, t: Ticket) {
    socket.forget(&t.peer);
    socket.release_user(&t.user);
}

#[derive(Deserialize)]
pub(crate) struct EventsQuery {
    #[serde(default)]
    ticket: String,
}

/// Detaches the peer and gives back the per user slot when the event stream ends.
struct Ending {
    socket: Socket,
    peer: Arc<Peer>,
    user: String,
    detached: Duration,
}

impl Drop for Ending {
    fn drop(&mut self) {
        self.socket.release_user(&self.user);
        detach(self.socket.clone(), self.peer.clone(), self.detached);
    }
}

/// The state of one event stream.
struct Feed {
    rx: mpsc::Receiver<Message>,
    first: VecDeque<String>,
    beat: tokio::time::Interval,
    ending: Ending,
}

/// `GET /_zeyn/socket/events?ticket=..`: the frames of the peer as Server-Sent Events, the ticket
/// used once and only by the user who opened it.
pub(in crate::server) async fn events(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<EventsQuery>,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let Some(socket) = st.realtime.clone() else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    let Some(limits) = socket.config().map(|c| c.limits) else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    let ticket = socket
        .0
        .tickets
        .lock()
        .ok()
        .and_then(|mut t| t.remove(&q.ticket));
    let Some(ticket) = ticket else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    let tenant = ctx
        .tenant()
        .map(|t| t.id())
        .unwrap_or("");
    let same = ctx.identity().user.as_deref() == Some(&*ticket.peer.user)
        && tenant == ticket.peer.tenant
        && ticket.made.elapsed() <= TICKET_TTL;
    if !same {
        expire(&socket, ticket);
        return st.error_response(&ctx, Failure::App(Error::Forbidden));
    }
    feed(socket, ticket, limits)
}

fn feed(socket: Socket, ticket: Ticket, limits: Limits) -> Response {
    let mut beat = tokio::time::interval(limits.heartbeat);
    beat.reset();
    let state = Feed {
        rx: ticket.rx,
        first: ticket.first.into(),
        beat,
        ending: Ending {
            socket,
            peer: ticket.peer,
            user: ticket.user,
            detached: limits.detached,
        },
    };
    let stream = futures_util::stream::unfold(state, |mut f| async move {
        if let Some(text) = f.first.pop_front() {
            return Some((Ok::<_, Infallible>(SseEvent::default().data(text)), f));
        }
        if f.ending
            .peer
            .lost
            .swap(false, Ordering::Relaxed)
        {
            return Some((Ok(SseEvent::default().data(r#"{"resync":true}"#)), f));
        }
        let event = tokio::select! {
            m = f.rx.recv() => match m {
                Some(Message::Text(t)) => SseEvent::default().data(t.as_str()),
                Some(_) => SseEvent::default().data(r#"{"p":1}"#),
                None => return None,
            },
            _ = f.beat.tick() => SseEvent::default().data(r#"{"p":1}"#),
        };
        Some((Ok(event), f))
    });
    let mut r = Sse::new(stream).into_response();
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r.headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    r
}

/// `POST /_zeyn/socket/send`: one frame for the peer named by `x-zeyn-socket` (its resume
/// token), from the user who owns it, with the CSRF token. Answers and events go down the event
/// stream.
pub(in crate::server) async fn send(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let Some(socket) = st.realtime.clone() else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    let Some(limits) = socket.config().map(|c| c.limits) else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    let peer = headers
        .get("x-zeyn-socket")
        .and_then(|v| v.to_str().ok())
        .and_then(|t| t.split_once('.'))
        .and_then(|(id, secret)| {
            let id: u64 = id.parse().ok()?;
            let peer = socket
                .0
                .conns
                .lock()
                .ok()?
                .get(&id)
                .cloned()?;
            crate::server::same_bytes(peer.secret.as_bytes(), secret.as_bytes()).then_some(peer)
        });
    let tenant = ctx
        .tenant()
        .map(|t| t.id())
        .unwrap_or("");
    let Some(peer) =
        peer.filter(|p| ctx.identity().user.as_deref() == Some(&*p.user) && p.tenant == tenant)
    else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    if !peer.take_frame(limits.frames) {
        let mut r = st.error_response(
            &ctx,
            Failure::App(Error::Denied(crate::i18n::Text::key(
                "zeyn.error.rate_limited",
            ))),
        );
        *r.status_mut() = StatusCode::TOO_MANY_REQUESTS;
        return r;
    }
    let Ok(raw) = axum::body::to_bytes(body, limits.message_bytes).await else {
        let mut r = bad_request(&st, &ctx);
        *r.status_mut() = StatusCode::PAYLOAD_TOO_LARGE;
        return r;
    };
    let text = String::from_utf8_lossy(&raw);
    let frame = match serde_json::from_str::<Incoming>(&text) {
        Ok(f) if f.hello.is_none() => f,
        _ => return bad_request(&st, &ctx),
    };
    match handle(&st, &ctx, &socket, &peer, &text, frame, &limits) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => bad_request(&st, &ctx),
    }
}
