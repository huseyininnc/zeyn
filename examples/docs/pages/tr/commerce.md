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


### Kartlarda fiyat

`ImageCard::price(Price)` aynı fiyatı bir `CardGrid` ya da `Rail` içindeki kartın metninin altına, kartın bağlantısının içine koyar; böylece ekran okuyucular başlığı ve fiyat cümlesini birlikte duyar. Script eklemez; mağaza stil dosyası fiyatı olan ilk kartla gelir.

```rust
CardGrid::of(catalog_products).card(|p: &Product| {
    Card::image(p.photo.clone(), p.name.clone())
        .price(Price::new(p.price).was(p.list_price))
        .link(product_page, ProductPage { id: p.id })
})
```

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

## Adet seçici

```rust
#[action(permission = "cart.edit")]
pub async fn set_quantity(change: QuantityChange, cart: CartStore) -> Result<QuantityState> {
    let stock = cart.stock(&change.target)?;
    let kept = change.quantity.min(stock);
    cart.set(&change.target, kept)?;
    Ok(QuantityState::new(kept).max(stock))
}

QuantityStepper::of(set_quantity, line.sku.clone())
    .label(t!("cart.quantity"))
    .range(1, 20)
    .value(line.quantity)
```

- **Sunucuda denetlenir.** Bir değişiklik (eksi ya da artıya basmak veya bir sayı yazmak) kullanıcı durduktan 400 ms sonra `POST /_zeyn/data` ile gönderilir: CSRF denetimi ve yazma hız sınırıyla, kullanıcı olarak, action'ın izni denetlenerek. Sunucu action çalışmadan önce adedi seçicinin aralığına kırpar; action yalnızca kendisinin bildiği şeyi, örneğin stoğu denetler. Kalem (`target`) ekran tarafından verilir ve tarayıcıdan hiç okunmaz.
- **Son söz cevabındır.** Action tuttuğu adedi ve izin verilen en çok adedi döndürür (`QuantityState::new(kept).max(stock)`). Bu, kullanıcının istediğinden azsa seçici onu gösterir ve bir canlı bölgede "Yalnızca 7 adet var." der. Yeni bir değişiklik yoldaki isteği iptal eder; böylece yalnızca son cevap gösterilir. Başarısız bir istek son tutulan adedi geri koyar ve bunu söyler.
- **Klavye ve dokunma.** Düğmeler en az 24 x 24 pikseldir (WCAG 2.5.8), "Azalt" ve "Artır" adlarını taşır ve aralığın uçlarında kapanır; basılan düğme kapanınca odak alana geçer. Alan rakamları ve ok tuşlarını kabul eder.
- **Script.** `quantity.js` (gzip ile yaklaşık 1 KB) yalnızca seçici olan sayfalarda yüklenir.

## Varyant seçici

```rust
VariantPicker::new()
    .group(
        VariantGroup::swatches("color", t!("shop.color"))
            .swatch("sand", t!("shop.sand"), "#d8c3a5")
            .swatch("navy", t!("shop.navy"), "#1f3a68")
            .current(p.color.as_deref()),
    )
    .group(
        VariantGroup::chips("size", t!("shop.size"))
            .option("m", "M")
            .option("xl", "XL")
            .unavailable("xl", t!("shop.not_in_color"))
            .current(p.size.as_deref()),
    )
    .drives("variant")
```

- **Radyo grupları.** Her grup yerel radyo düğmelerinden oluşan bir fieldset'tir; script olmadan W3C APG radio group desenidir: Tab seçili seçeneğe gelir, ok tuşları gezer ve seçer, başlık grubu ve seçili seçeneği söyler ("Renk Lacivert"). Renk örnekleri bir hex renk ya da tema token'ı alır; çipler metinlerini gösterir.
- **Olmayan seçenekler** bir gerekçeyle kapatılır (`unavailable(value, reason)`); ekran okuyucular gerekçeyi seçenekle birlikte okur, ipucu da gösterir. Renk örneklerine çapraz çizgi, çiplere üstü çizili yazı gelir.
- **Fiyat, fotoğraf ve stok izler.** Seçici konularını sürer ve kendisi de onları dinler. Bir seçeneği seçmek değerini grubun adındaki ekran parametresine yazar, adresi güncel tutar (böylece seçili varyantın bağlantısı paylaşılabilir) ve konuyu dinleyen her node'u tek istekte yeniler: fiyata, galeriye ve stok satırına `.listens("variant")` koyun; ekran yeni seçim için seçenekleri yeniden kapatır. Konular modülünden başka script gerekmez.
- **Denetimler.** `App::check_page` hiçbir şey sürmeyen bir seçiciyi ve geçerli rengi olmayan bir renk örneğini bildirir.

## Ürün galerisi

```rust
p.photos.iter().fold(ProductGallery::new(p.name.clone()), |g, photo| {
    g.image(
        Image::new(photo.large.clone())
            .alt(photo.alt.clone())
            .size(800, 800)
            .low(photo.tiny.clone())
            .placeholder(Placeholder::color("--z-surface-2")),
    )
    .thumb(photo.thumb.clone())
})
```

- **Bir seferde bir görsel.** Slaytlar kaydırınca yerine oturan bir şeritte durur; telefonlar script olmadan aralarında kaydırır. Görselin altındaki küçük resimler düğmedir ("5 fotoğraftan 2. fotoğrafı göster", geçerli olanda `aria-current`); odaktaki görselde ok tuşları sayfanın okuma yönünde slaytlar arasında gezer. Galeri adlandırılmış bir carousel bölgesidir, slaytları "2 / 5" der (W3C APG carousel deseni). Küçük resimler (`thumb`) yoksa numaralı noktalar gösterilir.
- **Yalnızca görülen yüklenir.** İlk görsel `fetchpriority="high"` ile hemen yüklenir, çünkü genellikle ürün sayfasındaki en büyük öğedir; diğerleri etkisiz şablonlarda bekler ve `gallery.js` ekrandaki görseli ve iki komşusunu tek bir IntersectionObserver ile izleyerek yerleştirir. 12 fotoğraflık bir galeri başta üçünü yükler. Görseller `Image` olduğu için yer tutucularını, düşük çözünürlüklü dosyalarını ve yedeklerini korur.
- **Yakınlaştırma ve tam ekran.** Fare, imlecin altındaki görseli yakınlaştırır (slaytın kutusu imleç girince bir kez okunur, hareket ederken hiç okunmaz). Görsele basmak ya da "Tam ekran" düğmesi görseli, önceki ve sonraki düğmelerini, yakınlaştırma düğmesini ve kapatma düğmesini içeren modal bir diyalog açar; odağı içeride tutar, Escape ile kapanır ve odağı açan öğeye geri verir. Azaltılmış hareket tercihinde hiçbir şey kaymaz ya da büyümez.
- **Script.** `gallery.js` (gzip ile yaklaşık 1,5 KB) yalnızca galeri olan sayfalarda yüklenir.

## Facet paneli

```rust
#[params]
#[derive(Default)]
pub struct Listing {
    #[serde(default)]
    pub color: Vec<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
}

#[screen(route = "/shop", public)]
pub async fn shop(p: Listing, store: ProductStore) -> Result<Screen> {
    let colors = store.count_colors(&p).await?;
    let panel = FacetPanel::of(shop, &p)
        .facet(colors.iter().fold(Facet::new("color", t!("shop.color")), |f, c| {
            f.option(c.value.clone(), c.name.clone(), c.count)
        })
        .chosen(p.color.clone()))
        .range(RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
            .money(Currency::Try)
            .value(p.min_price, p.max_price))
        .drives("products");
    let cards = CardGrid::of(search_products)
        .card(|p: &Product| Card::image(p.photo.clone(), p.name.clone()).price(Price::new(p.price)))
        .scope_any(Product::color, p.color.clone())
        .scope_range(Product::price, p.min_price.map(|m| Money::tl(m * 100)), p.max_price.map(|m| Money::tl(m * 100)))
        .listens("products");
    Ok(Screen::new(t!("shop.title")).body(Row::new().col(panel).col(cards).weight(3)))
}
```

- **Sayılar sunucudan.** Her facet, diğer facet'ler uygulanmışken her seçeneğin kaç satırı olduğunu gösteren bir onay kutusu grubudur ("Kırmızı (124)"; ekran okuyucular "Kırmızı, 124 sonuç" duyar). Ekran bunları facet başına sınırlı bir sorguyla sayar, örneğin diğer filtreler `WHERE` içindeyken `SELECT color, count(*) ... GROUP BY color LIMIT 50`; bir facet en çok `MAX_FACET_OPTIONS` (50) seçenek alır. Satırı olmayan seçenek, seçili değilse kapatılır.
- **Her değişiklikte tek istek.** Panel, konularını süren ve kendisi de onları dinleyen bir filtre formudur. Bir değişiklik değerleri ekran parametrelerine yazar (facet'ler liste, aralık iki sayı), adresi güncel tutar ve tüm dinleyicileri tek `POST /_zeyn/nodes` ile yeniler: kart ızgarası, bir sayı satırı ve yeni sayılarıyla panelin kendisi. Yeni bir değişiklik yoldaki isteği iptal eder. 2.000 üründe ölçüldü: tıklamadan yeni kartlara ve sayılara 26 ms.
- **Filtreler sunucuda kalır.** Izgara aynı parametreleri `scope_any(field, values)` ve `scope_range(field, from, to)` ile alır (`CardGrid` ve `DataGrid` üzerinde); böylece tarayıcı hiçbir zaman kendi filtresini göndermez. Değer olmaması ya da açık uçlar her satırı tutar.
- **Seçili filtreler** grupların üstünde çip olarak, "Tümünü temizle" ile birlikte görünür. Her çip o filtre olmadan ekrana giden bir bağlantıdır; script olmadan da çalışır ve "Kırmızı filtresini kaldır" adını taşır. Gruplar katlanan `<details>` öğeleridir ve açık gruplar yenilemelerden sonra açık kalır.
- **Denetimler.** `App::check_page` hiçbir şey sürmeyen bir paneli ve adı ekran parametrelerinde alan olmayan bir facet'i bildirir.
