# Mağaza bileşenleri

Ürün listeleri ve ürün sayfaları kendilerine ait birkaç bileşen ister: indirimli fiyatlar, sayılı filtreler, aralık kaydırıcısı, varyant seçiciler, galeri, adet seçiciler, değerlendirme özetleri ve geri sayımlar. Hepsi diğer bileşenler gibi sunucuda çizilir. Tarayıcı kodu gerekenler küçük bir modülü yalnızca kendilerini taşıyan sayfalarda yükler; stilleri (`commerce.css`) de yalnızca orada bağlanır.

## Fiyat

```rust
Price::new(p.price)
    .was(p.list_price)
    .unit(p.price_per_kg, PriceUnit::Kg)
    .large()
```

- **İndirim.** `was(old)` eski fiyatı güncel fiyatın yanında üstü çizili gösterir ve tam sayıya yuvarlanmış indirim yüzdesini ekler. Güncel fiyattan yüksek olmayan ya da başka para biriminde olan eski fiyat gösterilmez; böylece eskimiş bir indirim hiçbir zaman eksi yüzde göstermez.
- **Ekran okuyucular** tek bir cümle duyar: "Eski fiyat 1.599,00 ₺, yeni fiyat 1.299,00 ₺, %19 indirim". Üstü çizili fiyat ve rozet onlardan gizlenir, çünkü çoğu ekran okuyucu metnin üstünün çizili olduğunu söylemez.
- **Birim fiyat.** `unit(price, PriceUnit::Kg)` "Kg fiyatı 259,80 ₺" gibi daha sönük bir satır ekler. Birimler `Item`, `Kg`, `Grams100`, `Liter`, `Ml100` ve `Meter`.
- **Biçim.** Tutarlar sayfanın sayı biçimindeki `Money` değerleridir: İngilizcede `₺1,299.00`, Türkçede `1.299,00 ₺`; dil yüzde işaretini öne koyuyorsa işaret öne gelir (`%19`). `large()` ürün sayfası için güncel fiyatı büyütür.
- **Script yok.** Fiyat düz HTML'dir.

## Aralık kaydırıcısı

```rust
RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
    .step(50)
    .money(Currency::Try)
    .value(p.min_price, p.max_price)
    .drives("products")
```

- **İki yerel tutamaç.** Her tutamaç iki addan birini taşıyan bir `<input type="range">`'dir; bu yüzden tarayıcı ona W3C APG slider davranışını verir: her birinde bir sekme durağı, ok tuşları adım kadar kaydırır, Home ve End uçlara gider, ekran okuyucular biçimlenmiş değeri (`aria-valuetext`, örneğin "500,00 ₺") ve etiketi ("Fiyat, başlangıç", "Fiyat, bitiş") duyar. Page Up ve Page Down aralığın onda biri kadar kaydırır.
- **Filtre.** `.drives(topic)` kaydırıcıyı bir filtre formuna koyar: bir tutamaç bırakılınca iki değer aynı adlı ekran parametrelerine yazılır ve konunun dinleyicileri yerinde yenilenir. Bir facet panelinin içinde bunu panelin formu yapar; başka bir formda tutamaçlar diğer alanlar gibi gönderilir.
- **Değerler.** `value(low, high)` parametreleri alır (`None` ilgili ucu korur). Sınır dışındaki değerler kırpılır, yüksekten büyük alt değer yer değiştirir; böylece elle düzenlenmiş bir adres kaydırıcıyı bozamaz. `money(currency)` bir para biriminin tam birimlerini sayfanın biçiminde gösterir.
- **Tarihler.** `RangeSlider::dates(label, names, first, last)` ve `.date_value(from, to)` gün seçer; form `2026-03-31` gibi ISO tarihleri gizli alanlarla gönderir.
- **Script.** `range.js` (gzip ile yaklaşık 1 KB) yalnızca kaydırıcı olan sayfalarda yüklenir. Alt tutamacı üsttekinin altında tutar, tutamaçlar arasını doldurur, değerleri etiketin yanına yazar ve çubuğun boş yerine basılınca yakın tutamacı taşır. Çubuğun boyutunu her basışta bir kez okur, tutamaç kayarken hiç okumaz.
