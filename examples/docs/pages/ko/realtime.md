# 실시간 채널

Zeyn의 요청은 상태가 없습니다. 서버는 소켓도 사용자별 Screen 상태도 보관하지 않습니다. 일부 Screen에는 그 이상이 필요합니다. 한 다이어그램을 두 사람이 편집하거나, 다른 사람이 저장하면 갱신되어야 하는 목록이 그렇습니다. `realtime` Cargo 기능은 이를 위해 선택적으로 사용하는 WebSocket 채널을 추가합니다. 이를 켜지 않은 앱은 이 중 아무것도 컴파일하지 않으며, 이를 사용하지 않는 페이지는 아무것도 로드하지 않습니다.

WebSocket은 기업 네트워크에서 자주 차단되므로 채널은 개선 사항으로 취급하십시오. 모든 Screen은 채널 없이도 여전히 동작해야 합니다(다이어그램 편집기는 여전히 action으로 저장하며, 목록은 사용자가 다시 열면 여전히 새로 고쳐집니다).

## 1. 켜기

```toml
zeyn = { version = "...", features = ["realtime"] }
```

```rust
let live = ZeynSocket::new()
    .documents("diagram", "diagram.read", "diagram.write", MyDocStore::new(pool))
    .topic("orders", "sales.order.read")
    .limits(Limits::default());
let socket = live.socket();       // clone it into stores that notify
App::new(..).socket(live)         // also registers `Socket` as a service
```

## 2. 엔드포인트

| 엔드포인트 | 용도 |
|---|---|
| `GET /_zeyn/live/doc/{kind}/{doc}?csrf=TOKEN` | WebSocket: 협업 문서 |
| `GET /_zeyn/socket` | WebSocket: 토픽의 변경 알림 |
| `GET /_zeyn/live.js` | 토픽 스크립트이며 구독하는 페이지만 링크합니다 |

모든 핸드셰이크는 업그레이드 전에 다음 순서로 검사됩니다.

1. 채널이 켜져 있어야 합니다(아니면 `404`).
2. **Origin:** `Origin` 헤더는 요청이 도달한 호스트 또는 `ZeynSocket::allow_origin`으로 추가한 출처를 가리켜야 합니다. 브라우저는 WebSocket 핸드셰이크에서 항상 이를 보내며, 없으면 거부됩니다. 이것이 다른 사이트가 사용자의 쿠키로 소켓을 여는 것을 막습니다.
3. **CSRF 토큰:** 브라우저는 WebSocket에 헤더를 설정할 수 없으므로 `meta[name=zeyn-csrf]`의 토큰이 `csrf` 쿼리 매개변수로 들어가며, `POST`에서 `x-zeyn-csrf`가 그러듯이 `zeyn_csrf` 쿠키와 일정한 시간으로 비교됩니다. 실패하면 `403`으로 응답합니다.
4. **ID:** 모든 요청과 같은 인증기를 사용합니다. 사용자는 로그인되어 있어야 합니다(`403`).
5. **사용자별 제한** (`429`).
6. 문서: 선언된 kind와 일반 id(문자, 숫자, `-`, `_`, `.` 1자에서 64자)가 아니면 `404`. kind의 읽기 권한(`403`). 토픽: 모든 토픽이 선언되어 있고 그 권한을 가지고 있어야 합니다(`403`).
7. **인터셉터:** 앱의 인터셉터가 종류 `Live`의 호출로 실행됩니다(`screen`은 문서 kind이며 토픽에서는 비어 있음). 따라서 `RequireMfa`나 정지된 계정 검사 같은 앱 전체 규칙이 실시간 채널에도 적용됩니다. 거부는 해당 상태로 응답합니다.
8. 룸 제한: 메모리의 룸과 룸당 클라이언트(`Retry-After`와 함께 `503`).

쓰기 권한은 연결당 한 번 검사됩니다. 권한이 없는 클라이언트는 업데이트를 받지만, 업데이트를 보내는 순간 `1008`로 닫힙니다.

## 3. 문서와 Y-CRDT

문서는 `yrs`로 보관하는 [Y-CRDT](https://docs.yjs.dev) 문서입니다. 룸은 **테넌트, kind, 문서 id**를 키로 하므로 두 테넌트는 같은 id를 써도 룸을 공유하지 않습니다.

프레임은 바이너리입니다. 첫 바이트는 태그이며, `1`(`FRAME_UPDATE`)은 v1 인코딩의 Y 업데이트를 담습니다.

- 연결하면 서버가 문서 전체를 하나의 업데이트로 보냅니다.
- 클라이언트는 자신의 변경에 대한 업데이트를 보내며, 서버는 각각을 적용하고(유효한 업데이트가 아닌 프레임은 `1008`로 소켓을 닫음) 같은 바이트를 룸의 다른 모든 클라이언트에 전달합니다.
- (재)연결할 때마다 아일랜드가 자신의 전체 상태를 다시 보내므로 오프라인에서 한 편집도 병합됩니다. Y 업데이트는 멱등적이고 교환 가능하므로 순서와 반복은 문제가 되지 않습니다.

영속화는 트레이트를 거칩니다.

```rust
pub trait DocStore: Send + Sync + 'static {
    fn load(&self, key: RoomKey) -> BoxFuture<Result<Option<Vec<u8>>>>;
    fn save(&self, key: RoomKey, state: Vec<u8>) -> BoxFuture<Result<()>>;
}
```

`load`는 룸이 열릴 때 실행됩니다(오류는 `503`으로 연결을 거부하며, 이는 스토어가 존재하는 문서를 제한하는 방법이기도 합니다). `save`는 문서 전체를 하나의 업데이트로 받으며, 문서가 변경된 동안 `persist_every`마다, 그리고 유휴 룸이 버려지기 전에 실행됩니다. 실패한 저장은 다음 라운드에 다시 시도되고 룸은 유지됩니다. `MemoryDocStore`는 테스트와 데모용입니다.

아일랜드 측에서 `realtime` 기능을 사용하는 `zeyn-island`는 `live::Channel`을 제공합니다.

```rust
let channel = Channel::document("diagram", "main", on_open, on_update, on_state)?;
channel.send_update(&update);   // after a local change
channel.close();                // in unmount
```

1초 후에 다시 연결하며 최대 30초까지 두 배로 늘리고, `close` 또는 `1008` 후에는 멈춥니다.

## 4. 토픽: 제자리에서 새로 고쳐지는 Screen

```rust
Live::new("orders").add(DataGrid::of(search_orders)...)

#[action(permission = "sales.order.write")]
pub async fn save_order(o: Valid<Order>, store: Orders, socket: Socket, ctx: Ctx) -> Result<Order> {
    let saved = store.save(o.into_inner())?;
    socket.notify(&ctx, "orders");
    Ok(saved)
}
```

`Live`는 `div[data-zeyn-live="orders"]`를 렌더링합니다. Screen에 이것이 있는 페이지는 `/_zeyn/live.js`를 링크합니다(`nav.js`를 통해 도착하는 Screen은 `x-zeyn-live: 1`을 받고 그때 스크립트를 가져옵니다). 스크립트는 페이지의 모든 토픽에 대해 소켓 하나를 엽니다. 알림이 오면 구독한 각 요소는 자신의 그리드를 제자리에서 다시 불러오거나(`zeyn:reload`, 그리드의 현재 페이지, 정렬, 필터), 그리드가 없으면 Screen 전체를 제자리에서 새로 고칩니다(`zeyn:refresh`). 300 ms 이내의 알림은 한 번만 처리됩니다.

알림은 토픽 이름만 담고 데이터는 절대 담지 않습니다. 새로 고침은 모든 권한 검사가 있는 일반 그리드 엔드포인트를 거칩니다. `notify`는 호출자의 테넌트에만 도달하며, `notify_tenant(Some(id), topic)`은 하나를 지정하고 `None`은 테넌시가 없는 앱입니다.

같은 토픽 이름은 채널 없이도 동작합니다. `.listens("orders")`로 표시한 노드는 알림이 토픽을 지정할 때(`live.js`가 이를 `zeyn:topics`로 전달합니다) 제자리에서 새로 고쳐지며, action이 `ui.refresh("orders")`로 끝나거나 `.drives("orders")`를 가진 필터가 바뀔 때도 새로 고쳐집니다. 그리드는 현재 페이지를 다시 불러오고, 다른 리스너는 Screen마다 한 번의 `POST /_zeyn/nodes`로 다시 렌더링됩니다(`examples/docs/pages/effects.md`의 Topics 참고). 소켓은 `ZeynSocket::topic`으로 선언해야 하는 `Live` 노드의 토픽만 구독합니다. 그리드가 아닌 부분에는 그 안이나 옆에서 `.listens`를 쓰세요.

## 5. 메모리와 백프레셔

모든 상태에는 제한이 있습니다(`Limits`, 괄호 안은 기본값).

| 제한 | 기본값 | 도달했을 때 |
|---|---|---|
| `per_user` 소켓 | 8 | 핸드셰이크 `429` |
| `per_room` 클라이언트 | 32 | 핸드셰이크 `503` |
| 메모리의 `rooms` | 256 | 핸드셰이크 `503` |
| 클라이언트의 `message_bytes` | 64 KB | 소켓 닫힘 |
| 문서당 `doc_bytes` | 1 MB | 업데이트가 거부되고 소켓이 닫힘(`1009`) |
| 클라이언트당 `queue` | 메시지 32개 | 클라이언트가 룸에서 제거되고 `1013`으로 닫힘 |
| 클라이언트가 없는 `idle` 룸 | 30초 | 영속화되고 메모리에서 제거됨 |
| `heartbeat` | 20초 | ping. 3번의 간격 동안 조용한 클라이언트는 닫힘 |
| `persist_every` | 2초 | 변경된 문서가 저장됨 |

브로드캐스트는 절대 기다리지 않습니다. 업데이트는 `try_send`로 각 클라이언트의 제한된 큐에 제공됩니다. 큐가 가득 찬 클라이언트는 즉시 룸에서 제거되므로 느린 독자 하나가 메모리를 붙잡거나 다른 클라이언트를 느리게 할 수 없습니다. 그 클라이언트는 다시 연결하여 문서 전체를 받습니다. 큐가 가득 찬 토픽 구독자는 그 알림만 놓치며(다음 알림이 어차피 새로 고칩니다), 최악의 경우 메모리는 문서에 대해 약 `rooms * doc_bytes`, 큐에 대해 `clients * queue * message_bytes`입니다(업데이트는 큐 사이에서 복사되지 않고 공유됩니다). 다이어그램에 라이브 탭 두 개가 있는 쇼케이스 서버는 전체적으로 약 19 MB를 사용합니다(디버그 빌드). `Socket::stats()`는 모니터링을 위해 룸, 클라이언트, 구독자, 문서 바이트를 보고합니다.

## 6. 보안 참고 사항

- 채널은 action을 실행하지 않습니다. 문서는 앱이 `DocStore`를 통해 읽는 데이터이며 앱은 그곳에서 이를 검증합니다(쇼케이스는 저장되는 모든 다이어그램을 `Diagram::clamped`로 제한합니다).
- 문서는 테넌트에서 그 kind의 읽기 권한이 있는 모든 사람이 읽을 수 있습니다. 대상이 다른 문서에는 별도의 kind(또는 스토어가 거부하는 id)를 사용하십시오.
- 모든 페이지의 정책은 `default-src 'self'`를 통해 앱 자체의 출처로의 WebSocket을 허용합니다. 채널을 위해 콘텐츠 보안 정책에서 완화되는 것은 없습니다.

## 7. 예제

`examples/showcase`: Islands, Diagram editor를 탭 두 개로 여십시오. 한 탭에서 도형을 이동하거나 추가하면 다른 탭에 나타나고, 문서가 `DocStore`를 통해 저장되어 `diagram` 토픽이 알려지면 아래의 "Shapes stored on the server" 그리드가 약 2초 안에 제자리에서 새로 고쳐집니다.

현재 버전의 제한: 룸은 하나의 프로세스에 있고(여러 서버 인스턴스에는 스티키 세션이나 공유 브로커가 필요함), 프레즌스(다른 누가 있는지, 커서)는 내장되어 있지 않으며, 문서는 연결할 때마다 전체가 전송됩니다.
