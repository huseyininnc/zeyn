# Yönetim görünümleri

Arka ofis için bileşenler: bir kayıtta ne değiştiği, salt okunur kod ve veri, barkodlar, bildirimler, panolar ve takvimler. Her biri sunucuda çizilir ve stil dosyasını ve betiğini yalnızca onu kullanan sayfalarda yükler.

## DiffView

`DiffView`, bir kaydın iki değeri arasında değişen alanları gösterir: eski değerin üstü çizilir, yenisi eklenmiş olarak işaretlenir. Bir denetim kaydından `DiffView::changes(entry.changes)` ile, bir modelin iki değerinden `DiffView::of(&before, &after)` ile kurulur; ikincisi değerleri denetim izinin yaptığı gibi karşılaştırır (satır sürümleri dışarıda kalır).

```rust
DiffView::of(&before, &after).title(t!("orders.history"))

DiffView::changes(entry.changes.clone())
    .model::<Order>()
    .layout(DiffLayout::Inline)
```

`.model::<M>()` alanlara kullanıcının dilinde etiketlerini, seçimlere de seçenek etiketlerini verir. Yan yana düzen, eski değerler için bir sütunu ve yeni değerler için bir sütunu olan bir tablodur; `DiffLayout::Inline` yeni değeri eskisinin arkasına koyar, pencere gibi dar yerler içindir. En fazla `MAX_DIFF_FIELDS` (200) alan gösterilir, geri kalanı için bir not çıkar.

Maskeli alanlar maskeli kalır. Değerler denetim izinin tuttuğu maskeli metindir ve modelin `#[mask]` ile işaretlediği bir alan gösterilmeden önce yeniden maskelenir; böylece elle kurulmuş bir değişiklik listesi bile sayfaya ham değer koymaz. Bileşenin betiğe ihtiyacı yoktur; `diff.css` (1,9 KB) yalnızca onunla yüklenir.

## CodeView ve JsonView

`CodeView` kaynak metni salt okunur gösterir: sunucuda renklendirilir, satır numaraları, katlama ve bir kopyalama düğmesi vardır. `JsonView` bir JSON değerini aynı şekilde girintili gösterir.

```rust
CodeView::new(CodeLang::Sql, query_text).title("report.sql")

JsonView::new(&payload).title(t!("webhooks.last_payload"))

JsonView::of(&customer, &ctx)
```

Renklendirici (`zeyn::ui::highlight`) Rust, JSON, TOML, SQL, JavaScript ve kabuk için elle yazılmıştır; `CodeLang::Plain` ve başka her ad kaçışlı düz metin gösterir. Anahtar sözcükleri, dizeleri, sayıları, yorumları, türleri ve anahtarları renkleri tema belirteçlerinden gelen `z-hl-*` span'lerine sarar; böylece iki tema da çalışır ve tarayıcı hiç iş yapmaz. Birden çok satıra yayılan parantez blokları (`{}` ve `[]`) betik gerektirmeyen yerel bir `<details>` öğesiyle katlanır: ilk satır düğmedir (Enter veya Space) ve katlanmış bir blok üç noktayla biter.

`JsonView::of(value, &ctx)` herhangi bir değeri uç noktaların tarayıcıya gönderdiği gibi serileştirir, böylece maskeli alanlar maskeli kalır. En fazla `MAX_CODE_LINES` (10.000) satır ve `MAX_CODE_BYTES` (1 MB) gösterilir; daha uzun bir metin bir satır sonunda kesilir ve kaç satırın gösterildiğini söyleyen bir not çıkar. Kopyalama düğmesi (`Share` için de çalışan `share.js`) gösterilen satırları kopyalar ve bir bildirimle onaylar; betiği çalışana kadar gizlidir. `code.css` (2,7 KB) yalnızca bileşenle birlikte yüklenir.

| Tuş | Ne yapar |
|---|---|
| Tab | Metne geçer (tek durak, ok tuşlarıyla kayar), sonra katlama düğmelerinden ve kopyalama düğmesinden geçer |
| Enter, Space | Bir katlama düğmesinde: bloğu katlar veya açar |

## Barcode ve QrCode

`Barcode::code128(value)` ve `QrCode::new(value)` bir sembolü sunucuda SVG olarak çizer. İki kodlayıcı da herkese açık standartlara (Code 128 ve QR Code) göre elle yazılmıştır; bu yüzden ne crate ne de betik eklenir.

```rust
Row::new()
    .col(Barcode::code128(order.number.clone()).height(56))
    .col(QrCode::new(order_url).level(QrLevel::Quartile).label(t!("orders.scan")))
```

`Barcode` 1 ile 80 arasında yazdırılabilir ASCII karakteri (`MAX_BARCODE_CHARS`) kodlar ve rakam dizilerini bir sembolde iki rakam olacak şekilde paketler (C kod kümesi); kontrol sembolünü ve her iki yanda 10 modüllük boşluk bölgesini ekler. `QrCode` metni UTF-8 bayt olarak, seçilen düzeyde (`QrLevel::Low`, varsayılan `Medium`, `Quartile`, `High`) onu taşıyan en küçük sürümle (1 ile 40) kodlar; Reed Solomon hata düzeltmesi, en düşük cezalı maske ve 4 modüllük boşluk bölgesi kullanır; en fazla `MAX_QR_BYTES` (2.953) bayt. Kodlanamayan bir değer bozuk bir sembol yerine kısa bir not gösterir.

Her sembol, ekran okuyucuların altındaki metinle adlandırılmış tek bir görsel olarak duyduğu bir figürdür: değerin kendisi ya da verilmişse `.label(text)`. Çubuklar iki temada da beyaz bir zemin üzerinde siyah kalır, çünkü tarayıcı cihazlar açık zemin üzerinde koyu çubuk ister. `barcode.css` (1 KB) yalnızca bileşenlerle yüklenir; `.height(px)` ve `.size(px)` çizilen boyutu belirler ve sembol dar bir kapsayıcıda küçülür.

## Inbox ve NotificationCenter

`Inbox` kullanıcının bildirimlerini en yeni başta olmak üzere, kullanıcının saat diliminde güne göre gruplayarak (bugün, dün, sonra tarihler) listeler ve okunmamışları işaretler. Satırlar DataGrid'in kullandığı türde bir eylemden gelir (`DataQuery` girer, `Page` çıkar); gelen kutusu zaman alanına göre sıralar ve en fazla `.limit(n)` satır ister (50, en fazla `MAX_INBOX`, 200). `NotificationCenter` üst çubuğun zilidir: okunmamış sayısıyla gelen kutusu ekranına bir bağlantı.

```rust
#[action(permission = "notes.write")]
pub async fn mark_notes_read(s: InboxSelection, store: Notes, ctx: Ctx) -> Result<()> {
    store.mark_read(ctx.identity(), &s.ids)
}

#[action(permission = "notes.read")]
pub async fn count_unread(q: UnreadCount, store: Notes, ctx: Ctx) -> Result<u32> {
    store.count_unread(ctx.identity(), q.limit)
}

Inbox::of(my_notifications)
    .when(Note::created_at)
    .read(Note::read)
    .item(|n: &Note| ListItem::new(n.title.clone()).subtitle(n.body.clone()))
    .mark_read(mark_notes_read)
    .archive(archive_notes)

App::new("Admin").header(|h| h.end(NotificationCenter::of(count_unread, inbox, ())))
```

`.mark_read(action)` ve `.archive(action)` ile her satıra bir onay kutusu, gelen kutusuna da "Tümünü seç" içeren bir araç çubuğu gelir. Düğmeler seçilen anahtarları bir `InboxSelection` olarak `POST /_zeyn/data` üzerinden gönderir; bu uç nokta CSRF'yi denetler, yazma sınırına sayılır, ekranın erişimini ve eylemin iznini denetler; en fazla 200 karakterlik en fazla 200 anahtar kabul edilir. Anahtarlar tarayıcıdan gelir, bu yüzden eylem yalnızca `ctx.identity()` kullanıcısının bildirimlerini değiştirir. Kullanıcının çalıştıramayacağı düğmeler, onay kutularıyla birlikte dışarıda kalır.

Bir toplu işlemden sonra gelen kutusu konusunu adlandırır (`"notifications"`, değiştirmek için `.topic(..)`): gelen kutusu ve onu `.listens` ile dinleyen her düğüm yerinde yeniden çizilir, zil de sayısını yeniden yükler. Zil, `UnreadCount { limit }` alan ve sınırda (100) duran bir eylemle sayar, yani tek ve sınırlı bir sorgu; 99'un üstünde `99+` gösterir ve bağlantının adı ekran okuyucular için sayıyı söyler. Sayıyı sayfa göründükten sonra tek bir istekle yükler; konusu bir gelen kutusu, `ui.refresh("notifications")` ya da `.live()` ve `realtime` özelliğiyle herhangi bir eylemdeki `hub.notify(&ctx, "notifications")` tarafından adlandırıldığında yeniden yükler (konuyu `Realtime::topic` ile bildirin). `inbox.js` (6,1 KB, gzip ile 2,3 KB) ve `inbox.css` (3,1 KB) yalnızca bileşenlerle yüklenir.
