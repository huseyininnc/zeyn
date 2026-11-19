# ZeynSocket

ページごとに 1 本のライブ接続が、サーバーから送られるものをすべて運びます。screen を更新するトピック通知、ユーザー、グループ、テナント宛ての型付きイベント、そしてブラウザがページリクエストなしで呼び出す action です。`realtime` feature で有効にします。

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

`App::socket` は `Socket` サービスも登録するため、action は `socket: Socket` を受け取れます。ワイヤープロトコル、制限、その理由は `docs/zeynsocket.md` にあります。

## トピック: 更新される screen

トピックはデータを運びません。あるデータを表示している screen に対して、通常のエンドポイントとその権限チェックを通じてデータを読み込み直すよう伝えます。

```rust
Live::new("orders").add(DataGrid::of(search_orders))

#[action(permission = "sales.order.write")]
pub async fn ship(o: OrderId, store: Orders, socket: Socket, ctx: Ctx) -> Result<()> {
    store.ship(&o)?;
    socket.notify(&ctx, "orders");
    Ok(())
}
```

すべてのトピックを `ZeynSocket::topic` で、購読者に必要な権限とともに宣言します。`.listens("orders")` を付けたノードも更新されます。

## イベント: 一部のユーザー向けのデータ

イベントは、安定した名前を持つ `Serialize` 型です。受信者は全員、そのイベントが宣言された権限を持っている必要があります。グループがその権限を与えることはありません。

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

すべては 1 つのテナントの中にとどまります。イベントがマスクされた値を運ぶことはありません。id を送り、受信側に action を通じてレコードを読み込ませてください。action はいつもどおりマスクします。`Limits::event_bytes`（16 KB）より大きいイベントや宣言されていないイベントはエラーを返し、何も送りません。

## グループ

接続がグループに参加するのは、ソケット経由で呼び出された action の中だけです。その action で、アプリはユーザーがそのグループに入ってよいかをチェックします。

```rust
#[action(permission = "chat.read")]
pub async fn join_room(r: RoomId, conn: Conn, rooms: Rooms, ctx: Ctx) -> Result<()> {
    rooms.check_member(&ctx, &r)?;
    conn.join(format!("room:{}", r.0))
}
```

`Conn` はソケット経由の呼び出しにだけ存在します。同じ action を HTTP で呼び出すと `422` が返ります。1 つの接続が入れるグループは最大 `Limits::groups`（32）個です。

## JavaScript なしの screen

```rust
List::of(messages).on_event::<MessageSent>(OnEvent::Refresh)
Alert::info(t!("orders.title")).on_event::<OrderShipped>(OnEvent::Toast(t!("orders.shipped")))
```

`OnEvent::Refresh` は、イベントがこのユーザーに届いたときにノードをその場で更新します。`OnEvent::Toast` はトーストを表示します。届くのは、そのユーザーが受け取れるイベントだけです。

## ブラウザのコード

アイランドと独自のスクリプトは `zeyn.socket` を使います。

```js
const off = zeyn.socket.on('chat.message', (m) => show(m));
await zeyn.socket.call('send_message', { room, text }); // rejects with the action's problem
zeyn.socket.state; // 'connected' | 'reconnecting' | 'offline', also as zeyn:socket-state
```

`Feature::api` で公開された action を呼び出せます。各呼び出しは API 呼び出しと同じチェックを通ります。権限、guard、interceptor（呼び出し種別 `Live`）、検証、監査、そしてデータを変更する action に対するアプリの書き込み制限です。

## 切れない接続

ブラウザは、だんだん長くなるランダムな間隔で再接続し、オンラインに戻ったときはすぐに再接続します。切断後、サーバーは接続のトピック、グループ、直近のメッセージを `Limits::detached`（60 秒）の間保持し、ページが受け取れなかったものを再送します。それができない場合、ページはライブ部分を読み込み直します。プロキシが WebSocket をブロックする環境では、ページは自動的に下りを Server-Sent Events、上りを `POST` に切り替え、同じメッセージを使います。

## 複数のサーバー

```rust
let redis = RedisStore::new("redis://cache:6379", "shop")?;
ZeynSocket::new().redis(&redis)   // features `realtime` and `redis`
```

各インスタンスは、他のインスタンスが発行したものを自分の接続に届けるため、スティッキーセッションは不要です。Redis が停止している間、送信はそのインスタンス内にとどまります。

## テスト

`socket-client` feature は、ブラウザと同じようにソケットと通信するクライアント `TestSocket` を追加します。

```rust
let addr = TestSocket::serve(app.router()?).await;
let mut c = TestSocket::connect(addr, &[("authorization", "Bearer ..")]).await?;
c.call("join_room", json!({ "room": "1" })).await?;
let m: MessageSent = c.next_event().await?;
```

## 制限

| 上限 | デフォルト |
|---|---|
| ユーザーごとの開いているソケット | 8 |
| 接続ごとの待機メッセージ | 32 |
| 接続ごと 1 秒あたりの呼び出し | 20 |
| 接続ごと 1 秒あたりのフレーム | 100 |
| 最大フレーム | 64 KB |
| 最大イベント | 16 KB |
| 接続ごとの再送バッファ | 64 メッセージ、64 KB |
| 切断された接続を保持する時間 | 60 s |

アイドル状態の接続のコストは約 16 KB です（サーバーとクライアントを 1 つのプロセスで動かして測定）。
