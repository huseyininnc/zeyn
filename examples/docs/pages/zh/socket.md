# ZeynSocket

每个页面一条实时连接，承载服务器推送的一切：刷新屏幕的主题通知，发给某个用户、某个群组或某个租户的类型化事件，以及浏览器无需页面请求即可调用的 action。用 `realtime` feature 启用：

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

`App::socket` 还会注册 `Socket` 服务，因此 action 可以接收 `socket: Socket`。传输协议、各项限制及其原因见 `docs/zeynsocket.md`。

## 主题：会刷新的屏幕

主题不携带数据。它通知显示某些数据的屏幕，通过普通端点及其权限检查重新加载这些数据。

```rust
Live::new("orders").add(DataGrid::of(search_orders))

#[action(permission = "sales.order.write")]
pub async fn ship(o: OrderId, store: Orders, socket: Socket, ctx: Ctx) -> Result<()> {
    store.ship(&o)?;
    socket.notify(&ctx, "orders");
    Ok(())
}
```

用 `ZeynSocket::topic` 声明每个主题，并给出订阅者需要的权限。标记了 `.listens("orders")` 的节点也会刷新。

## 事件：给部分用户的数据

事件是一个具有稳定名称的 `Serialize` 类型。每个接收者都必须拥有声明该事件时指定的权限；群组永远不会授予这项权限。

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

一切都限定在一个租户之内。事件从不携带脱敏的值：发送一个 id，让接收者通过 action 加载记录，action 会照常脱敏。大于 `Limits::event_bytes`（16 KB）或从未声明的事件会返回错误，并且不发送任何内容。

## 群组

连接只能在通过套接字调用的 action 中加入群组。应用正是在这个 action 中检查用户是否可以加入该群组。

```rust
#[action(permission = "chat.read")]
pub async fn join_room(r: RoomId, conn: Conn, rooms: Rooms, ctx: Ctx) -> Result<()> {
    rooms.check_member(&ctx, &r)?;
    conn.join(format!("room:{}", r.0))
}
```

`Conn` 只存在于通过套接字的调用中；通过 HTTP 调用同一个 action 会返回 `422`。一个连接最多加入 `Limits::groups`（32）个群组。

## 无需 JavaScript 的屏幕

```rust
List::of(messages).on_event::<MessageSent>(OnEvent::Refresh)
Alert::info(t!("orders.title")).on_event::<OrderShipped>(OnEvent::Toast(t!("orders.shipped")))
```

`OnEvent::Refresh` 在事件到达该用户时就地刷新节点；`OnEvent::Toast` 显示一条提示（toast）。只有用户有权接收的事件才会到达。

## 浏览器代码

岛（island）和自定义脚本使用 `zeyn.socket`：

```js
const off = zeyn.socket.on('chat.message', (m) => show(m));
await zeyn.socket.call('send_message', { room, text }); // rejects with the action's problem
zeyn.socket.state; // 'connected' | 'reconnecting' | 'offline', also as zeyn:socket-state
```

可以调用通过 `Feature::api` 发布的 action。每次调用都经过与 API 调用相同的检查：权限、guard、interceptor（调用类型 `Live`）、校验、审计，以及对修改数据的 action 应用的写入限制。

## 不会断开的连接

浏览器以逐渐变长的随机间隔重新连接，恢复联网时立即重连。断开后，服务器在 `Limits::detached`（60 秒）内保留该连接的主题、群组和最近的消息，并重放页面错过的内容；无法重放时，页面会重新加载其实时部分。在代理屏蔽 WebSocket 的地方，页面会自动改为下行使用 Server-Sent Events、上行使用 `POST`，消息保持不变。

## 多台服务器

```rust
let redis = RedisStore::new("redis://cache:6379", "shop")?;
ZeynSocket::new().redis(&redis)   // features `realtime` and `redis`
```

每个实例都会把其他实例发布的内容投递给自己的连接，因此不需要粘性会话。Redis 停机期间，发送的内容只留在本实例内。

## 测试

`socket-client` feature 增加了 `TestSocket`，这是一个像浏览器一样与套接字通信的客户端：

```rust
let addr = TestSocket::serve(app.router()?).await;
let mut c = TestSocket::connect(addr, &[("authorization", "Bearer ..")]).await?;
c.call("join_room", json!({ "room": "1" })).await?;
let m: MessageSent = c.next_event().await?;
```

## 限制

| 上限 | 默认值 |
|---|---|
| 每个用户打开的套接字 | 8 |
| 每个连接等待中的消息 | 32 |
| 每个连接每秒的调用 | 20 |
| 每个连接每秒的帧 | 100 |
| 最大帧 | 64 KB |
| 最大事件 | 16 KB |
| 每个连接的重放缓冲区 | 64 条消息，64 KB |
| 断开的连接保留的时间 | 60 s |

一个空闲连接大约占用 16 KB，测量时服务器和客户端运行在同一个进程中。
