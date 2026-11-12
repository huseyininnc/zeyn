# Realtime Kanalı

Zeyn istekleri durumsuzdur (stateless): sunucu hiçbir soket tutmaz ve kullanıcı başına ekran durumu saklamaz. Bazı ekranlar daha fazlasını ister: aynı diyagramı düzenleyen iki kişi ya da başka biri kaydettiğinde güncellenmesi gereken bir liste. `realtime` Cargo feature'ı bunun için isteğe bağlı (opt in) bir WebSocket kanalı ekler. Bunu etkinleştirmeyen uygulamalar bu kodun hiçbirini derlemez, kullanmayan sayfalar da hiçbir şey yüklemez.

WebSocket kurumsal ağlarda sık sık engellenir, bu yüzden kanalı bir iyileştirme olarak ele alın: her ekran kanal olmadan da çalışmalıdır (diyagram düzenleyici yine action'ı üzerinden kaydeder; bir liste kullanıcı yeniden açtığında yine yenilenir).

## 1. Açmak

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

## 2. Endpoint'ler

| Endpoint | Amaç |
|---|---|
| `GET /_zeyn/live/doc/{kind}/{doc}?csrf=TOKEN` | WebSocket: ortak çalışılan bir doküman |
| `GET /_zeyn/socket` | WebSocket: topic'ler için değişiklik bildirimleri |
| `GET /_zeyn/live.js` | Topic betiği, yalnızca abone olan sayfalar bağlar |

Her el sıkışma (handshake) yükseltmeden önce şu sırayla denetlenir:

1. Kanal açıktır (değilse `404`).
2. **Origin:** `Origin` başlığı isteğin geldiği sunucuyu ya da `ZeynSocket::allow_origin` ile eklenmiş bir origin'i belirtmelidir. Tarayıcılar bunu WebSocket el sıkışmalarında her zaman gönderir; eksik olan reddedilir. Başka bir sitenin kullanıcının çerezleriyle soket açmasını engelleyen budur.
3. **CSRF token'ı:** tarayıcılar WebSocket'e başlık ekleyemez, bu yüzden `meta[name=zeyn-csrf]` içindeki token `csrf` sorgu parametresinde gider ve `POST`'larda `x-zeyn-csrf` için yapıldığı gibi `zeyn_csrf` çerezi ile sabit zamanlı karşılaştırılır. Başarısızlık `403` döner.
4. **Kimlik:** her istekteki ile aynı authenticator; kullanıcı oturum açmış olmalıdır (`403`).
5. **Kullanıcı başına sınır** (`429`).
6. Dokümanlar: tanımlı bir kind ve sade bir id (1 ila 64 harf, rakam, `-`, `_`, `.`), değilse `404`; kind'ın okuma izni (`403`). Topic'ler: her topic tanımlı olmalı ve izni sahip olunmalıdır (`403`).
7. **Interceptor'lar:** uygulamanın interceptor'ları `Live` türünde bir çağrıyla çalışır (`screen` doküman kind'ıdır, topic'lerde boştur), böylece `RequireMfa` ya da askıya alınmış hesap denetimi gibi uygulama geneli bir kural canlı kanalları da kapsar. Reddedilirse kendi durum koduyla yanıt verilir.
8. Oda sınırları: bellekteki odalar ve oda başına istemciler (`Retry-After` ile `503`).

Yazma izni bağlantı başına bir kez denetlenir: bu izni olmayan istemci güncellemeleri alır, ama bir güncelleme gönderdiği anda `1008` ile kapatılır.

## 3. Dokümanlar ve Y-CRDT

Dokümanlar `yrs` ile tutulan [Y-CRDT](https://docs.yjs.dev) dokümanlarıdır. Odalar **tenant, kind ve doküman id'si** ile anahtarlanır, bu yüzden iki tenant aynı id'lere sahip olsa bile hiçbir zaman oda paylaşmaz.

Çerçeveler (frame) ikilidir (binary). İlk bayt bir etikettir; `1` (`FRAME_UPDATE`) v1 kodlamasında bir Y güncellemesi taşır.

- Bağlanınca sunucu dokümanın tamamını tek bir güncelleme olarak gönderir.
- İstemci kendi değişiklikleri için güncelleme gönderir; sunucu her birini uygular (geçerli bir güncelleme olmayan çerçeve soketi `1008` ile kapatır) ve aynı baytları odanın diğer tüm istemcilerine iletir.
- Her (yeniden) bağlanmadan sonra island kendi durumunun tamamını geri gönderir, böylece çevrimdışı yapılan düzenlemeler birleşir. Y güncellemeleri idempotent ve değişmelidir (commutative), bu yüzden sıra ve tekrarlar önemli değildir.

Kalıcılık bir trait üzerinden yapılır:

```rust
pub trait DocStore: Send + Sync + 'static {
    fn load(&self, key: RoomKey) -> BoxFuture<Result<Option<Vec<u8>>>>;
    fn save(&self, key: RoomKey, state: Vec<u8>) -> BoxFuture<Result<()>>;
}
```

`load` bir oda açılırken çalışır (hata, bağlantıyı `503` ile reddeder; bir store'un hangi dokümanların var olabileceğini sınırlaması da böyledir). `save` dokümanın tamamını tek bir güncelleme olarak alır ve doküman değişikliği olduğu sürece her `persist_every` süresinde, ayrıca boştaki bir oda düşürülmeden önce çalışır. Başarısız bir kayıt sonraki turda yeniden denenir ve oda tutulur. `MemoryDocStore` testler ve demolar içindir.

Island tarafında, `realtime` feature'lı `zeyn-island` `live::Channel` sağlar:

```rust
let channel = Channel::document("diagram", "main", on_open, on_update, on_state)?;
channel.send_update(&update);   // after a local change
channel.close();                // in unmount
```

1 saniye sonra yeniden bağlanır, süreyi 30 saniyeye kadar ikiye katlar ve `close` ya da `1008` sonrasında durur.

## 4. Topic'ler: yerinde yenilenen ekranlar

```rust
Live::new("orders").add(DataGrid::of(search_orders)...)

#[action(permission = "sales.order.write")]
pub async fn save_order(o: Valid<Order>, store: Orders, socket: Socket, ctx: Ctx) -> Result<Order> {
    let saved = store.save(o.into_inner())?;
    socket.notify(&ctx, "orders");
    Ok(saved)
}
```

`Live`, `div[data-zeyn-live="orders"]` üretir. Screen'i bir tane içeren sayfa `/_zeyn/live.js` dosyasını bağlar (`nav.js` ile gelen bir screen `x-zeyn-live: 1` alır ve betik o zaman içe aktarılır). Betik sayfadaki tüm topic'ler için tek bir soket açar. Bir bildirim geldiğinde abone her öğe grid'lerini yerinde yeniden yükler (`zeyn:reload`; grid'in geçerli sayfası, sıralaması ve filtreleri korunur), grid içermiyorsa tüm screen'i yerinde yeniler (`zeyn:refresh`). 300 ms içindeki bildirimler tek kez işlenir.

Bildirimler yalnızca topic adını taşır, hiçbir zaman veri taşımaz: yenileme, tüm izin denetimleriyle birlikte normal grid endpoint'i üzerinden gider. `notify` yalnızca çağıranın tenant'ına ulaşır; `notify_tenant(Some(id), topic)` birini adlandırır, `None` tenant kullanmayan uygulamadır.

Aynı topic adları kanal olmadan da çalışır. `.listens("orders")` ile işaretlenen node'lar, bir bildirim topic'i adlandırdığında (`live.js` onu `zeyn:topics` olarak iletir) yerinde yenilenir; bir action `ui.refresh("orders")` ile bittiğinde ya da `.drives("orders")` diyen bir filtre değiştiğinde de yenilenir: grid'ler geçerli sayfalarını yeniden yükler, diğer dinleyiciler screen başına tek bir `POST /_zeyn/nodes` ile yeniden render edilir (`examples/docs/pages/effects.md` içindeki Topics bölümüne bakın). Socket yalnızca `ZeynSocket::topic` ile bildirilmesi gereken `Live` node'larının topic'lerine abone olur; grid olmayan parçalar için onların içinde ya da yanında `.listens` kullanın.

## 5. Bellek ve geri basınç (backpressure)

Her durum parçasının bir sınırı vardır (`Limits`, varsayılanlar köşeli parantez içinde):

| Sınır | Varsayılan | Ulaşıldığında |
|---|---|---|
| `per_user` soket | 8 | el sıkışma `429` |
| `per_room` istemci | 32 | el sıkışma `503` |
| bellekteki `rooms` | 256 | el sıkışma `503` |
| istemciden gelen `message_bytes` | 64 KB | soket kapatılır |
| doküman başına `doc_bytes` | 1 MB | güncelleme reddedilir ve soket kapatılır (`1009`) |
| istemci başına `queue` | 32 mesaj | istemci odadan çıkarılır ve `1013` ile kapatılır |
| istemcisiz oda için `idle` | 30 sn | kalıcılaştırılır ve bellekten düşürülür |
| `heartbeat` | 20 sn | bir ping; 3 aralık boyunca sessiz kalan istemci kapatılır |
| `persist_every` | 2 sn | değişen dokümanlar kaydedilir |

Yayınlar (broadcast) hiçbir zaman beklemez: bir güncelleme `try_send` ile her istemcinin sınırlı kuyruğuna sunulur. Kuyruğu dolan istemci hemen odadan çıkarılır, böylece tek bir yavaş okuyucu belleği tutamaz ve diğerlerini yavaşlatamaz; yeniden bağlanır ve dokümanın tamamını alır. Kuyruğu dolu bir topic abonesi yalnızca o bildirimi kaçırır (bir sonraki zaten onu yeniler).

Bu yüzden en kötü durum belleği, dokümanlar için yaklaşık `rooms * doc_bytes` artı kuyruklar için `clients * queue * message_bytes` kadardır (bir güncelleme kuyruklar arasında paylaşılır, kopyalanmaz). Diyagramda iki canlı sekmesi olan showcase sunucusu toplamda yaklaşık 19 MB kullanır (debug derlemesi). `Socket::stats()` izleme için odaları, istemcileri, aboneleri ve doküman baytlarını bildirir.

## 6. Güvenlik notları

- Kanal hiçbir zaman action çalıştırmaz: dokümanlar, uygulamanın `DocStore` üzerinden okuduğu verilerdir ve doğrulamayı yaptığı yer orasıdır (showcase kaydedilen her diyagramı `Diagram::clamped` ile sınırlar).
- Bir doküman, tenant içinde kind'ın okuma iznine sahip herkes tarafından okunabilir; farklı kitleleri olan dokümanlar için ayrı kind'lar (ya da store'un reddettiği id'ler) kullanın.
- Her sayfanın politikası, `default-src 'self'` sayesinde uygulamanın kendi origin'ine WebSocket'e izin verir; kanal için içerik güvenlik politikasında (CSP) hiçbir şey gevşetilmez.

## 7. Örnek

`examples/showcase`: Islands, Diagram editor bölümünü iki sekmede açın. Bir sekmede bir şekli taşımak ya da eklemek diğerinde görünür ve aşağıdaki "Shapes stored on the server" grid'i, doküman `DocStore` ile kaydedilip `diagram` topic'i bildirildiğinde yaklaşık iki saniye içinde yerinde yenilenir.

Mevcut sürümün sınırları: odalar tek bir süreçte yaşar (birden çok sunucu örneği yapışkan oturum (sticky session) ya da paylaşılan bir broker ister), presence (burada başka kimin olduğu, imleçler) yerleşik değildir ve bir doküman her bağlanmada bütün olarak gönderilir.
