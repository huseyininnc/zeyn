# ZeynSocket: design

Status: ZEYN-272 (the page socket at `/_zeyn/socket`, `ZeynSocket`, `Socket`, events, groups,
`Conn`, actions over the socket), ZEYN-273 (resume, replay, heartbeat, backoff) and ZEYN-277
(limits, tenant isolation, memory), ZEYN-276 (`on_event`, `zeyn.socket`, `TestSocket`) and
ZEYN-274 (Server-Sent Events fallback) and ZEYN-275 (Redis) are built. ZEYN-278 follows.

ZeynSocket is the one live connection between a Zeyn page and the server. It grows out of the
`realtime` feature (`docs/realtime.md`), which already has a checked handshake, bounded queues,
topics and collaborative documents. What it adds: actions called over the socket, typed events
pushed to a user, a group or a tenant, a connection that survives network drops without losing
messages, a fallback for networks that block WebSocket, and several app instances behind one
Redis.

The goal for the API: an agent that already knows actions and screens can write a chat, a
notification bell or a job progress bar without learning a second model.

## 1. Decisions

| Question | Decision | Why |
|---|---|---|
| Client to server calls | Existing `#[action]`s, called over the socket | Permissions, `Valid<T>`, injection, audit and interceptors already apply to actions. A separate hub method type would be a second way to write server code. |
| Server to client | Typed events: a `Serialize` struct with a stable name (`#[derive(Event)]`) | Names are checked by the compiler; the browser and the Rust client read the same name. |
| Topics (`Hub::notify`) | Kept, carried by the same socket | Topics are events with no data; `Live` and `.listens` keep working unchanged. |
| Collaborative documents | Stay on their own socket per document | Binary Y updates and per room limits do not fit the event envelope; no gain from merging. |
| Names | `Realtime` becomes `ZeynSocket`, `Hub` becomes `Socket` | One name per idea. Alpha: no deprecated aliases, `zeyn check --fix` rewrites old calls. |
| Sockets per page | One, shared by every component on the page | Fewer connections per user; the `per_user` limit stays meaningful. |
| Server state | Only open connections, groups and short replay buffers, all capped | Screens stay stateless; a lost server loses no data, clients resync. |
| CSRF | Sent as the subprotocol `csrf.TOKEN` next to `zeyn`, never in the URL | URLs land in proxy and server logs (ZEYN-258). The subprotocol keeps it out of them and is checked before the upgrade, so no unauthenticated socket is ever opened. The handshake also checks `Origin`. |
| Fallback | Server-Sent Events down, `POST` up | Works through proxies that strip `Upgrade`; same envelope, same resume. Long polling is not built. `POST /_zeyn/socket/open` with the CSRF token returns a single use ticket (30 s, same user) for `GET /_zeyn/socket/events?ticket=`; the ticket is the only thing in a URL. `POST /_zeyn/socket/send` takes one frame with the CSRF token and `x-zeyn-socket: <resume token>`. |
| Scale out | Redis pub/sub behind the `redis` feature | Already a dependency of the `redis` feature; no sticky sessions needed. |

Not built: server to client calls that wait for an answer (SignalR client results), binary
message packs, streams of values from an action (use events with a job id instead).

## 2. API sketch

```rust
use zeyn::prelude::*;

#[derive(Serialize, Event)]
#[event(name = "chat.message")]
pub struct MessageSent {
    pub room: String,
    pub text: String,
    pub by: String,
}

#[action(permission = "chat.write")]
pub async fn send_message(m: Valid<NewMessage>, socket: Socket, ctx: Ctx) -> Result<()> {
    let m = m.into_inner();
    socket
        .to_group(&ctx, format!("room:{}", m.room))
        .emit(MessageSent { room: m.room, text: m.text, by: ctx.user_name() });
    Ok(())
}

#[action(permission = "chat.read")]
pub async fn join_room(r: RoomId, conn: Conn, rooms: Rooms, ctx: Ctx) -> Result<()> {
    rooms.check_member(&ctx, &r)?;
    conn.join(format!("room:{}", r.0));
    Ok(())
}

App::new("Chat")
    .socket(
        ZeynSocket::new()
            .topic("orders", "sales.order.read")
            .event::<MessageSent>("chat.read")
            .limits(Limits::default()),
    )
```

- `Socket` is a service like `Hub` today. `to_user(&ctx, id)`, `to_group(&ctx, name)`,
  `to_tenant(&ctx)` and `notify(&ctx, topic)` all stay inside the caller's tenant;
  `to_tenant_id(Some(id))` names one explicitly, the same as `notify_tenant`.
- `Conn` is the calling connection. It exists only when the action was called over the socket;
  asking for it in a plain HTTP call answers `422` with a validation message. `join` and `leave` change its groups. Joining is
  never automatic: the action that calls `join` is where the app checks membership.
- `.event::<E>(permission)` declares an event and the permission a receiver needs. A send to a user
  without it is dropped on the server. A type without `#[derive(Event)]` does not compile
  (`#[diagnostic::on_unimplemented]` says what to add); emitting an event that was never declared
  with `.event::<E>()` returns an error.

Screens, no JavaScript (`OnEvent::Refresh` and `OnEvent::Toast` are built; a refresh reloads grids in
place, so a separate `Reload` is not needed):

```rust
List::of(messages)
    .on_event::<MessageSent>(OnEvent::Reload)
Badge::count(unread)
    .on_event::<MessageSent>(OnEvent::Refresh)
Screen::new("Orders")
    .on_event::<OrderShipped>(OnEvent::Toast(t!("orders.shipped")))
```

Islands and custom browser code:

```js
const off = zeyn.socket.on('chat.message', (m) => { ... });
await zeyn.socket.call('send_message', { room, text });   // rejects with the action's problem
zeyn.socket.state;  // 'connected' | 'reconnecting' | 'offline'
```

Rust client, for tests and services:

```rust
let addr = TestSocket::serve(app.router()?).await;          // feature `socket-client`
let mut c = TestSocket::connect(addr, &[("authorization", "Bearer ..")]).await?;
c.call("join_room", json!({ "room": "1" })).await?;
let m: MessageSent = c.next_event().await?;
```

## 3. Wire protocol

One JSON text frame per message. Short keys keep frames small.

Client to server:

| Frame | Meaning |
|---|---|
| `{"hello":{"resume":"..","seq":41}}` | First frame; `resume` and `seq` only when reconnecting. Any other frame first starts a new session without one. |
| `{"c":7,"a":"send_message","d":{..}}` | Call action `a` with input `d`; `c` is the caller's id. |
| `{"sub":["orders"]}` / `{"unsub":[..]}` | Topics of the page; the server answers `{"denied":[..]}` with the ones the user may not read. |
| `{"t":"typing","topic":"chat","room":".."}` | A typing signal on a subscribed topic. |
| `{"ack":41}` | Highest event sequence received, sent at most once a second. Lets the server trim the replay buffer early. |

Server to client:

| Frame | Meaning |
|---|---|
| `{"welcome":{"resume":"..","seq":0,"resumed":true,"replayed":3}}` | After `hello`. `resume` is the connection id and a random secret; it resumes only a dropped connection of the same user and tenant. |
| `{"p":1}` | Heartbeat every 20 s, so the page can tell a dead connection. |
| `{"s":42,"e":"chat.message","d":{..}}` | Event with sequence `s`. |
| `{"s":43,"t":"orders"}` | Topic notification (no data). |
| `{"r":7,"ok":..}` / `{"r":7,"err":{problem}}` | Answer to call `c:7`; `err` is the same problem JSON as HTTP. |
| `{"resync":true}` | Events were lost; the client reloads its live parts. |

Close codes:

| Code | Meaning | Client |
|---|---|---|
| 1000 | Normal | Reconnect only if components still need the socket |
| 1008 | Policy (bad frame, forbidden) | Stop |
| 1009 | Too big | Stop |
| 1013 | Too slow, queue full | Reconnect and resume |
| 4401 | Signed out or session expired | Stop, `zeyn:signed-out` |

## 4. A connection that does not drop

- **Heartbeat:** the server pings every 20 s; three silent intervals close the socket. The client
  treats 45 s without any frame as a dead connection and reconnects (proxies that swallow closes).
- **Backoff:** 0.5 s, doubling to 30 s, with full jitter. `online` and `visibilitychange` to
  visible retry at once. Codes that say stop never retry.
- **Resume:** after a drop the server keeps the connection's groups and a replay buffer for 60 s.
  The client sends `resume` and its last `seq`; the server replays what is newer and keeps the same
  groups and topics. If the buffer no longer holds `seq + 1`, or the token is for another user or
  expired, it answers `welcome` with `resumed: false` and `resync` on the open socket. A close with
  1000 or 1001 (the page closed the socket on purpose) forgets the connection at once. Calls in flight during the drop are rejected on the client with `offline`; actions are
  not retried automatically, because they are not known to be idempotent.
- **State in the page:** `zeyn:socket-state` events and an optional `SocketStatus` component
  (hidden while connected, a polite live region otherwise).

## 5. Limits and memory

Existing `Limits` stay. New fields, defaults in brackets:

| Bound | Default | When reached |
|---|---|---|
| `replay` events per connection | 64 | oldest dropped; a resume past it gets `resync` |
| `replay_bytes` per connection | 64 KB | as above |
| `detached` time a dropped connection is kept | 60 s | groups and buffer dropped |
| `groups` per connection | 32 | `join` answers `429` |
| `calls` per connection per second | 20 | the call answers `429`; the write limit applies to write actions as for HTTP |
| `frames` per connection per second | 100 | the socket is closed with 1008 |
| `event_bytes` | 16 KB | `emit` returns an error and sends nothing |

Worst case per user: `per_user (8) * (queue + replay_bytes)`, about 1 MB. An event is serialized
once and shared between receivers' queues (`Arc<str>`).

## 6. Security

- Handshake order from `docs/realtime.md` stays: origin, CSRF token in the subprotocol, a signed
  in user, the per user limit, the interceptors with call kind `Live`.
- Every call runs the action's full pipeline: permission, guards, interceptors (call kind `Live`),
  validation, audit. Actions that are not reads also take a token of the app's write limit, keyed
  by user. Only actions published with `Feature::api` can be called.
- Every event delivery checks the receiver's permission for that event. Group membership alone
  never grants it.
- Group and user names are prefixed with the tenant on the server; a client never sends a group
  name, only actions do.
- The resume token is signed and bound to user and tenant. It does not grant anything the session
  cookie does not.
- Events never carry masked values. Send the id and let the receiver load the record through an
  action, which masks as usual. The guide and `zeyn check` (ZEYN-219) flag events built from
  entities with masked fields.

## 7. Scale out

With `ZeynSocket::redis(&store)` (a `RedisStore`, so its prefix and connection settings are reused)
every `emit`, `notify` and typing signal is published on one channel, `zeyn:<prefix>:socket`, with
the tenant inside the message; one channel keeps subscriptions fixed, and every instance would get
every tenant's messages anyway when users of a tenant are spread over instances.
Each instance delivers to its own connections. Sequence numbers are per connection, so they are
assigned by the instance that holds the connection. A resume that lands on another instance has no
buffer there and gets `resync`. Redis being down turns sends into local delivery and logs a warning
once a minute.

## 8. Order of work

1. ZEYN-272: rename, envelope, actions over the socket, events, groups, `Conn`.
2. ZEYN-273: heartbeat, backoff, resume, state events.
3. ZEYN-277: security and load tests (runs next to 273, needs 272).
4. ZEYN-276: `on_event` for screens, `zeyn.socket`, `TestSocket`.
5. ZEYN-274: SSE fallback.
6. ZEYN-275: Redis.
7. ZEYN-278: guide, skill file, examples.
