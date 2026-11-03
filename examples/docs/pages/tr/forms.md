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
| `.step(title)` | Bir sihirbaz adımı; form bir seferde tek adım gösterir |
| `.range(M::from, M::to)` | Tek bir aralık seçici olarak iki tarih |
| `Form::auto(value)` | Her alan, tanım sırasıyla |

## Kaydetme

`.submit(label, action)` kaydeden action'ı adlandırır. Action, form'un modeli için `Valid<M>` almalıdır; başka bir şey derlenmez. Runtime değerleri gönderir, sunucu her kuralı, ardından action'ı çalıştırır.

- Kurallardan ya da action içindeki `ValidationErrors` değerinden gelen alan hataları alanların yanında görünür.
- `Error::Conflict` (eski bir `#[version]`), kaydı başka birinin değiştirdiğini söyleyen bir ileti gösterir.
- `.on_success(|result, ui| ...)` başarılı bir kayıttan sonra effect'leri çalıştırır ([Effects and windows](effects.md) sayfasına bakın).

## Kontroller

Her alan türünün varsayılan bir kontrolü vardır: metin girdileri, sayı girdileri, para birimiyle birlikte para, tarih ve saat seçiciler, `bool` için onay kutuları ve seçenekler için seçim listeleri. `.control(M::f, Control::...)` başka bir tanesini seçer, `.lookup(M::f, action)` ise kayıtları sunucuda arar. [Controls and pickers](controls.md) sayfasına bakın.

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

Kontrol, bir bırakma alanının içindeki yerel bir dosya girdisidir. Her dosya kendi ilerleme çubuğuyla ayrı ayrı yüklenir; çalışırken iptal edilebilir, başarısız olduysa yeniden denenebilir ve bittiğinde kaldırılabilir. Görseller, seçilen dosyanın bir nesne URL'sinden üretilen bir önizleme gösterir. İlerleme ve sonuçlar, nazik (polite) bir canlı bölgede duyurulur. Showcase'in `/uploads` adresinde çalışan bir örnek vardır; bu dokümantasyon sitesinde oturum açmış kullanıcı olmadığı için canlı yükleme göstermez.

| Tuş | Ne yapar |
|---|---|
| Tab | Dosya girdisine, sonra her dosyanın indirme bağlantısına ve İptal, Yeniden dene ya da Kaldır düğmesine geçer |
| Dosya girdisinde Enter veya Space | Sistemin dosya seçicisini açar (sürükle bırak için klavye yolu) |
| İptal, Yeniden dene, Kaldır üzerinde Enter veya Space | Çalıştırır; İptal'den sonra odak Yeniden dene'ye, Kaldır'dan sonra sonraki dosyaya ya da girdiye döner |

Sunucunun `POST /_zeyn/files/{store}` için yaptıkları:

1. CSRF başlığını, store'un iznini ve Feature'ın guard'larını denetler ve interceptor'ları çalıştırır (`CallKind::Upload`, yeni dosya kimliğiyle denetlenir); yüklemeler yazma sınırına sayılır;
2. bildirilen `Content-Length` değeri sınırın üzerindeyse reddeder (`413`), sonra ilk 4 KiB'ı okur ve türü içerikten bulur; uzantı aynı türe ait olmalı ve tür `accept` içinde bulunmalıdır, aksi halde `415`. HTML, SVG, XML ve betikler asla kabul edilmez. Tarayıcının içerik türü yok sayılır;
3. kalanını, dörtlük bir kanaldan geçirerek en çok 64 KiB'lık parçalarla store'a akıtır ve baytları sayar; sınırı aşmak, kopan bir bağlantı ya da 30 saniye veri gelmemesi yüklemeyi durdurur ve store yazdığını siler;
4. Feature'ın `FileScanner`'ını çalıştırır; reddedilen bir dosya ya da başarısız olan bir tarayıcı dosyanın silinmesine yol açar ve yanıt `422` olur;
5. `{id, name, size, type}` ile `201` yanıtı verir. Kimlik, sunucudan gelen 128 rastgele bittir; adlar yollardan ve denetim karakterlerinden temizlenir.

İndirmeler (`GET /_zeyn/files/{store}/{id}`) kimliğin biçimini, izni, guard'ları ve tenant'ı denetler ve medya sunumundan geçer (`docs/media.md` dosyasına bakın): aralıklar (range), entity tag'ler ve `nosniff`. Yalnızca raster görseller, ses ve video satır içi sunulur; geri kalan her şey `Content-Disposition: attachment` ile, tırnaklı bir ASCII ad ve UTF-8 adla birlikte, `Content-Security-Policy: sandbox` altında `application/octet-stream` olarak gönderilir.

`DiskStore`, `{id}.bin` ve `{id}.json` dosyalarını tek bir klasörde tutar ve yüklemeleri önce `{id}.part` olarak yazar. `MemoryStore` dosyaları bayt üst sınırıyla bellekte tutar; testler ve demolar içindir. Form değeri yalnızca dosyaları adlandırır: bir dosyanın var olduğundan emin olması gereken bir kaydetme action'ı, showcase'in yaptığı gibi `store.meta(id)` çağırır.

Bütçe: 256 MiB'lık bir yükleme sunucu heap'ini yaklaşık 0,4 MiB büyütür (`tests/files_memory.rs`); kontrol 10 DOM düğümü artı dosya başına 5 düğüm çizer; `files.js` ham 12 KB ve gzip 4 KB, `files.css` ham 3 KB ve gzip 1 KB ekler.
