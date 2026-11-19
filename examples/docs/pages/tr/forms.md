# Form'lar

`Form::of(value)` bir model ya da entity'yi düzenler. Alanlar alan referanslarından, etiketler locale dosyalarından, doğrulama ise `#[rule]` öznitelikleri üzerinden gelir; böylece tarayıcı ve sunucu aynı kuralları denetler.

## Bir form

```rust
Form::of(Contact { quantity: 1, ..Contact::default() })
    .row(|r| r.field(Contact::name).field(Contact::email))
    .row(|r| r.field(Contact::quantity).field(Contact::size))
    .submit(t!("docs.live.send"), send_contact)
    .on_success(|_, ui| ui.toast(t!("docs.live.sent")))
```

@live form

Adı boş bırakın ya da yanlış bir e-posta yazıp *Send* düğmesine basın: tarayıcı hatayı hemen gösterir ve sunucu aynı girdiyi aynı iletiyle reddederdi.

## Yerleşim

| Çağrı | Sonuç |
|---|---|
| `.field(M::f)` | Kendi satırında tek bir alan |
| `.row(\|r\| r.field(a).field(b))` | Yan yana alanlar; dar ekranlarda üst üste dizilirler |
| `.section(title)` | Kendisinden sonraki alanların başlıklı bir grubu |
| `.settings()` | Ayarlar yerleşimi: her denetimin yanında etiket ve yardım, bir şey değişince altta sabit duran kaydetme çubuğu |
| `.step(title)` | Bir sihirbaz adımı; form bir seferde tek adım gösterir |
| `.range(M::from, M::to)` | Tek bir aralık seçici olarak iki tarih |
| `Form::auto(value)` | Her alan, tanım sırasıyla |

Formlar en fazla 720 px genişliğindedir ve yalnızca bir form içeren bir kart ya da panel formun genişliğinde olur, böylece kartın yarısı boş kalmaz. Etiketler 36 px'lik girdilerin 6 px üstünde durur; alanlar arasında 20 px vardır. Odaklanmış bir girdi odak renginde bir kenarlık ve yumuşak 3 px'lik bir halka alır, geçersiz bir girdi ise tehlike renginde bir kenarlık. Birimler, para birimi ve `%` girdinin içinde soluk çizilir; sayı alanları eksi ve artı düğmelerini (28 px) alanın sonunun içine koyar; ok tuşları yine adım adım değiştirir.

## Kaydetme

`.submit(label, action)` kaydeden action'ı adlandırır. Action, form'un modeli için `Valid<M>` almalıdır; başka bir şey derlenmez. Runtime değerleri gönderir, sunucu her kuralı, ardından action'ı çalıştırır.

- Kurallardan ya da action içindeki `ValidationErrors` değerinden gelen alan hataları alanların yanında görünür.
- `Error::Conflict` (eski bir `#[version]`), kaydı başka birinin değiştirdiğini söyleyen bir ileti gösterir.
- `.on_success(|result, ui| ...)` başarılı bir kayıttan sonra effect'leri çalıştırır ([Effects and windows](effects.md) sayfasına bakın).

## Kontroller

Her alan türünün varsayılan bir kontrolü vardır: metin girdileri, sayı girdileri, para birimiyle birlikte para, tarih ve saat seçiciler, `bool` için onay kutuları ve seçenekler için seçim listeleri. `.control(M::f, Control::...)` başka bir tanesini seçer, `.lookup(M::f, action)` ise kayıtları sunucuda arar. [Controls and pickers](controls.md) sayfasına bakın.

## Bağımlı lookup'lar

`.depends_on(field, source)`, `field` lookup'ının seçeneklerini başka bir alana bağlar; bir ülkenin şehirleri gibi. Her arama `source` değerini `LookupQuery::parent` içinde gönderir (`q.parent_id()` onu bir id olarak okur) ve `source` değişince `field` temizlenir; böylece seçenekleri yeni değer için yeniden yüklenir.

```rust
#[action(permission = "geo.read")]
pub async fn find_cities(q: LookupQuery, store: Cities) -> Result<Page<LookupItem>> {
    Ok(q.apply(store.of_country(q.parent_id())))
}

Form::of(address)
    .lookup(Address::country, find_countries)
    .lookup(Address::city, find_cities)
    .depends_on(Address::city, Address::country)
    .submit(t!("save"), save_address)
```

Bu ikiliye tarayıcıda güvenilemez; bu yüzden submit sırasında sunucu, seçilen şehri gönderilen ülkeyle birlikte lookup action'ına action'ın izniyle yeniden sorar ve action'ın döndürmediği bir şehri bir alan hatasıyla reddeder. `q.apply` öncesinde filtrelemenin yaptığı gibi id sorgularında da üst değere göre filtreleyin. `App::check_page`, alanı form'un bir lookup'ı olmayan ya da kaynağı form'un başka bir alanı olmayan bir `depends_on` bildirir.

## Filtre formları

Submit düğmesi olmayan bir form screen'i filtreleyebilir: `.drives(topic)` alanlarını screen params yapar. Bir alan değişince (bir seçim listesi, bir tarih aralığı, Enter ile bir metin alanı) değeri screen'in aynı adlı param'ına girer, adres güncel tutulur ve `.listens(topic)` diyen node'lar yerinde yenilenip değeri kendi params'larından okur ([Topic'ler](effects.md#topics) bölümüne bakın). Sayılar ve boolean değerler olduğu gibi, boş alanlar `null` olarak gönderilir; sunucu params'ı screen'in tipli params'ı olarak okur, bu yüzden uymayan bir değer reddedilir.

```rust
#[params]
pub struct SalesParams {
    pub region: Option<Region>,
    pub from: Option<Date>,
    pub to: Option<Date>,
}

#[screen(route = "/sales", permission = "sales.read")]
pub fn sales(p: SalesParams, store: Sales) -> Screen {
    Screen::new(t!("sales.title"))
        .body(
            Form::of(SalesFilter { region: p.region, from: p.from, to: p.to })
                .field(SalesFilter::region)
                .range(SalesFilter::from, SalesFilter::to)
                .drives("sales"),
        )
        .body(Kpi::new(t!("sales.total"), store.total(&p)).listens("sales"))
}
```

`submit` içeren bir form sürücü olamaz; onun yerine kaydettikten sonra `on_success` içinde `ui.refresh(topic)` ile yenileyin.

## Salt okunur alanlar

`#[readonly]` alanlar form'larda gösterilir ama düzenlenemez; yalnızca hareketlerin değiştirdiği bir stok düzeyi buna örnektir.

## Dosya yüklemeleri

`File`, `Option<File>` ve `Vec<File>` alanları, bir `FileStore` tarafından tutulan dosyaları taşır. Kuralları bir kez `const` olarak bildirin, store'u bir izinle Feature'a kaydedin ve aynı kuralları form'da kullanın:

```rust
pub const ATTACHMENTS: FileRules = FileRules::new("attachments")
    .max_bytes(5 * 1024 * 1024)
    .max_files(5)
    .accept(&[FileType::Pdf, FileType::Png, FileType::Jpeg]);

Feature::new("support")
    .files(ATTACHMENTS, "support.ticket.files", DiskStore::new("data/attachments"))
    .scan_files(MyVirusScanner)

Form::of(ticket).field(Ticket::title).file(Ticket::attachments, ATTACHMENTS)
```

Tek bir izin adı aynı kullanıcılara hem yükleme hem indirme verir. Okuyucuların ekleyemedikleri dosyaları açabilmesi için ikiye ayırın: `.files(ATTACHMENTS, FileAccess::read("support.ticket.read").write("support.ticket.files"), store)`.

Kontrol, bir bırakma alanının içindeki yerel bir dosya girdisidir. Her dosya kendi ilerleme çubuğuyla ayrı ayrı yüklenir; çalışırken iptal edilebilir, başarısız olduysa yeniden denenebilir ve bittiğinde kaldırılabilir. Görseller, seçilen dosyanın bir nesne URL'sinden üretilen bir önizleme gösterir. İlerleme ve sonuçlar, nazik (polite) bir canlı bölgede duyurulur. Showcase'in `/uploads` adresinde çalışan bir örnek vardır; bu dokümantasyon sitesinde oturum açmış kullanıcı olmadığı için canlı yükleme göstermez.

Bırakılan bir klasör, içindeki ve alt klasörlerindeki dosyaları ekler; klasör, bitene kadar en fazla 100 girdilik sayfalarla okunur. Alanın almadığı türdeki dosyalar atlanır ve sayıları canlı bölgede duyurulur. Alan odaktayken yapıştırılan dosyalar da yüklenir. Uzantısı veya türü `accept` içinde olmayan bir dosya, tek bir bayt gönderilmeden tarayıcıda reddedilir; sunucu içeriği yine denetler. Bir alanın en fazla üç dosyası aynı anda yüklenir; diğerleri meşgul bir ilerleme çubuğuyla sırada bekler.

| Tuş | Ne yapar |
|---|---|
| Tab | Dosya girdisine, sonra her dosyanın indirme bağlantısına ve İptal, Yeniden dene ya da Kaldır düğmesine geçer |
| Dosya girdisinde Enter veya Space | Sistemin dosya seçicisini açar (sürükle bırak için klavye yolu) |
| Dosya girişinde Ctrl+V veya Cmd+V | Panodaki dosyaları, örneğin bir ekran görüntüsünü yükler |
| İptal, Yeniden dene, Kaldır üzerinde Enter veya Space | Çalıştırır; İptal'den sonra odak Yeniden dene'ye, Kaldır'dan sonra sonraki dosyaya ya da girdiye döner |

Sunucunun `POST /_zeyn/files/{store}` için yaptıkları:

1. CSRF başlığını, store'un iznini ve Feature'ın guard'larını denetler ve interceptor'ları çalıştırır (`CallKind::Upload`, yeni dosya kimliğiyle denetlenir); yüklemeler yazma sınırına sayılır;
2. bildirilen `Content-Length` değeri sınırın üzerindeyse reddeder (`413`), sonra ilk 4 KiB'ı okur ve türü içerikten bulur; uzantı aynı türe ait olmalı ve tür `accept` içinde bulunmalıdır, aksi halde `415`. HTML, SVG, XML ve betikler asla kabul edilmez. Tarayıcının içerik türü yok sayılır;
3. kalanını, dörtlük bir kanaldan geçirerek en çok 64 KiB'lık parçalarla store'a akıtır ve baytları sayar; sınırı aşmak, kopan bir bağlantı ya da 30 saniye veri gelmemesi yüklemeyi durdurur ve store yazdığını siler;
4. Feature'ın `FileScanner`'ını çalıştırır; reddedilen bir dosya ya da başarısız olan bir tarayıcı dosyanın silinmesine yol açar ve yanıt `422` olur;
5. `{id, name, size, type}` ile `201` yanıtı verir. Kimlik, sunucudan gelen 128 rastgele bittir; adlar yollardan ve denetim karakterlerinden temizlenir.

`image` özelliğiyle `FileRules::clean_images()`, dosya depoya yazıldıktan sonra bir adım ekler: PNG, JPEG ve WebP yüklemeleri çözülür ve aynı biçimde yeniden kodlanır; böylece EXIF (GPS ile), XMP, ICC profilleri, metin parçaları ve görüntü verisinin ardına gizlenmiş baytlar sunucuda kalmaz. EXIF yönü önce piksellere uygulanır; JPEG 90 kaliteyle, PNG ve WebP kayıpsız yazılır. Çözme, bir kenarda `MAX_IMAGE_SIDE` (12.000) pikselde ve `MAX_IMAGE_MEMORY` (256 MiB) bellekte durur; çözülemeyen bir görüntü silinir ve yanıt `422` olur. İki sınır da herhangi bir piksel çözülmeden önce görüntü başlığındaki boyuta göre denetlenir; böylece dev bir görüntü olduğunu söyleyen küçük bir dosya o belleği almadan reddedilir. Hareketli bir görüntüden yalnızca ilk kare kalır. Temizlenen dosya özgün dosyanın yerine yeni bir kimlikle geçer ve tarayıcı (scanner) ardından temizlenen dosyayı denetler. Özellik olmadan bu yöntem yoktur; böylece bir depo temizlenmediği hâlde temizlendiğini sanamaz.

İndirmeler (`GET /_zeyn/files/{store}/{id}`) kimliğin biçimini, izni, guard'ları ve tenant'ı denetler ve medya sunumundan geçer (`docs/media.md` dosyasına bakın): aralıklar (range), entity tag'ler ve `nosniff`. Yalnızca raster görseller, ses ve video satır içi sunulur; geri kalan her şey `Content-Disposition: attachment` ile, tırnaklı bir ASCII ad ve UTF-8 adla birlikte, `Content-Security-Policy: sandbox` altında `application/octet-stream` olarak gönderilir.

`DiskStore`, `{id}.bin` ve `{id}.json` dosyalarını tek bir klasörde tutar ve yüklemeleri önce `{id}.part` olarak yazar. `MemoryStore` dosyaları bayt üst sınırıyla bellekte tutar; testler ve demolar içindir. Form değeri yalnızca dosyaları adlandırır: bir dosyanın var olduğundan emin olması gereken bir kaydetme action'ı, showcase'in yaptığı gibi `store.meta(id)` çağırır.

`FileRules::quota(bytes, window)` bir kullanıcının bir depoya belirli bir sürede yükleyebileceği miktarı sınırlar ve uygulamanın rate store'u ile sayılır; sınırı aşan yükleme yine kaydedilir, sonraki `429` alır. `DiskStore`, bir saatten eski `.part` dosyalarını (çökme nedeniyle yarım kalan yüklemeler) saatte en fazla bir kez siler. `ZipGuard`, ZIP tabanlı yüklemeler (ZIP, Word, Excel, PowerPoint) için bir tarayıcıdır: merkezi dizini 10.000'den fazla girdi, toplam 1 GiB'tan fazla veya dosya boyutunun 200 katından fazla açılacağını söyleyen arşivleri ve ZIP64 arşivlerini reddeder; kabul edilen bir arşivi açan kod da bu sınırlarda durmalıdır.

Bütçe: 256 MiB'lık bir yükleme sunucu heap'ini yaklaşık 0,4 MiB büyütür (`tests/files_memory.rs`); kontrol 10 DOM düğümü artı dosya başına 5 düğüm çizer; `files.js` ham 17 KB ve gzip 6 KB, `files.css` ham 3 KB ve gzip 1 KB ekler.

## File drop

`FileDrop::new(label, rules)`, aynı yükleme kontrolünün form dışında, tek başına kullanılan hâlidir; bir belge gelen kutusu gibi işin tamamının dosyalar olduğu ekranlar içindir. Form alanı gibi klasörleri, yapıştırılan dosyaları ve kabul listesini alır ve `rules` deposuna yükler; bu depoyu izniyle birlikte `Feature::files` kaydeder. `.single()` bir seferde tek dosya alır. `.on_upload(action)` saklanan her `File` için bir eylem çalıştırır; sunucu önce kimliğin kendi ürettiği biçimde olduğunu denetler, adı yeniden temizler ve yalnızca kurallardaki bir içerik türünü bırakır, böylece eylem tarayıcının uydurduğu hiçbir şeyi almaz. Eylem reddederse öğe nedeni gösterir. Alanın içindeki dosya seçici, klavye ve tek işaretçi yoludur.

```rust
#[action(permission = "support.ticket.files")]
pub async fn attach(file: File, tickets: Tickets) -> Result<()> {
    tickets.attach(file).await
}

FileDrop::new(t!("ticket.attachments"), ATTACHMENTS).on_upload(attach)
```
