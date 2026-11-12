# ZeynSocket

Одно живое соединение на страницу несёт всё, что отправляет сервер: уведомления тем, которые
обновляют экраны, типизированные события для пользователя, группы или тенанта, а также действия,
которые браузер вызывает без запроса страницы. Включается feature `realtime`:

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

`App::socket` также регистрирует сервис `Socket`, поэтому действия принимают `socket: Socket`.
Протокол передачи, ограничения и их причины описаны в `docs/zeynsocket.md`.

## Темы: экраны, которые обновляются

Тема не несёт данных. Она сообщает экранам, которые показывают какие-то данные, что их нужно
загрузить снова через обычные конечные точки с их проверками прав.

```rust
Live::new("orders").add(DataGrid::of(search_orders))

#[action(permission = "sales.order.write")]
pub async fn ship(o: OrderId, store: Orders, socket: Socket, ctx: Ctx) -> Result<()> {
    store.ship(&o)?;
    socket.notify(&ctx, "orders");
    Ok(())
}
```

Объявляйте каждую тему через `ZeynSocket::topic` вместе с правом, которое нужно подписчику. Узлы,
помеченные `.listens("orders")`, тоже обновляются.

## События: данные для некоторых пользователей

Событие это тип `Serialize` с постоянным именем. Каждый получатель должен иметь право, с которым
событие было объявлено; группа никогда его не даёт.

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

Всё остаётся внутри одного тенанта. События никогда не несут маскированных значений: отправьте id,
и пусть получатель загрузит запись через действие, которое маскирует как обычно. Событие больше
`Limits::event_bytes` (16 KB) или не объявленное вовсе возвращает ошибку и ничего не отправляет.

## Группы

Соединение вступает в группу только внутри действия, вызванного через сокет. Именно в этом
действии приложение проверяет, может ли пользователь состоять в группе.

```rust
#[action(permission = "chat.read")]
pub async fn join_room(r: RoomId, conn: Conn, rooms: Rooms, ctx: Ctx) -> Result<()> {
    rooms.check_member(&ctx, &r)?;
    conn.join(format!("room:{}", r.0))
}
```

`Conn` существует только при вызовах через сокет; то же действие, вызванное по HTTP, отвечает
`422`. Соединение состоит не более чем в `Limits::groups` (32) группах.

## Экраны без JavaScript

```rust
List::of(messages).on_event::<MessageSent>(OnEvent::Refresh)
Alert::info(t!("orders.title")).on_event::<OrderShipped>(OnEvent::Toast(t!("orders.shipped")))
```

`OnEvent::Refresh` обновляет узел на месте, когда событие доходит до этого пользователя;
`OnEvent::Toast` показывает всплывающее уведомление. Приходят только те события, которые
пользователь вправе получать.

## Код в браузере

Острова и собственные скрипты используют `zeyn.socket`:

```js
const off = zeyn.socket.on('chat.message', (m) => show(m));
await zeyn.socket.call('send_message', { room, text }); // rejects with the action's problem
zeyn.socket.state; // 'connected' | 'reconnecting' | 'offline', also as zeyn:socket-state
```

Можно вызывать действия, опубликованные через `Feature::api`. Каждый вызов проходит те же
проверки, что и вызов API: право, guards, interceptors (вид вызова `Live`), валидацию, аудит и
лимит записи приложения для действий, которые меняют данные.

## Соединение, которое не обрывается

Браузер переподключается с растущими случайными паузами и сразу, как только снова появляется сеть.
После обрыва сервер хранит темы, группы и последние сообщения соединения в течение
`Limits::detached` (60 секунд) и повторно отправляет то, что страница пропустила; если это
невозможно, страница перезагружает свои живые части. Там, где прокси блокируют WebSocket,
страница сама переходит на Server-Sent Events для входящих данных и `POST` для исходящих, с теми
же сообщениями.

## Несколько серверов

```rust
let redis = RedisStore::new("redis://cache:6379", "shop")?;
ZeynSocket::new().redis(&redis)   // features `realtime` and `redis`
```

Каждый экземпляр доставляет своим соединениям то, что публикуют другие, поэтому привязка сессий
(sticky sessions) не нужна. Пока Redis недоступен, отправки остаются внутри экземпляра.

## Тесты

Feature `socket-client` добавляет `TestSocket`, клиент, который общается с сокетом как браузер:

```rust
let addr = TestSocket::serve(app.router()?).await;
let mut c = TestSocket::connect(addr, &[("authorization", "Bearer ..")]).await?;
c.call("join_room", json!({ "room": "1" })).await?;
let m: MessageSent = c.next_event().await?;
```

## Ограничения

| Ограничение | По умолчанию |
|---|---|
| Открытых сокетов на пользователя | 8 |
| Ожидающих сообщений на соединение | 32 |
| Вызовов на соединение в секунду | 20 |
| Кадров на соединение в секунду | 100 |
| Самый большой кадр | 64 KB |
| Самое большое событие | 16 KB |
| Буфер повтора на соединение | 64 сообщения, 64 KB |
| Сколько хранится оборванное соединение | 60 s |

Простаивающее соединение стоит около 16 KB, если измерять с сервером и клиентом в одном процессе.
