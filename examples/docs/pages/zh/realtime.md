# 实时通道

Zeyn 的请求是无状态的：服务器不保留套接字，也不保存按用户划分的屏幕状态。有些屏幕需要更多：两个人编辑同一张图表，或者当别人保存时应当更新的列表。`realtime` Cargo feature 为此增加了一个需主动启用的 WebSocket 通道。不启用它的应用不会编译其中任何内容，不使用它的页面也不会加载任何东西。

企业网络经常屏蔽 WebSocket，因此请把该通道视为一种增强：每个屏幕在没有它时也必须能正常工作（图表编辑器仍通过其 action 保存；列表在用户再次打开时仍会刷新）。

## 1. 启用

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

## 2. 端点

| 端点 | 用途 |
|---|---|
| `GET /_zeyn/live/doc/{kind}/{doc}?csrf=TOKEN` | WebSocket：协作文档 |
| `GET /_zeyn/live/topics?t=a,b&csrf=TOKEN` | WebSocket：主题（topic）的变更通知 |
| `GET /_zeyn/live.js` | 主题脚本，只被订阅主题的页面链接 |

每次握手在升级之前都会按以下顺序检查：

1. 通道已开启（否则返回 `404`）。
2. **Origin：** `Origin` 请求头必须是请求所到达的主机，或是通过 `Realtime::allow_origin` 添加的源。浏览器在 WebSocket 握手时总会发送它；缺失则被拒绝。这正是阻止其他站点携带用户 cookie 打开套接字的机制。
3. **CSRF 令牌：** 浏览器无法在 WebSocket 上设置请求头，因此来自 `meta[name=zeyn-csrf]` 的令牌放在 `csrf` 查询参数中，并以常量时间与 `zeyn_csrf` cookie 比较，与 `POST` 使用 `x-zeyn-csrf` 的方式完全一样。失败时返回 `403`。
4. **身份：** 使用与每个请求相同的认证器；用户必须已登录（否则 `403`）。
5. **每用户限制**（`429`）。
6. 文档：必须是已声明的 kind 和普通 id（1 到 64 个字母、数字、`-`、`_`、`.`），否则 `404`；还需具备该 kind 的读取权限（`403`）。主题：每个主题都已声明且用户持有其权限（`403`）。
7. **拦截器：** 应用的拦截器会以 `Live` 类型的调用运行（`screen` 是文档 kind，主题则为空），因此应用范围的规则，如 `RequireMfa` 或账户停用检查，同样覆盖实时通道。被拒绝时返回其状态码。
8. 房间限制：内存中的房间数和每个房间的客户端数（`503`，带 `Retry-After`）。

写入权限每个连接检查一次：没有该权限的客户端可以接收更新，但在它发送更新的瞬间就会以 `1008` 被关闭。

## 3. 文档与 Y-CRDT

文档是由 `yrs` 持有的 [Y-CRDT](https://docs.yjs.dev) 文档。房间以**租户、kind 和文档 id** 为键，因此即使 id 相同，两个租户也绝不会共享同一个房间。

帧是二进制的。第一个字节是标签；`1`（`FRAME_UPDATE`）携带 v1 编码的 Y 更新。

- 连接时，服务器把整个文档作为一次更新发送。
- 客户端为自己的更改发送更新；服务器应用每一个更新（不是有效更新的帧会使套接字以 `1008` 关闭），并把同样的字节转发给房间中的其他所有客户端。
- 每次（重新）连接之后，island 会把它的完整状态发回，因此离线时所做的编辑会被合并。Y 更新是幂等且可交换的，所以顺序和重复都无关紧要。

持久化通过一个 trait 完成：

```rust
pub trait DocStore: Send + Sync + 'static {
    fn load(&self, key: RoomKey) -> BoxFuture<Result<Option<Vec<u8>>>>;
    fn save(&self, key: RoomKey, state: Vec<u8>) -> BoxFuture<Result<()>>;
}
```

`load` 在房间打开时运行（出错时连接会被以 `503` 拒绝，存储也借此限制哪些文档存在）。`save` 以一次更新的形式接收整个文档，在文档有变更期间每隔 `persist_every` 运行一次，并在空闲房间被丢弃之前运行。保存失败会在下一轮重试，并保留房间。`MemoryDocStore` 用于测试和演示。

在 island 端，启用 `realtime` feature 的 `zeyn-island` 提供 `live::Channel`：

```rust
let channel = Channel::document("diagram", "main", on_open, on_update, on_state)?;
channel.send_update(&update);   // after a local change
channel.close();                // in unmount
```

它在 1 秒后重连，间隔翻倍直至 30 秒，并在 `close` 或收到 `1008` 之后停止。

## 4. 主题：就地刷新的屏幕

```rust
Live::new("orders").add(DataGrid::of(search_orders)...)

#[action(permission = "sales.order.write")]
pub async fn save_order(o: Valid<Order>, store: Orders, hub: Hub, ctx: Ctx) -> Result<Order> {
    let saved = store.save(o.into_inner())?;
    hub.notify(&ctx, "orders");
    Ok(saved)
}
```

`Live` 渲染为 `div[data-zeyn-live="orders"]`。屏幕中包含它的页面会链接 `/_zeyn/live.js`（通过 `nav.js` 到达的屏幕会带上 `x-zeyn-live: 1`，脚本在那时被导入）。该脚本为页面上的所有主题打开一个套接字。收到通知时，每个已订阅的元素会就地重新加载其表格（`zeyn:reload`，保持表格当前的页码、排序和筛选），若其中没有表格，则就地刷新整个屏幕（`zeyn:refresh`）。300 毫秒内的通知只处理一次。

通知只携带主题名称，绝不携带数据：刷新走的是正常的表格端点，并带有所有权限检查。`notify` 只到达调用者所在的租户；`notify_tenant(Some(id), topic)` 指定某一个租户，`None` 表示没有租户机制的应用。

## 5. 内存与背压

每一份状态都有上限（`Limits`，括号中为默认值）：

| 上限 | 默认值 | 达到时 |
|---|---|---|
| `per_user` 套接字 | 8 | 握手返回 `429` |
| `per_room` 客户端 | 32 | 握手返回 `503` |
| 内存中的 `rooms` | 256 | 握手返回 `503` |
| 来自客户端的 `message_bytes` | 64 KB | 套接字被关闭 |
| 每个文档的 `doc_bytes` | 1 MB | 更新被拒绝且套接字被关闭（`1009`） |
| 每个客户端的 `queue` | 32 条消息 | 客户端被移出房间，并以 `1013` 关闭 |
| 没有客户端的 `idle` 房间 | 30 秒 | 被持久化并从内存中丢弃 |
| `heartbeat` | 20 秒 | 发送 ping；沉默 3 个间隔的客户端会被关闭 |
| `persist_every` | 2 秒 | 保存有变更的文档 |

广播从不等待：更新通过 `try_send` 提交到每个客户端的有界队列。队列已满的客户端会立即被移出房间，因此一个慢速读取者无法占用内存或拖慢其他人；它会重新连接并收到完整文档。队列已满的主题订阅者只会错过那一次通知（下一次通知反正会刷新它）。

因此最坏情况下的内存约为：文档为 `rooms * doc_bytes`，加上队列为 `clients * queue * message_bytes`（更新在队列之间共享，而不是复制）。在 showcase 服务器上，图表打开两个实时标签页时总共使用约 19 MB（debug 构建）。`Hub::stats()` 报告房间数、客户端数、订阅者数和文档字节数，供监控使用。

## 6. 安全说明

- 通道从不运行 action：文档是应用通过其 `DocStore` 读取的数据，应用在那里对其进行校验（showcase 用 `Diagram::clamped` 限制每个保存的图表）。
- 租户中任何拥有该 kind 读取权限的人都可以读取一个文档；对于受众不同的文档，请使用不同的 kind（或存储会拒绝的 id）。
- 每个页面的策略都通过 `default-src 'self'` 允许向应用自身的源建立 WebSocket；内容安全策略中没有为该通道放宽任何内容。

## 7. 示例

`examples/showcase`：在两个标签页中打开 Islands、图表编辑器。在一个标签页中移动或添加图形，另一个标签页中会随之显示，并且当文档通过 `DocStore` 保存且 `diagram` 主题被通知时，下方的表格“Shapes stored on the server”会在大约两秒内就地刷新。

当前版本的限制：房间存在于一个进程中（多个服务器实例需要粘性会话或共享的消息代理），未内置在线状态（谁也在这里、光标），并且每次连接都会完整发送文档。
