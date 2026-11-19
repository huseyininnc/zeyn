# ZeynSocket

페이지마다 하나의 실시간 연결이 서버가 보내는 모든 것을 전달합니다. Screen을 새로 고치는 토픽 알림, 한 사용자나 그룹 또는 테넌트를 위한 타입이 있는 이벤트, 그리고 브라우저가 페이지 요청 없이 호출하는 action입니다. `realtime` 기능으로 켭니다.

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

`App::socket`은 `Socket` 서비스도 등록하므로 action은 `socket: Socket`을 받을 수 있습니다. 전송 프로토콜, 제한과 그 이유는 `docs/zeynsocket.md`에 있습니다.

## 토픽: 새로 고쳐지는 Screen

토픽은 데이터를 전달하지 않습니다. 어떤 데이터를 보여 주는 Screen에게 일반 엔드포인트와 그 권한 검사를 거쳐 데이터를 다시 불러오라고 알립니다.

```rust
Live::new("orders").add(DataGrid::of(search_orders))

#[action(permission = "sales.order.write")]
pub async fn ship(o: OrderId, store: Orders, socket: Socket, ctx: Ctx) -> Result<()> {
    store.ship(&o)?;
    socket.notify(&ctx, "orders");
    Ok(())
}
```

모든 토픽은 `ZeynSocket::topic`으로, 구독자에게 필요한 권한과 함께 선언하십시오. `.listens("orders")`로 표시한 노드도 새로 고쳐집니다.

## 이벤트: 일부 사용자를 위한 데이터

이벤트는 고정된 이름을 가진 `Serialize` 타입입니다. 모든 수신자는 이벤트를 선언할 때 지정한 권한을 가지고 있어야 하며, 그룹이 이 권한을 부여하는 일은 없습니다.

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

모든 것은 하나의 테넌트 안에 머뭅니다. 이벤트는 마스킹된 값을 절대 전달하지 않습니다. id를 보내고, 수신자가 평소처럼 마스킹하는 action을 통해 레코드를 불러오게 하십시오. `Limits::event_bytes`(16 KB)보다 크거나 선언되지 않은 이벤트는 오류를 반환하고 아무것도 보내지 않습니다.

## 그룹

연결은 소켓을 통해 호출된 action 안에서만 그룹에 참여합니다. 앱은 바로 그 action에서 사용자가 그룹에 속해도 되는지 검사합니다.

```rust
#[action(permission = "chat.read")]
pub async fn join_room(r: RoomId, conn: Conn, rooms: Rooms, ctx: Ctx) -> Result<()> {
    rooms.check_member(&ctx, &r)?;
    conn.join(format!("room:{}", r.0))
}
```

`Conn`은 소켓을 통한 호출에서만 존재합니다. 같은 action을 HTTP로 호출하면 `422`로 응답합니다. 하나의 연결은 최대 `Limits::groups`(32)개의 그룹에 속합니다.

## JavaScript 없는 Screen

```rust
List::of(messages).on_event::<MessageSent>(OnEvent::Refresh)
Alert::info(t!("orders.title")).on_event::<OrderShipped>(OnEvent::Toast(t!("orders.shipped")))
```

`OnEvent::Refresh`는 이벤트가 이 사용자에게 도착하면 노드를 제자리에서 새로 고칩니다. `OnEvent::Toast`는 토스트를 표시합니다. 사용자가 받을 수 있는 이벤트만 도착합니다.

## 브라우저 코드

아일랜드와 직접 작성한 스크립트는 `zeyn.socket`을 사용합니다.

```js
const off = zeyn.socket.on('chat.message', (m) => show(m));
await zeyn.socket.call('send_message', { room, text }); // rejects with the action's problem
zeyn.socket.state; // 'connected' | 'reconnecting' | 'offline', also as zeyn:socket-state
```

`Feature::api`로 공개된 action을 호출할 수 있습니다. 각 호출은 API 호출과 같은 검사를 거칩니다. 권한, guard, interceptor(호출 종류 `Live`), 유효성 검사, 감사, 그리고 데이터를 변경하는 action에 대한 앱의 쓰기 제한입니다.

## 끊기지 않는 연결

브라우저는 점점 길어지는 무작위 간격으로 다시 연결하며, 다시 온라인이 되면 즉시 연결합니다. 연결이 끊긴 뒤 서버는 연결의 토픽, 그룹, 최근 메시지를 `Limits::detached`(60초) 동안 보관하고 페이지가 놓친 것을 다시 보냅니다. 그것이 불가능하면 페이지는 실시간 부분을 다시 불러옵니다. 프록시가 WebSocket을 차단하는 곳에서는 페이지가 스스로 받는 쪽은 Server-Sent Events로, 보내는 쪽은 `POST`로 전환하며 메시지는 같습니다.

## 여러 서버

```rust
let redis = RedisStore::new("redis://cache:6379", "shop")?;
ZeynSocket::new().redis(&redis)   // features `realtime` and `redis`
```

각 인스턴스는 다른 인스턴스가 발행한 것을 자신의 연결에 전달하므로 스티키 세션이 필요하지 않습니다. Redis가 중단된 동안에는 보내는 내용이 해당 인스턴스 안에 머뭅니다.

## 테스트

`socket-client` 기능은 브라우저처럼 소켓과 통신하는 클라이언트인 `TestSocket`을 추가합니다.

```rust
let addr = TestSocket::serve(app.router()?).await;
let mut c = TestSocket::connect(addr, &[("authorization", "Bearer ..")]).await?;
c.call("join_room", json!({ "room": "1" })).await?;
let m: MessageSent = c.next_event().await?;
```

## 제한

| 한도 | 기본값 |
|---|---|
| 사용자별 열린 소켓 | 8 |
| 연결별 대기 메시지 | 32 |
| 연결별 초당 호출 | 20 |
| 연결별 초당 프레임 | 100 |
| 가장 큰 프레임 | 64 KB |
| 가장 큰 이벤트 | 16 KB |
| 연결별 재전송 버퍼 | 64개 메시지, 64 KB |
| 끊긴 연결을 보관하는 시간 | 60 s |

유휴 연결의 비용은 약 16 KB이며, 서버와 클라이언트를 한 프로세스에서 실행해 측정했습니다.
