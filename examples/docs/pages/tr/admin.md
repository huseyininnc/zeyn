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
