# ZeynSocket

Sayfa başına tek bir canlı bağlantı, sunucunun gönderdiği her şeyi taşır: ekranları yenileyen
topic bildirimleri, bir kullanıcıya, bir gruba ya da bir tenant'a giden tipli olaylar ve
tarayıcının sayfa isteği olmadan çağırdığı action'lar. `realtime` feature'ı ile açılır:

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

`App::socket`, `Socket` servisini de kaydeder, böylece action'lar `socket: Socket` alabilir. Kablo
protokolü, sınırlar ve bunların gerekçeleri `docs/zeynsocket.md` içindedir.

## Topic'ler: yenilenen ekranlar

Bir topic veri taşımaz. Bazı verileri gösteren ekranlara bu verileri, normal endpoint'ler ve
onların izin denetimleri üzerinden yeniden yüklemelerini söyler.

```rust
Live::new("orders").add(DataGrid::of(search_orders))

#[action(permission = "sales.order.write")]
pub async fn ship(o: OrderId, store: Orders, socket: Socket, ctx: Ctx) -> Result<()> {
    store.ship(&o)?;
    socket.notify(&ctx, "orders");
    Ok(())
}
```

Her topic'i `ZeynSocket::topic` ile ve bir abonenin sahip olması gereken izinle tanımlayın.
`.listens("orders")` ile işaretlenen düğümler de yenilenir.

## Olaylar: bazı kullanıcılar için veri

Bir olay, sabit bir adı olan `Serialize` bir tiptir. Her alıcı, olayın tanımlandığı izne sahip
olmalıdır; bir grup bu izni hiçbir zaman vermez.

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

Her şey tek bir tenant içinde kalır. Olaylar hiçbir zaman maskelenmiş değer taşımaz: bir id
gönderin ve alıcının kaydı, her zamanki gibi maskeleyen bir action üzerinden yüklemesine izin
verin. `Limits::event_bytes` (16 KB) sınırını aşan ya da hiç tanımlanmamış bir olay hata döner ve
hiçbir şey göndermez.

## Gruplar

Bir bağlantı bir gruba yalnızca soket üzerinden çağrılan bir action içinde katılır. Kullanıcının
grupta olup olamayacağını uygulama bu action'da denetler.

```rust
#[action(permission = "chat.read")]
pub async fn join_room(r: RoomId, conn: Conn, rooms: Rooms, ctx: Ctx) -> Result<()> {
    rooms.check_member(&ctx, &r)?;
    conn.join(format!("room:{}", r.0))
}
```

`Conn` yalnızca soket üzerinden yapılan çağrılarda vardır; aynı action HTTP üzerinden çağrılırsa
`422` döner. Bir bağlantı en fazla `Limits::groups` (32) grupta bulunur.

## JavaScript'siz ekranlar

```rust
List::of(messages).on_event::<MessageSent>(OnEvent::Refresh)
Alert::info(t!("orders.title")).on_event::<OrderShipped>(OnEvent::Toast(t!("orders.shipped")))
```

`OnEvent::Refresh`, olay bu kullanıcıya ulaştığında düğümü yerinde yeniler;
`OnEvent::Toast` bir bildirim (toast) gösterir. Yalnızca kullanıcının alabileceği olaylar gelir.

## Tarayıcı kodu

Island'lar ve özel script'ler `zeyn.socket` kullanır:

```js
const off = zeyn.socket.on('chat.message', (m) => show(m));
await zeyn.socket.call('send_message', { room, text }); // rejects with the action's problem
zeyn.socket.state; // 'connected' | 'reconnecting' | 'offline', also as zeyn:socket-state
```

`Feature::api` ile yayımlanan action'lar çağrılabilir. Her çağrı bir API çağrısıyla aynı
denetimlerden geçer: izin, guard'lar, interceptor'lar (çağrı türü `Live`), doğrulama, denetim
kaydı (audit) ve veri değiştiren action'lar için uygulamanın yazma sınırı.

## Kopmayan bir bağlantı

Tarayıcı, giderek uzayan rastgele aralıklarla ve yeniden çevrimiçi olduğu anda hemen yeniden
bağlanır. Bir kopmadan sonra sunucu bağlantının topic'lerini, gruplarını ve son mesajlarını
`Limits::detached` (60 saniye) boyunca tutar ve sayfanın kaçırdıklarını yeniden oynatır; bu mümkün
olmadığında sayfa canlı bölümlerini yeniden yükler. Proxy'lerin WebSocket'i engellediği yerlerde
sayfa kendiliğinden aşağı yönde Server-Sent Events'e ve yukarı yönde `POST`'a geçer, mesajlar aynı
kalır.

## Birden çok sunucu

```rust
let redis = RedisStore::new("redis://cache:6379", "shop")?;
ZeynSocket::new().redis(&redis)   // features `realtime` and `redis`
```

Her örnek (instance), diğerlerinin yayımladıklarını kendi bağlantılarına iletir, bu yüzden sticky
session gerekmez. Redis çalışmadığı sürece gönderimler örneğin içinde kalır.

## Testler

`socket-client` feature'ı, sokete bir tarayıcı gibi konuşan bir istemci olan `TestSocket`'i ekler:

```rust
let addr = TestSocket::serve(app.router()?).await;
let mut c = TestSocket::connect(addr, &[("authorization", "Bearer ..")]).await?;
c.call("join_room", json!({ "room": "1" })).await?;
let m: MessageSent = c.next_event().await?;
```

## Sınırlar

| Sınır | Varsayılan |
|---|---|
| Kullanıcı başına açık soket | 8 |
| Bağlantı başına bekleyen mesaj | 32 |
| Bağlantı başına saniyede çağrı | 20 |
| Bağlantı başına saniyede çerçeve (frame) | 100 |
| En büyük çerçeve | 64 KB |
| En büyük olay | 16 KB |
| Bağlantı başına yeniden oynatma tamponu | 64 mesaj, 64 KB |
| Kopan bir bağlantının tutulma süresi | 60 s |

Boşta bekleyen bir bağlantı, sunucu ve istemci aynı süreçte çalışırken ölçüldüğünde yaklaşık
16 KB tutar.
