# Operasyon

Bir Zeyn uygulamasını üretimde çalıştırma: saat dilimi verisi, imzalı lisanslar, birden fazla sunucu için paylaşılan durum, loglar ve izler, PDF belgeleri ve e-posta.

## Saat dilimleri

`TimeZone::named` her IANA bölgesini (`Europe/Istanbul`, `America/New_York`, `Australia/Sydney`, büyük küçük harf fark etmez) ve sabit ofsetleri (`UTC+03:00`, `+05:30`) kabul eder.

- Bölge verisi, `jiff` aracılığıyla ikili dosyanın içine gömülü IANA saat dilimi veritabanıdır (yaklaşık 200 KB). Cevaplar Windows dahil her platformda aynıdır ve sistemin zoneinfo verisi okunmaz.
- Geçmiş de modellenir: Türkiye'nin 2016'ya kadar yaz saati uygulaması ve o tarihten beri kalıcı `+03` ofseti, Amerika Birleşik Devletleri'nin 2007 öncesi kuralları ve kayıtlı diğer tüm değişiklikler.
- Var olmayan bir duvar saati (saatlerin ileri alındığı an) `LocalTimeError::Skipped` ile reddedilir. İki kez yaşanan bir saat (saatlerin geri alındığı an) önceki anı ifade eder.
- Bir bölge, adı ilk kullanıldığında pakete gömülü veriden okunur ve sürecin ömrü boyunca tutulur; her biri birkaç kilobayttır. Bilinmeyen adlar hiçbir zaman tutulmaz, bu yüzden istemciler önbelleği büyütemez.
- `TimeZone::names()`, kabuğun (shell) bölge seçicisi için yaygın bölgelerden oluşan kısa bir listedir. Listede olmayan bir kullanıcı bölgesi, o kullanıcı için seçiciye eklenir.
- Yeni bölge kuralları daha yeni bir `jiff` sürümüyle gelir (`cargo update -p jiff-tzdb`). Bir ülke kurallarını değiştirdiğinde yeniden derleyin ve dağıtın.

## İmzalı lisanslar

Bir lisans dosyası, yalnızca satıcının elinde bulunan bir ed25519 anahtarıyla imzalanır. Uygulama genel anahtarı gömülü taşır; böylece müşteri lisansı okuyabilir ama değiştiremez ya da yenisini üretemez. `signed-licenses` Cargo feature'ı ile açılır.

```
zeyn license keygen                      # writes zeyn-license.key, prints the public key
zeyn license sign acme.json --out license.json
```

`acme.json` lisans alanlarını içerir:

```json
{ "holder": "Acme", "valid_until": "2027-12-31", "modules": ["pos", "stock"], "max_users": 50 }
```

```rust
const LICENSE_KEY: &str = "f414a89e9dae57598d60141df96dd1c9fb655fdcf49f5eaf24e093fda7d332e3";

App::new("Shop")
    .license(SignedLicenseFile::new("license.json", LICENSE_KEY), Duration::from_secs(3600))
    .feature(Feature::new("pos").guard(Licensed::module("pos")).screen(pos))
```

- Dosya JSON'dur: lisans `payload` içinde metin olarak, imza `signature` içinde durur. İmzalanan şey `zeyn-license-v1\n` ifadesi ve ardından gelen payload metnidir; yani imzalanan baytlar saklanan baytlarla birebir aynıdır.
- Özel anahtar imzalama makinesinde kalır. `keygen` var olan bir anahtar dosyasının üzerine asla yazmaz ve `*.key` yeni projelerin `.gitignore` dosyasında yer alır. `sign` anahtarı `--key` seçeneğinden, `ZEYN_LICENSE_KEY` değişkeninden ya da `zeyn-license.key` dosyasından okur; `holder` alanı olmayan veya `valid_until` değeri `YYYY-MM-DD` biçiminde olmayan bir lisansı reddeder.
- Değiştirilmiş bir dosya, başka bir anahtarla imzalanmış bir dosya veya lisans olmayan bir dosya reddedilir: `License::from_signed` `LicenseError` döndürür (`BadSignature`, `InvalidPublicKey`, `Malformed`), sunucu nedenini loglar ve her `Licensed` guard "Your license does not cover this." mesajıyla erişimi reddeder. Okunamayan bir dosya, başarısız olan her lisans kaynağında olduğu gibi son lisansı korur.
- Süre sonu, guard tarafından her istekte kullanıcının bugününe göre denetlenir; bu yüzden süresi dolmuş bir lisans, yeniden başlatma gerekmeden uygulamayı "Your license has expired." mesajıyla kilitler. Yenilenmiş bir dosya sonraki yenilemede okunur.
- ed25519, değiştirilmiş bir dosyayı başka bir anahtarla imzalanmış bir dosyadan ayırt edemez; ikisi de eşleşmeyen bir imzadır ve mesaj bunu söyler.
- `max_users` taşınır ve imzalanır ama framework tarafından uygulanmaz; neyin kullanıcı sayılacağına uygulama karar verir.

## Birden fazla sunucu: Redis'te paylaşılan durum

`redis` Cargo feature'ı ile `RedisStore`, bir yük dengeleyicinin arkasındaki her örnekte aynı olması gereken durumu tutar:

```rust
let redis = RedisStore::new(config.require("redis.url")?, "billing").map_err(Error::Internal)?;
App::new("Billing")
    .rate_limit(RateLimit::per_ip(300, Duration::from_secs(60)))
    .rate_store(redis.clone())
    .datagrid_views(redis)
```

Sunucunun kullanıcı başına neyi nerede tuttuğu:

| Durum | Nerede | Paylaşılan |
|---|---|---|
| Hız sınırı sayaçları | `RateStore` | `RedisStore` |
| Kaydedilmiş grid görünümleri ve dock yerleşimleri | `DataGridViewStore` | `RedisStore` |
| Boşta kalma zaman aşımı | tarayıcı (uygulamanın tüm sekmeleri) | sunucuda hiçbir şey |
| Oturum açma ve yeniden kimlik doğrulama | tek oturum açma (single sign on) proxy'si | sunucuda hiçbir şey |
| Hassas bir ekran için gerekçe | bir `HttpOnly` cookie | sunucuda hiçbir şey |
| Form taslakları | `sessionStorage` | sunucuda hiçbir şey |
| Lisans önbelleği | süreç başına, kaynaktan yeniden yüklenir | her sunucu kendi önbelleğini yükler |
| Arka plan işleri | süreç başına | paylaşılmaz; bkz. `enterprise.md` içindeki işler (jobs) bölümü |

- Hız sınırları tek bir Lua betiğinde token bucket olarak çalışır; bu yüzden farklı sunuculardaki eşzamanlı istekler sınırı asla aşmaz. Zaman uygulama sunucularından değil, Redis'ten gelir. Sayaçlar son kullanımlarından bir pencere sonra sona erer.
- Görünümler `zeyn:<prefix>:view:<user length>:<user>:<grid>` anahtarı altında JSON olarak durur ve asla sona ermez.
- Bağlantı ilk kullanımda kurulur ve kendiliğinden geri gelir. Çağrılar 500 ms sonra zaman aşımına uğrar. Redis kapalıyken hız sınırları istekleri geçirir, görünümler ne yüklenir ne kaydedilir (grid kendi yerleşimine, dock `localStorage`'a döner), kesinti ve düzelme ayrı ayrı birer kez loglanır ve her hatadan sonra Redis beş saniye rahat bırakılır; böylece istekler tek tek zaman aşımını beklemez.
- `RateStore::take` bir future döndürür; böylece depolar bir ağ ile konuşabilir. `MemoryRateStore::take` doğrudan kullanım için eşzamanlı (synchronous) kalır.
- TLS (`rediss://`) yerleşik değildir.
- `ZEYN_TEST_REDIS` ayarlıysa testler gerçek bir sunucuya karşı çalışır:

  ```
  docker run -d --rm --name zeyn-redis -p 6379:6379 redis:7-alpine
  ZEYN_TEST_REDIS=redis://127.0.0.1:6379 cargo test -p zeyn --features redis --test redis
  docker stop zeyn-redis
  ```

## Loglar ve izler

Framework `tracing` üzerinden raporlar. Kendiliğinden hiçbir şey yazdırmaz.

- `App::serve` ve `App::serve_until`, uygulama önceden bir tane kurmadıysa bir subscriber kurar: olaylar standart hataya metin satırları olarak gider ve `log.level` ile filtrelenir (varsayılan `info`; `warn,zeyn=debug` gibi herhangi bir `tracing` hedef filtresi). JSON satırları ya da başka bir çıkış isteyen bir uygulama, servis vermeden önce kendi subscriber'ını kurar ve Zeyn ona dokunmaz.
- `info` düzeyinde çıktı her zamanki gibidir: dinlenen adres, başlangıç uyarıları (eksik çeviriler, `DevAuth`, `dev_latency`), başarısız lisans kaynakları ve iç hatalar. `debug` düzeyinde her çağrı; tür, ekran, action, kullanıcı kimliği, tenant kimliği, sonuç, hata kodu ve milisaniye içeren bir `call finished` satırı ekler.
- İz arka uçları için span'ler:

  | Span | Düzey | Alanlar |
  |---|---|---|
  | `request` | info | method, path (sorgu dizesi olmadan), status, ms ve bilindiğinde: kind, screen, action, user, tenant, ok, error |
  | `run` | info | uygulama kodu: bir action, bir veri kaynağı (grid fetch, satır sorgusu, görünüm deposu), bir screen builder |
  | `interceptor` | debug | name (tür adı veya `Interceptor::name`), stage (`before` ya da `after`) |

- `otel` Cargo feature'ı ile, `Config` içinde `otel.endpoint` ayarlıysa (`http://collector:4318`; span'ler `/v1/traces` adresine gider) span'ler OTLP/HTTP üzerinden dışa aktarılır. `otel.service_name` servisi adlandırır; varsayılan uygulamanın adıdır. Span'ler bir arka plan thread'inde gruplanır ve `serve_until` durduğunda boşaltılır.
- Dışa aktarma yoksa standart hata yalnızca olayları alır; bu yüzden hiçbir span oluşturulmaz ve span'ler başına önbelleğe alınmış bir denetim kadar maliyet getirir. `App::router` ile başka bir sunucuya bağlanan uygulamalar ve testler de aynı bedeli öder.
- Hiçbir zaman kaydedilmeyenler: alan değerleri (maskeli olsun olmasın), form ve API girdisi, sorgu dizeleri, cookie'ler, header'lar ve gizli bilgiler. Bir test, grid'leri, form kayıtlarını, API çağrılarını ve açığa çıkarmaları (reveal) trace düzeyinde bir subscriber altında çalıştırır ve hiçbir maskeli değerin görünmediğini denetler. İki şey uygulamada kalır: path'ler kaydedilir, bu yüzden route'lara kişisel veri koymayın; ve `Error::Internal` mesajları yazıldığı gibi loglanır.

## PDF belgeleri

`pdf` Cargo feature'ı, [Typst](https://typst.app) şablonlarından PDF belgeleri üretir. Typst büyük olduğu için varsayılan olarak kapalıdır (aşağıdaki rakamlara bakın).

```rust
const INVOICE: PdfTemplate = PdfTemplate::new("invoice", include_str!("../templates/invoice.typ"));

#[action(permission = "sales.order.read")]
pub async fn invoice_data(p: OrderParams, store: Orders, ctx: Ctx) -> Result<InvoiceData> { ... }

Screen::window("Order").body(PdfLink::new("Print invoice", INVOICE, invoice_data, params))

App::new("Shop").service(Pdf::from_config(&config)?)
```

- `PdfLink`, bir screen'e indirme düğmesi koyar. `GET /_zeyn/pdf` screen'i erişim denetimleriyle açar, action'ı kullanıcı için çalıştırır (action'ın izni ve interceptor'ları geçerlidir, çağrı türü `Document`'tır) ve şablonu action'ın döndürdüğü veriyle işler. Action'ı çalıştırma izni olmayan kullanıcılar düğmeyi görmez.
- `exportable()` olan grid'ler, araç çubuğunun `Export::Pdf` seçeneğiyle bir liste raporu da yazdırır (`GET /_zeyn/grid/export?format=pdf`): kullanıcının sütunları, etiketleri, filtreleri ve sıralaması, sağa hizalı sayılar, tekrarlanan başlık satırı ve sayfa numaraları. Bir rapor en fazla 2.000 satır içerir (`MAX_PDF_ROWS`, ya da daha küçükse grid'in dışa aktarma sınırı) ve satır dışarıda kaldığında bunu belirtir.
- **Veri hiçbir zaman Typst koduna dönüşmez.** Action'ın çıktısı JSON'a serileştirilir ve şablon onu `data` olarak okur (işleyici, şablondan önce `#let data = json("/data.json")` ifadesini bağlar). `#panic()`, `]` veya `$x$` gibi değerler metin olarak yazdırılır; bir test bunu denetler. Şablonlar başka hiçbir şeyi okuyamaz: dosya yok, paket yok.
- İşleme (rendering) bir engelleyen thread'de çalışır. Aynı anda en fazla `pdf.max_concurrent` belge işlenir (1 ile 64 arası; varsayılan iki, tek CPU'da bir); geri kalanlar bir yuva için bekler. Typst'in memoization önbelleği her belgeden sonra boşaltılır, böylece bellek yeniden düşer.

| Anahtar | Anlamı |
|---|---|
| `pdf.fonts` | Başlangıçta bir kez okunan, ek `.ttf`, `.otf`, `.ttc` veya `.otc` yazı tiplerinin dizini |
| `pdf.max_concurrent` | Aynı anda işlenen belge sayısı |

Yazı tipleri: Libertinus Serif (normal, kalın, italik, kalın italik; SIL Open Font License) gömülüdür ve Türkçeyi kapsar; DejaVu Sans Mono, `₺` gibi Libertinus'ta bulunmayan işaretler için yedek olarak gömülüdür. Lisansları `crates/zeyn/assets/fonts` içindedir. Matematik yazı tipi yoktur; şablonlar denklem kullanıyorsa `pdf.fonts` ile bir tane ekleyin.

Windows'ta ölçüldü (release derlemesi, x86_64):

| | Sonuç |
|---|---|
| İkili dosya boyutu | `pdf` olmadan 5,5 MB, ile yaklaşık 46 MB (Typst yaklaşık 39 MB, yazı tipleri 1,6 MB ekler) |
| Tek sayfalık bir belge | 4 ile 7 ms, işleme sırasında 1 MB'ın altında heap, 10 KB dosya |
| 1.000 satırlık bir liste raporu (25 sayfa) | 0,54 ile 0,74 sn, işleme sırasında 51 MB tepe heap, sonrasında 0 kalır, 0,87 MB dosya |
| Başlangıçtan sonraki ilk belge | Typst'in tembel biçimde oluşturulan kendi tabloları için yaklaşık 42 MB heap kalır |
| Yazı tipleri | Yazı tipi dizini için 0,25 MB heap |

Debug derlemesi yaklaşık on kat daha yavaştır (showcase'de 2.000 satırlık bir rapor 71 sn sürdü).

## E-posta

`email` Cargo feature'ı, bildirimleri [lettre](https://lettre.rs) ile gönderir.

```rust
let mailer = Mailer::from_config(&config)?;
App::new("People").mailer(mailer)

#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(r: TransitionRequest<LeaveRequestId>, store: Leaves, mailer: Mailer, ctx: Ctx) -> Result<LeaveState> {
    let state = leave_flow().apply(&ctx, current, &r.transition, r.comment.as_deref())?;
    mailer.send(&ctx, Mail::transition(&ctx, &manager_email, &employee)?)?;
    Ok(state)
}
```

- `Mailer::send` iletiyi işler ve bellekteki bir kuyruğa koyar (en fazla 10.000 ileti), sonra döner. `zeyn.mail` arka plan işi kuyruğu hemen gönderir, başarısız iletileri geri çekilme (backoff) ile yeniden dener (`Mailer::retry` başka bir şey söylemedikçe on saniyeden başlayarak beş kez) ve kuyruğa her dakika bakar. Bir ileti, sekiz başarısız denemeden sonra logda bir hata ile bırakılır. Her iş gibi, süreç durduğunda gönderilmemiş bir kuyruk kaybolur; kalıcı teslimat için veri katmanının veritabanı destekli kuyruğu gerekir.
- İletiler i18n kataloglarından gelir: `<template>.subject`, `<template>.text` ve isteğe bağlı olarak `<template>.html`, `{name}` argümanlarıyla. `Mail::locale` ayarlamadıkça alıcının dili göndericinin dilidir. Her iletinin düz metin ve HTML bölümü vardır; HTML bölümünde her argüman kaçırılır (escape) ve `.html` girdisi yoksa HTML, kaçırılmış metindir. Konulardan satır sonları kaldırılır ve adresler ayrıştırılır; böylece veri header veya alıcı ekleyemez.
- `Mail::transition`, çağrının az önce `Workflow::apply` ile yaptığı geçişi (kayıt, önceki durum, yeni durum, kim ve yorum) yerleşik `zeyn.mail.transition` şablonuyla yazar. Showcase, bir izin talebi ilerlediğinde yöneticiye haber verir.

| Anahtar | Anlamı |
|---|---|
| `mail.from` | `Northwind <no-reply@example.com>` gibi gönderici (zorunlu) |
| `mail.transport` | `smtp`, `file` (ileti başına bir `.eml` dosyası, geliştirme ve testler için) veya `log` (alıcı sayısını ve boyutu içeren, asla adres ya da içerik içermeyen bir log satırı; varsayılan) |
| `mail.smtp.host`, `mail.smtp.port` | SMTP sunucusu |
| `mail.smtp.tls` | `starttls` (varsayılan), `tls` (örtük TLS, genellikle 465 portu) veya `none` (yalnızca yerel röleler) |
| `mail.smtp.user`, `mail.smtp.password` | Kimlik bilgileri; parolayı ortam gibi bir gizli kaynaktan okuyun |
| `mail.file.dir` | `file` taşıyıcısının dizini |

TLS, Mozilla kök sertifikalarıyla (`webpki-roots`) rustls kullanır.
