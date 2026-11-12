# ZeynSocket

One live connection per page carries everything the server pushes: topic notifications that
refresh screens, typed events for a user, a group or a tenant, and actions the browser calls
without a page request. Turn it on with the `realtime` feature:

```toml
zeyn = { version = "...", features = ["realtime"] }
```

```rust
App::new("Shop")
    .socket(
        ZeynSocket::new()
            .topic("orders", "sales.order.read")
            .event::<OrderShipped>("sales.order.read")
            .event::<MessageSent>("chat.read"),
    )
```

`App::socket` also registers the `Socket` service, so actions take `socket: Socket`. The wire
protocol, the limits and the reasons behind them are in `docs/zeynsocket.md`.

## Topics: screens that refresh

A topic carries no data. It tells the screens that show some data to load it again, through the
normal endpoints and their permission checks.

```rust
Live::new("orders").add(DataGrid::of(search_orders))

#[action(permission = "sales.order.write")]
pub async fn ship(o: OrderId, store: Orders, socket: Socket, ctx: Ctx) -> Result<()> {
    store.ship(&o)?;
    socket.notify(&ctx, "orders");
    Ok(())
}
```

Declare every topic with `ZeynSocket::topic` and the permission a subscriber needs. Nodes marked
with `.listens("orders")` refresh as well.

## Events: data for some users

An event is a `Serialize` type with a stable name. Every receiver must hold the permission the
event was declared with; a group never grants it.

```rust
#[derive(Serialize, Event)]
#[event(name = "chat.message")]
pub struct MessageSent {
    pub room: String,
    pub text: String,
}

socket.to_user(&ctx, "ayse").emit(&notice)?;          // every open page of one user
socket.to_group(&ctx, format!("room:{id}")).emit(&m)?; // members of a group
socket.to_tenant(&ctx).emit(&notice)?;                 // the whole tenant
socket.to_user_id(Some("acme"), "ayse").emit(&n)?;     // from a job, without a request
```

Everything stays inside one tenant. Events never carry masked values: send an id and let the
receiver load the record through an action, which masks as usual. An event larger than
`Limits::event_bytes` (16 KB) or never declared returns an error and sends nothing.

## Groups

A connection joins a group only in an action called over the socket. That action is where the
app checks that the user may be in the group.

```rust
#[action(permission = "chat.read")]
pub async fn join_room(r: RoomId, conn: Conn, rooms: Rooms, ctx: Ctx) -> Result<()> {
    rooms.check_member(&ctx, &r)?;
    conn.join(format!("room:{}", r.0))
}
```

`Conn` exists only for calls over the socket; the same action called over HTTP answers `422`.
A connection is in at most `Limits::groups` (32) groups.

## Screens without JavaScript

```rust
List::of(messages).on_event::<MessageSent>(OnEvent::Refresh)
Alert::info(t!("orders.title")).on_event::<OrderShipped>(OnEvent::Toast(t!("orders.shipped")))
```

`OnEvent::Refresh` refreshes the node in place when the event reaches this user;
`OnEvent::Toast` shows a toast. Only events the user may receive ever arrive.

## Browser code

Islands and custom scripts use `zeyn.socket`:

```js
const off = zeyn.socket.on('chat.message', (m) => show(m));
await zeyn.socket.call('send_message', { room, text }); // rejects with the action's problem
zeyn.socket.state; // 'connected' | 'reconnecting' | 'offline', also as zeyn:socket-state
```

Actions published with `Feature::api` can be called. Each call runs the same checks as an API
call: permission, guards, interceptors (call kind `Live`), validation, audit, and the app's write
limit for actions that change data.

## A connection that does not drop

The browser reconnects with growing, random pauses and at once when it comes back online. After a
drop the server keeps the connection's topics, groups and last messages for `Limits::detached`
(60 seconds) and replays what the page missed; when that is not possible the page reloads its live
parts. Where proxies block WebSocket the page switches by itself to Server-Sent Events down and
`POST` up, with the same messages.

## Several servers

```rust
let redis = RedisStore::new("redis://cache:6379", "shop")?;
ZeynSocket::new().redis(&redis)   // features `realtime` and `redis`
```

Every instance delivers what the others publish to its own connections, so no sticky sessions
are needed. While Redis is down sends stay on the instance.

## Tests

The `socket-client` feature adds `TestSocket`, a client that talks to the socket like a browser:

```rust
let addr = TestSocket::serve(app.router()?).await;
let mut c = TestSocket::connect(addr, &[("authorization", "Bearer ..")]).await?;
c.call("join_room", json!({ "room": "1" })).await?;
let m: MessageSent = c.next_event().await?;
```

## Limits

| Bound | Default |
|---|---|
| Open sockets per user | 8 |
| Messages waiting per connection | 32 |
| Calls per connection per second | 20 |
| Frames per connection per second | 100 |
| Largest frame | 64 KB |
| Largest event | 16 KB |
| Replay buffer per connection | 64 messages, 64 KB |
| Time a dropped connection is kept | 60 s |

An idle connection costs about 16 KB, measured with server and client in one process.
