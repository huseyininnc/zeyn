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
