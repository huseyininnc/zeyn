# Realtime Channel

Zeyn requests are stateless: the server keeps no socket and no per user screen state. Some
screens need more: two people editing one diagram, or a list that should update when someone else
saves. The `realtime` Cargo feature adds an opt in WebSocket channel for that. Apps that do not
enable it compile none of it, and pages that do not use it load nothing.

WebSocket is often blocked on corporate networks, so treat the channel as an improvement: every
screen must still work without it (the diagram editor still saves through its action; a list
still refreshes when the user opens it again).

## 1. Turning it on

```toml
zeyn = { version = "...", features = ["realtime"] }
```

```rust
let realtime = Realtime::new()
    .documents("diagram", "diagram.read", "diagram.write", MyDocStore::new(pool))
    .topic("orders", "sales.order.read")
    .limits(Limits::default());
let hub = realtime.hub();          // clone it into stores that notify
App::new(..).realtime(realtime)    // also registers `Hub` as a service
```

## 2. Endpoints

| Endpoint | Purpose |
|---|---|
| `GET /_zeyn/live/doc/{kind}/{doc}?csrf=TOKEN` | WebSocket: a collaborative document |
| `GET /_zeyn/live/topics?t=a,b&csrf=TOKEN` | WebSocket: change notifications for topics |
| `GET /_zeyn/live.js` | The topic script, linked only by pages that subscribe |

Every handshake is checked before the upgrade, in this order:

1. The channel is on (`404` otherwise).
2. **Origin:** the `Origin` header must name the host the request came to, or an origin added with
   `Realtime::allow_origin`. Browsers always send it on WebSocket handshakes; a missing one is
   refused. This is what stops another site from opening a socket with the user's cookies.
3. **CSRF token:** browsers cannot set headers on a WebSocket, so the token from
   `meta[name=zeyn-csrf]` goes in the `csrf` query parameter and is compared in constant time with
   the `zeyn_csrf` cookie, exactly as `x-zeyn-csrf` is for `POST`s. Failures answer `403`.
4. **Identity:** the same authenticator as every request; the user must be signed in (`403`).
5. **Per user limit** (`429`).
6. Documents: a declared kind and a plain id (1 to 64 letters, digits, `-`, `_`, `.`), else `404`;
   the kind's read permission (`403`). Topics: every topic declared and its permission held (`403`).
7. **Interceptors:** the app's interceptors run with a call of kind `Live` (`screen` is the
   document kind, empty for topics), so an app wide rule such as `RequireMfa` or a suspended
   account check covers live channels too. A refusal answers with its status.
8. Room limits: rooms in memory and clients per room (`503` with `Retry-After`).

The write permission is checked once per connection: a client without it receives updates but is
closed with `1008` the moment it sends one.

## 3. Documents and Y-CRDT

Documents are [Y-CRDT](https://docs.yjs.dev) documents held with `yrs`. Rooms are keyed by
**tenant, kind and document id**, so two tenants never share a room even with the same ids.

Frames are binary. The first byte is a tag; `1` (`FRAME_UPDATE`) carries a Y update in v1 encoding.

- On connect the server sends the whole document as one update.
- A client sends updates for its own changes; the server applies each one (a frame that is not a
  valid update closes the socket with `1008`) and passes the same bytes to every other client of
  the room.
- After each (re)connect the island sends its whole state back, so edits made offline merge.
  Y updates are idempotent and commutative, so order and repeats do not matter.

Persistence goes through a trait:

```rust
pub trait DocStore: Send + Sync + 'static {
    fn load(&self, key: RoomKey) -> BoxFuture<Result<Option<Vec<u8>>>>;
    fn save(&self, key: RoomKey, state: Vec<u8>) -> BoxFuture<Result<()>>;
}
```

`load` runs when a room opens (an error refuses the connection with `503`, which is also how a
store limits which documents exist). `save` receives the whole document as one update and runs
every `persist_every` while the document has changes, and before an idle room is dropped. A failed
save is retried on the next round and the room is kept. `MemoryDocStore` is for tests and demos.

On the island side, `zeyn-island` with the `realtime` feature provides `live::Channel`:

```rust
let channel = Channel::document("diagram", "main", on_open, on_update, on_state)?;
channel.send_update(&update);   // after a local change
channel.close();                // in unmount
```

It reconnects after 1 second, doubling up to 30 seconds, and stops after `close` or a `1008`.

## 4. Topics: screens that refresh in place

```rust
Live::new("orders").add(DataGrid::of(search_orders)...)

#[action(permission = "sales.order.write")]
pub async fn save_order(o: Valid<Order>, store: Orders, hub: Hub, ctx: Ctx) -> Result<Order> {
    let saved = store.save(o.into_inner())?;
    hub.notify(&ctx, "orders");
    Ok(saved)
}
```

`Live` renders `div[data-zeyn-live="orders"]`. A page whose screen holds one links
`/_zeyn/live.js` (a screen that arrives through `nav.js` gets `x-zeyn-live: 1` and the script is
imported then). The script opens one socket for all topics on the page. On a notification each
subscribed element reloads its grids in place (`zeyn:reload`, the grid's current page, sort and
filters), or the whole screen in place (`zeyn:refresh`) when it holds no grid. Notifications within
300 ms are handled once.

Notifications carry only the topic name, never data: the refresh goes through the normal grid
endpoint with every permission check. `notify` reaches the caller's tenant only;
`notify_tenant(Some(id), topic)` names one, `None` is the app without tenancy.

The same topic names work without the channel. Nodes marked with `.listens("orders")` refresh in
place when a notification names the topic (`live.js` passes it on as `zeyn:topics`), and also
when an action ends with `ui.refresh("orders")` or a filter that `.drives("orders")` changes:
grids reload their current page, other listeners are rendered again with one `POST /_zeyn/nodes`
per screen (see Topics in `examples/docs/pages/effects.md`). The socket subscribes only to the
topics of `Live` nodes, which must be declared with `Realtime::topic`; use `.listens` inside or
next to them for the parts that are not grids.

## 5. Memory and backpressure

Every piece of state has a bound (`Limits`, defaults in brackets):

| Bound | Default | When reached |
|---|---|---|
| `per_user` sockets | 8 | handshake `429` |
| `per_room` clients | 32 | handshake `503` |
| `rooms` in memory | 256 | handshake `503` |
| `message_bytes` from a client | 64 KB | socket closed |
| `doc_bytes` per document | 1 MB | the update is refused and the socket closed (`1009`) |
| `queue` per client | 32 messages | the client is dropped from the room and closed with `1013` |
| `idle` room without clients | 30 s | persisted and dropped from memory |
| `heartbeat` | 20 s | a ping; a client silent for 3 intervals is closed |
| `persist_every` | 2 s | changed documents are saved |

Broadcasts never wait: an update is offered to each client's bounded queue with `try_send`. A
client whose queue is full is removed from the room at once, so one slow reader cannot hold memory
or slow the others; it reconnects and receives the whole document. A topic subscriber with a full
queue only misses that notification (the next one refreshes it anyway).

Worst case memory is therefore about `rooms * doc_bytes` for documents plus
`clients * queue * message_bytes` for queues (an update is shared between queues, not copied). The
showcase server with two live tabs on the diagram uses about 19 MB in total (debug build).
`Hub::stats()` reports rooms, clients, subscribers and document bytes for monitoring.

## 6. Security notes

- The channel never runs actions: documents are data the app reads through its `DocStore`, which
  is where it validates them (the showcase clamps every saved diagram with `Diagram::clamped`).
- A document is readable by anyone with the kind's read permission in the tenant; use separate
  kinds (or ids the store refuses) for documents with different audiences.
- The policy of every page allows WebSocket to the app's own origin through `default-src 'self'`;
  nothing in the content security policy is relaxed for the channel.

## 7. Example

`examples/showcase`: open Islands, Diagram editor in two tabs. Moving or adding a shape in one tab
shows up in the other, and the grid "Shapes stored on the server" below refreshes in place within
about two seconds, when the document is saved through `DocStore` and the `diagram` topic is
notified.

Limits of the current version: rooms live in one process (several server instances need sticky
sessions or a shared broker), presence (who else is here, cursors) is not built in, and a document
is sent whole on every connect.
