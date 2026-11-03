# Kurumsal Özellikler

İş uygulamaları için çapraz kesen özellikler: veri denetimi, kişisel veri koruması, güvenli dağıtımlar,
sağlık kontrolleri, oturum güvenliği, CSV ve XLSX içe aktarma, arka plan işleri ve onay iş akışları.
Her şey sunucuda denetlenir; tarayıcı çalışma zamanı yalnızca kullanımı rahatlatır. KVKK ve GDPR
akılda tutulan gizlilik yasalarıdır, ancak buradaki hiçbir şey bir ülkeye veya sektöre özgü değildir.

## Alan düzeyinde veri denetimi

`AuditLog`, her form gönderimi ve `audit` işaretli bir action'a yapılan her çağrı için bir `AuditEntry`
yazar. Kaydetmelerde giriş ayrıca kaydı ve değişen her alanı adıyla belirtir:

```json
{"kind":"form","screen":"customer","action":"save_customer","ok":true,"user":"ayse",
 "model":"Customer","record":"2","changes":[{"field":"name","old":"Customer 2","new":"Renamed"}]}
```

- Formlar bunu kendiliğinden yapar: eski değerler ekranın sunucuda yüklediği kayıttan, yeni değerler
  doğrulanmış `Valid<M>` içinden gelir. İstemcinin gönderdiği hiçbir şey eski değer olarak kullanılmaz
  ve formun istemciden almadığı alanlar hiçbir zaman değişmiş görünmez.
- API kayıtları, modeli ve anahtarı `Valid<M>` girdisinden alır. Veritabanında ne olduğunu yalnızca
  action bilir, bu yüzden farkı kendisi bildirir:

  ```rust
  #[action(permission = "crm.customer.write", audit)]
  pub async fn save_customer(c: Valid<Customer>, store: Customers, ctx: Ctx) -> Result<Customer> {
      let current = store.get(c.id)?;
      ctx.record_changes(&current, &c);
      store.save(c.into_inner())
  }
  ```

  Aynısını form kaydetme action'larında yapmak da uygundur: action'ın raporu formunkinin yerine geçer.
- Değerler düz, dilden bağımsız string'lerdir (`1234.56 TRY`, `2024-05-01`, seçim değerleri).
  Satır sürümleri dışarıda bırakılır. Maskeli alanlar maskeli kalır (aşağıya bakın).
- Başarısız bir kayıt yine de kaydı adıyla belirtir, değişiklik içermez.

## Alan maskeleme

Kişisel veri tutan metin alanlarını `#[mask]` ile işaretleyin. Burada ulusal kimlik numarası `•••• 8901`, e-posta `p••••@example.com` ve notlar `••••` olarak görünür:

```rust
#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    pub name: String,
    #[mask(last4)]
    pub national_id: String,
    #[mask(email, permission = "hr.contact.unmask")]
    pub email: Option<String>,
    #[mask]
    pub notes: Option<String>,
}
```

- Maskeleme sunucuda gerçekleşir: grid hücrelerinde, formlarda, API çıktısında, ajan araçlarında ve
  dolayısıyla hücrelerden oluşturulan CSV dışa aktarımlarında. Ham değer, izin olmadan hiçbir
  tarayıcıya veya API istemcisine ulaşmaz.
- Maskeyi açma izni, öznitelik bir isim vermedikçe snake case biçiminde `<model>.<field>.unmask`
  olur (`person.national_id.unmask`).
- İzni olan kullanıcılar yine de önce maskeli metni görür ve bir gösterme düğmesi bulunur. Göstermek
  `POST /_zeyn/reveal` çağrısı yapar; bu çağrı izni yeniden denetler ve bir denetim kaydı yazar
  (kind `reveal`, alan `revealed` içinde). Reddedilen denemeler de denetlenir.
- İzni olan API istemcileri ham değerleri alır; bu alanların her biri çağrının denetim kaydındaki
  `revealed` içinde listelenir.
- Formlar maskeli alanları salt okunur metin olarak gösterir. Saklanan değer, kullanıcı göstermeye
  yetkili değilse ve gösterdikten sonra değiştirmediyse, kaydetme sırasında korunur. Boş bir maskeli
  alan normal bir girdidir, böylece veri giriş personeli onu bir kez doldurabilir ama sonradan geri
  okuyamaz.
- Maskeli bir alan üzerindeki filtreler ve sıralamalar, onu gösterme yetkisi olmayan kullanıcılar için
  yok sayılır, böylece bir değer arama yoluyla tahmin edilemez. Bu, bir grid'in her okuması için
  (sayfalar, dışa aktarımlar, tüm satırlar üzerindeki toplu action'lar, kayıtlı görünümler, ajan
  araçları) ve bir `DataQuery` alan API action'ları için geçerlidir. Maskeli sütunlara `.filter()`
  eklemeyin.
- Sayfalama imleçleri hiçbir zaman maskeli bir alanın değerini tutmaz. Maskeli bir alana göre
  sıralanmış liste, anahtar yerine konuma göre sayfalanır.
- Bir grid hiçbir zaman maskeli bir alana göre gruplamaz; üzerindeki `group_by` yok sayılır ve
  günlüğe yazılır.
- Island ve diğer veri kaynağı çıktıları, API çıktısı gibi her derinlikte maskelenir: içerideki her
  modelin maskeli alanları maskeli metin olarak çıkar ve hassas modellerin anahtarları okuma olarak
  kaydedilir.
- Grid'de gösterme, satırı grid'in action'ı üzerinden anahtar üzerinde bir filtreyle arar, bu yüzden
  action kendi anahtar alanındaki filtrelere uymalıdır.
- `#[mask]` yalnızca `String` ve `Option<String>` üzerinde derlenir.

## Hassas kayıt okumaları

Kimin hangi kişisel veriyi ne zaman gördüğü, kimin değiştirdiği kadar kaydedilir.

- `#[entity(table = "cases", sensitive)]` (veya `#[entity]` altında `#[sensitive]`) bir modeli
  işaretler. Böyle bir kayıt bir kullanıcıya her gösterildiğinde, çağrının denetim kaydı bunu
  `reads` içinde listeler (`{"model":"Case","id":"2"}`): bir grid sayfasının satırları, bir form,
  bir ajan aracı sonucu ve API çıktısı. Okumalar verinin render edildiği yerde kaydedilir, bu
  yüzden layout'ların içindeki bileşenler de sayılır.
- `#[screen(..., sensitive)]`, ekranın her açılışını, `model` değeri ekran adı ve `id` değeri
  parametreleri olan bir okuma olarak kaydeder. Bu, tek bir modele bağlı olmayan kişisel veriler
  gösteren ekranlar içindir.
- `#[screen(..., reason)]` ayrıca herhangi bir veri gösterilmeden önce kullanıcıya nedenini sorar.
  Sunucu, bir neden verilene kadar sayfaya, parçalarına, grid, form ve gösterme çağrılarına `428`
  ve `reason_required` koduyla (tam sayfa yüklemelerinde bir neden formu olan bir sayfa) yanıt
  verir. Çalışma zamanı bir iletişim kutusunda sorar, `POST /_zeyn/reason {screen, reason}`
  gönderir (3 ila 300 karakter) ve isteği tekrarlar. Neden, `HttpOnly`, `SameSite=Strict` bir
  çerezde 15 dakika saklanır, yalnızca verildiği ekranı açar ve o ekranın her denetim kaydındaki
  `reason` alanına yazılır. Ajanlar ve API istemcileri neden veremez, bu yüzden böyle ekranları
  açamaz.
- Hassas bir model üzerindeki bir grid'in dışa aktarımları (CSV, XLSX, PDF), `export` ayarlı olarak
  dosya başına bir denetim kaydı yazar: model, biçim, satır sayısı ve istenen sıralama ile
  filtreler. Her satırı `reads` içinde listelemezler, böylece büyük bir dışa aktarım ucuz kalır;
  diğer modellerin dışa aktarımları ek bir maliyet getirmez. Bir `sensitive` veya `reason` ekranından
  dışa aktarmak ekran okumasını da kaydeder.
- Bir ekranın tembel (lazy) parçaları (tembel sekmelerin kullandığı `POST /_zeyn/node`), interceptor'ları
  çalıştırır ve okumaları ekranı açmakla tamamen aynı şekilde kaydeder.
- `ctx.record_read(&record)`, uygulama kodundan bir okuma kaydeder.
- Okuma kayıtlarını `AuditLog`, sayfa yüklemeleri dahil herhangi bir çağrı için, hassas bir kayıt
  gösterilir gösterilmez yazar. Bir denetim interceptor'ı olmadan hiçbir şey kaydedilmez; kişisel
  veri tutan uygulamalar her zaman bir tane kaydetmelidir.

## Onay iş akışları

Bir iş akışı, küçük ve tipli bir durum makinesidir. Durumlar bir `#[choice]` enum'udur; her geçiş
başlangıç ve bitiş durumunu ve ihtiyaç duyduğu izni belirtir:

```rust
#[choice]
pub enum LeaveState { Draft, Submitted, Approved, Rejected }

pub fn leave_flow() -> Workflow<LeaveState> {
    Workflow::new()
        .transition("submit", LeaveState::Draft, LeaveState::Submitted, "people.leave.request")
        .transition("approve", LeaveState::Submitted, LeaveState::Approved, "people.leave.approve")
        .transition("reject", LeaveState::Submitted, LeaveState::Rejected, "people.leave.approve")
}

#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(req: TransitionRequest<LeaveRequestId>, store: LeaveStore, ctx: Ctx) -> Result<LeaveState> {
    let mut leave = store.get(req.id)?;
    leave.state = leave_flow().apply(&ctx, leave.state, &req.transition, req.comment.as_deref())?;
    store.save(leave)
}
```

- `apply`, depodan okunan durumla sunucuda çalışır. Bilinmeyen bir geçiş `404`, artık başlangıç
  durumunda olmayan bir kayıt `409` (başka biri önce taşıdı), eksik bir izin `403`, 1000 karakteri
  aşan bir yorum `422` döndürür.
- Her geçiş denetim kaydındaki `transition` alanına yazılır: ad, nereden, nereye ve yorum;
  kullanıcı ve zaman kaydın kendisindedir.
- `Transitions::of(flow, state, id, action)` geçerli durumu, isteğe bağlı bir yorumu ve kullanıcının o
  durumdan yapabileceği her geçiş için bir düğmeyi gösterir (`Workflow::allowed`). Kayıt anahtarı
  sunucuda ekrandan gelir, bu yüzden bir istemci başka bir kaydı taşıyamaz. Ajanlar bunu bir form
  aracı olarak görür.
- Düğme etiketleri kataloglardaki `workflow.<name>` anahtarından gelir; `.label(...)` birini elle
  ayarlar.
- `Workflow::check`, geçersiz izinleri ve aynı durumdan iki kez tanımlanmış geçişleri bildirir;
  bir testte çağırın.
- Showcase'deki izin talepleri bunu kullanır: taslak olarak kaydet, gönder, sonra onayla veya reddet.

## Güvenli dağıtımlar

- Her yanıt `x-zeyn-build` taşır ve her sayfa `<meta name="zeyn-build">` taşır. Kimlik, ayarlıysa
  `ZEYN_BUILD_ID` değeridir (harfler, rakamlar, `.`, `_`, `-`, en fazla 64), aksi halde çalıştırılabilir
  dosyanın bir karmasıdır (hash). `zeyn::server::build_id()` bunu günlükler için döndürür.
- Çalışma zamanı başka bir derlemeden bir yanıt gördüğünde nazik bir bildirim gösterir: yeni bir
  sürüm var, hazır olunca yeniden yükleyin. Kendiliğinden hiç yeniden yüklemez.
- Kullanıcıların formlara yazdıkları, ekran, parametreler ve form başına `sessionStorage` içinde
  saklanır ve yeniden yüklemeden sonra bir not ve atma düğmesiyle geri konur. Maskeli, parola, dosya
  ve gizli girdiler hiçbir zaman saklanmaz. Taslak, yazıldığı satır sürümünü korur, bu yüzden bu
  arada başkasının değiştirdiği bir kaydın taslağını kaydetmek çakışma olarak reddedilir. Başarılı bir
  kayıt taslağı temizler; bir günden eski taslaklar atılır. `sessionStorage` yalnızca o tarayıcı
  sekmesinde yaşar.

## Sağlık kontrolleri

- `GET /_zeyn/health`, süreç çalıştığı sürece `200 ok` yanıtı verir. Liveness için kullanın.
- `GET /_zeyn/ready`, `App::readiness(name, check)` ile eklenen her kontrolü aynı anda, her biri
  beş saniye ile çalıştırır ve `200 {"status":"ready"}` veya
  `503 {"status":"not_ready","failed":["database"]}` yanıtı verir. Yalnızca kontrol adları gösterilir,
  hatalar asla.
- İkisi de `base_path` altında yer alır (`/billing/_zeyn/health`), `Cache-Control: no-store` ile yanıt
  verir ve oturum açmayı gerektirmez.
- Hazırlık sonucu bir saniye boyunca yeniden kullanılır (`READY_CACHE`) ve aynı anda yalnızca bir istek
  kontrolleri çalıştırır, bu yüzden bir yoklama seli veritabanını yükleyemez.
- Sağlık hiçbir zaman hız sınırına tabi değildir, bu yüzden yoğun bir yoklama kendini kilitleyip
  sağlıklı bir uygulamayı yeniden başlatamaz. Hazırlık, bir `probe_ips` adresinden gelmiyorsa hız
  sınırlarına sayılır.
- IP kuralları yine de geçerlidir: `deny_ips` her zaman, `allow_only_ips` de yoklama adresi
  `App::probe_ips([...])` içinde listelenmedikçe geçerlidir; bu liste yalnızca bu iki uç noktayı o
  adrese açar.

## Geliştirme gecikmesi

`App::dev_latency(Duration::from_millis(400))`, her çağrıyı (sayfalar, parçalar, grid'ler, formlar,
API, MCP ve çalışma zamanı çağrıları; statik varlıklar veya sağlık yoklamaları değil) geciktirir,
böylece geliştiriciler yükleme durumlarını ve çift tıklamaları yavaş bir ağdaki kullanıcılar gibi
görür. Yalnızca debug derlemelerinde çalışır: release derlemelerinde yöntem hiçbir şey yapmaz ve
gecikme kodu derlemeden çıkarılır, bu yüzden yanlışlıkla bırakılması üretimi yavaşlatamaz. Açıkken
başlangıçta bir uyarı yazdırılır.

## Oturum güvenliği

Oturum açma bir tek oturum açma (single sign on) proxy'sinden, yerleşik OpenID Connect oturum
açmasından veya bearer token'lardan gelir (bkz. `docs/auth.md`), bu yüzden bu özellikler yalnızca
kimlik doğrulayıcının kullanıcının kim olduğunu ve yeniden kimlik doğrulama için ne zaman oturum
açtığını söylemesini gerektirir
(`Identity::authenticated_at`; `ForwardAuth::auth_time("x-auth-time")` Unix saniyelerini veya
RFC 3339'u okur; `DevAuth` her isteği yeni oturum açılmış olarak bildirir).

- `App::idle_timeout(Duration)`, çalışma zamanının kullanıcıyı sondan `idle_warning` önce (varsayılan
  bir dakika) geri sayımlı bir uyarı iletişim kutusunda ve "oturumu açık tut" düğmesiyle uyarmasını
  sağlar; düğme `POST /_zeyn/keepalive` çağrısı yapar, böylece öndeki bir proxy etkinlik görür.
  Uygulamanın herhangi bir sekmesindeki girdi sayılır. Sonunda kullanıcı `logout_url` adresine,
  yoksa `return_to` ile `login_url` adresine gider, o da yoksa oturumu kapatılmış bir ekran görür.
  Form taslakları saklanır. Boşta kalma zaman aşımı yalnızca unutulmuş ekranları kapatır; gerçek
  oturumu sonlandırmak kimlik sağlayıcının işidir.
- `#[action(permission = "...", reauth)]`, `App::reauth_within` içinde (varsayılan beş dakika) bir
  oturum açma gerektirir. Aksi halde sunucu, istemci ne gönderirse göndersin `reauth_required` koduyla
  `401` yanıtı verir (API'de `urn:zeyn:problem:reauth_required` problem türü). Çalışma zamanı
  kullanıcıdan yeni bir sekmede yeniden oturum açmasını ister (`reauth_url`, yoksa `login_url`) ve
  geri döndüğünde isteği tekrarlar, böylece yazılan hiçbir şey kaybolmaz. Oturum açma zamanı olmayan
  kimlikler asla geçemez.

## CSV içe aktarma

```rust
#[action(permission = "people.leave.import", audit)]
pub async fn import_leaves(rows: Vec<Valid<LeaveRequest>>, store: LeaveStore) -> Result<usize> { ... }

Screen::new("Import").body(Import::of(import_leaves).max_rows(1000))
```

- Kullanıcı bir CSV dosyası seçer. Çalışma zamanı onu UTF-8 olarak, geçerli UTF-8 değilse Windows-1254
  olarak okur (Türkçe elektronik tablolar) ve metni bir JSON gövdesinde `POST /_zeyn/import` adresine
  gönderir (multipart gerekmez; en fazla 8 MB). Sunucu bir özetle, ilk on satırın önizlemesiyle,
  satır başına hatalarla ve indirilebilir bir hata raporuyla yanıt verir.
- Ayrıştırma: bayt sırası işareti atılır, `,`, `;` veya sekme başlık satırından algılanır, ayırıcıları
  ve satır sonlarını içeren `"` alıntılama, `\r\n` veya `\n`.
- Sınırlar dosya okunurken denetlenir, sonrasında değil: okuma satır sınırında durur, 512 hücreden
  fazla bir başlık reddedilir ve veri satırları başlığın en fazla bir hücre ötesini tutar (böylece
  fazla değerler yine de bildirilir). Bir XLSX dosyası en fazla başlığın sahip olduğu sayıda sütun
  okur, başlığı en fazla modelin içe aktarabileceği sayıda sütun adlandırabilir ve 16 MB'dan büyük
  bir paylaşılan string parçası hiçbir şey açılmadan önce reddedilir.
- Sütunlar alan adına veya uygulamanın herhangi bir dilindeki etikete göre eşleştirilir; büyük küçük
  harf ve Türkçe noktalı ve noktasız i yok sayılır. Bilinmeyen, salt okunur, anahtar ve satır sürümü
  sütunları reddedilir.
- Her satır, modelin bir formu gibi aynen okunur ve doğrulanır: o dilin kullanıcısının yazdığı gibi
  değerler (`1.234,56`, `05.12.2026`), alan kuralları ve modelin kendi `validate` fonksiyonu.
- İçe aktarma action'a yalnızca geçerli satırları gönderir; `.all_or_nothing()`, her satır geçerli
  olmadıkça hiçbir şey içe aktarmaz. İçe aktarırken sunucu dosyayı yeniden okur ve doğrular; önizlemeden
  hiçbir şeye güvenilmez. Her iki adım da action'ın iznini gerektirir ve içe aktarma denetlenir.
- `.max_rows(n)` satırları sınırlar (varsayılan 5000, hiçbir zaman 50000'den fazla olamaz). Hata
  raporu elektronik tablo formüllerini etkisiz hale getirir. Maskeli alanlar önizlemede maskeli
  gösterilir.

### XLSX içe aktarma

`xlsx` Cargo feature'ı ile aynı bileşen Excel çalışma kitaplarını (`.xlsx`) da alır. Çalışma zamanı
dosyayı base64 olarak gönderir (aynı JSON gövdesinde `"csv"` yerine `"xlsx"`) ve sunucu ilk çalışma
sayfasını `calamine` ile okuyup CSV ile aynı satır satır doğrulamaya, önizlemeye ve hata raporuna
aktarır.

- Hücreler türlerini korur: bir sayı sayı, bir tarih hücresi tarih olarak okunur, kullanıcının dili
  ne olursa olsun, bu yüzden Türkçe bir oturumda `1234.5` yine 1.234,50 olur. Bir tarih hücresindeki
  duvar saati zamanları, bir formun okuduğu gibi, kullanıcının saat diliminde okunur. Seçimler değere
  veya etikete göre eşleşir.
- Sınırlar: en fazla 8 MB dosya ve arşiv en fazla 1000 parçada en fazla 64 MB'a açılabilir; bu, hiçbir
  şey açılmadan önce arşivin bildirdiği boyutlardan denetlenir. Hücreler tek tek okunur ve okuma satır
  sınırında durur, bu yüzden bir milyon satırlı bir sayfa sınırdan fazlaya mal olmaz. Çalışma kitabı
  olmayan, parola korumalı veya yarım kesilmiş bir dosya net bir hata alır.
- Yalnızca ilk çalışma sayfası okunur. Formüller önbelleğe alınmış değerleriyle sayılır.

## Arka plan işleri

```rust
App::new("Billing")
    .job_concurrency(2)
    .job("send_reminders", Schedule::every(Duration::from_secs(3600)).retry(3, Duration::from_secs(30)), send_reminders)
    .job("rebuild_index", Schedule::manual(), rebuild_index)
    .jobs_permission("ops.jobs.read")
    .serve_until("0.0.0.0:3000", shutdown_signal())
```

- Bir iş `async fn(Ctx) -> Result<()>` biçimindedir. Bağlamı uygulamanın servislerini, yapılandırmasını
  ve varsayılan dilini ve hiçbir izni olmayan `job:<name>` kimliğini içerir.
- `Schedule::every(d)` her aralıkta çalışır; `Schedule::manual()` yalnızca tetiklendiğinde. Action'lar
  işleri enjekte edilebilir `Jobs` servisi ile tetikler: `jobs.trigger("rebuild_index")`. Çalışırken
  tetiklenen bir iş sonrasında bir kez daha çalışır; bir iş hiçbir zaman aynı anda iki kez çalışmaz.
- `.retry(times, first_delay)` başarısız bir çalıştırmayı yeniden dener, beklemeyi on dakikaya kadar
  ikiye katlayarak.
- Aynı anda en fazla `job_concurrency` iş (varsayılan dört) çalışır.
- Durum (state, çalıştırma sayısı, hata sayısı, son başlangıç ve bitiş, son hata kodu) kodda
  `jobs.status()`, `jobs_permission` sahibi kullanıcılar için `GET /_zeyn/jobs` ile alınır. Hata
  ayrıntıları yalnızca sunucu günlüğüne gider.
- `App::serve_until(addr, signal)`, sinyal tamamlandığında istek almayı bırakır, çalışan isteklerin
  bitmesine izin verir, yeniden deneme beklemelerini iptal eder ve çalışan işlere 30 saniye tanır.
- Bir depo olmadan işler bellektedir: süreç durduğunda bekleyen veya çalışan iş kaybolur ve birden
  çok sunucuyla her biri kendi zamanlamasını çalıştırır.

### Kalıcı işler

```rust
App::new("Billing")
    .job_store(SqlJobStore::new(db))
    .queued_job("send_invoice", Schedule::manual().retry(5, Duration::from_secs(10)), send_invoice)
    .job("close_day", Schedule::every(Duration::from_secs(86_400)), close_day)

async fn send_invoice(ctx: Ctx, invoice_id: String) -> Result<()> { ... }
jobs.enqueue("send_invoice", "42").await?;
```

- `JobStore` çalıştırmaları tutar: `enqueue`, `schedule`, lease (worker kimliği ve bitiş zamanı) ile
  `claim`, `heartbeat`, `complete` ve `fail`. `MemoryJobStore` (en fazla 10.000 çalıştırma, önce en
  eski başarısız çalıştırma atılır) varsayılandır; `SqlJobStore` (`postgres`, `oracle` feature'ları)
  çalıştırmaları `zeyn_jobs` içinde tutar. `create_job_tables(dialect, prefix)` bir migration için
  ifadeleri verir.
- Bir depo ayarlıyken, `Schedule::every` işleri sonraki çalıştırmalarını depoda tutar, bu yüzden bir
  yeniden başlatma onu kaydırmaz ve her aralığı yalnızca bir sunucu çalıştırır. `trigger` orada da bir
  çalıştırma kuyruğa alır.
- `queued_job` işleri her çalıştırmanın yükünü (payload) alır (en fazla `MAX_JOB_PAYLOAD`, 4000 bayt);
  düz işler de kuyruğa alınabilir ve yükü yok sayar. `enqueue_after` bir çalıştırmayı geciktirir.
- Claim, bu sunucunun bildiği işlerin zamanı gelmiş çalıştırmalarını, en eskiden başlayarak alır ve
  onları `JOB_LEASE` (60 saniye) boyunca tutar; çalıştırma sürerken her 20 saniyede yenilenir.
  `complete` ve `fail`, bir çalıştırmayı yalnızca worker onu hâlâ tutarken değiştirir. Worker'ı
  duran bir çalıştırma, lease'i bitince yeniden claim edilir; kaybolan deneme sayılır, bu yüzden
  sunucusunu çökerten bir çalıştırma başarısız olarak biter.
- Başarısız bir deneme, deneme hakkı kaldıkça backoff süresini bekler (ikiye katlanır, en fazla
  `MAX_BACKOFF_MS`, on dakika), sonra hata koduyla `failed` olarak kalır. Bir zamanlama sonraki
  aralığına geçer.
- Worker'lar zamanı gelmiş çalıştırmaları her `JOB_POLL` (bir saniye) aralığında ve aynı sunucuda bir
  `enqueue` sonrasında hemen arar. Her claim, boş iş yuvası sayısı kadar çalıştırma alır, bu yüzden
  bellek kuyrukla birlikte büyümez.
- PostgreSQL tek bir `UPDATE ... WHERE id IN (SELECT ... FOR UPDATE SKIP LOCKED) RETURNING` ile claim
  eder. Oracle 11g ve sonrası, bir transaction içinde `ROWNUM` ve `FOR UPDATE SKIP LOCKED` ile
  kimlikleri seçer, günceller ve commit'ten önce geri okur. Oracle kilitleyen bir select'i
  sıralayamaz, bu yüzden orada zamanı gelmiş çalıştırmalar belirli bir sıra olmadan alınır. Zamanlar,
  `BIGINT` veya `NUMBER(19)` sütunlarında Unix milisaniyesidir.
- Bir depo varken, aynı işin zamanlanmış bir çalıştırması ile tetiklenmiş bir çalıştırması çakışabilir;
  aynı anda iki kez çalışmaması gereken işler kendilerini korumalıdır (örneğin bir satır kilidiyle).
