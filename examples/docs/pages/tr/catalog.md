# Kartlar, ray ve arama

Bir mağaza gibi public sayfalar satırları tablo yerine kart olarak gösterir. `CardGrid`, `Rail` ve `SearchBox` satırlarını DataGrid'in kullandığı türden bir action ile yükler: action bir `DataQuery` alır ve bir `Page` döndürür. Sayfalama, sıralama ve arama metni sunucuda çalışır ve okumak, DataGrid'de olduğu gibi action'ın erişimini gerektirir.

```rust
#[action(public)]
pub async fn catalog_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?.into_iter().filter(|p| p.active).collect()))
}
```

## Kart ızgarası

```rust
CardGrid::of(catalog_products)
    .title(t!("shop.products"))
    .card(|p: &Product| {
        Card::image(p.image.clone(), p.name.clone())
            .text(p.price.to_string())
            .badge(Badge::new(t!("shop.new")).tone(Tone::Success))
            .link(shop_product, ProductPage { id: p.id })
    })
    .sort_option(t!("shop.sort.name"), Product::name, false)
    .sort_option(t!("shop.sort.cheapest"), Product::price, false)
    .image_size(300, 300)
    .paging(Paging::infinite(24))
```

- **Kartlar.** `Card::image(image, title)` bir satırdan tek bir kart üretir; `text`, `badge`, `alt` ve `link` ile birlikte kullanılır (`link` bir screen ve onun parametrelerini alır; kartın tamamı tek bir bağlantıdır ve sayfa yenilenmeden açılır). Görseller tembel yüklenir; genişlik ve yükseklik `image_size`'dan gelir, böylece sayfa zıplamaz. Görselin alternatif metni boştur, çünkü yanındaki başlık kartı adlandırır; resim daha fazlasını anlatıyorsa `alt` verin. Bir yol, `https:` ya da `data:image/` dışındaki görsel adresleri çıkarılır.
- **Karta ek öğeler.** `subtitle`, başlığın altına daha sönük ikinci bir satır ekler (yazar, kategori). `corner(badge)` bir rozeti görselin bitiş köşesine koyar; başlangıç köşesinde ise `badge` durur. `badges([...])` metnin altına etiket gibi bir rozet sırası ekler. `progress(fraction, label)` görselin alt kenarı boyunca, ne kadarının tamamlandığını gösteren ince bir çubuk çizer: oran 0 ile 1 arasındadır ve ekran okuyucular etiketi ve yüzdeyi okur (`aria-valuenow` ile `role="progressbar"`). `horizontal()` kartı bir satır olarak yerleştirir: başta küçük bir görsel, yanında metin; böyle kartlar içeren bir kart ızgarası ya da rail onlara daha geniş sütunlar verir. Hepsi `CardGrid` ve `Rail` içinde çalışır.

```rust
Card::image(c.cover.clone(), c.title.clone())
    .subtitle(c.teacher.clone())
    .corner(Badge::new(t!("courses.new")).tone(Tone::Success))
    .badges(c.tags.iter().map(|t| Badge::new(t.clone())))
    .progress(c.done, t!("courses.done"))
    .horizontal()
```
- **Hızlı ürün ızgaraları.** Izgaradaki `.placeholder(...)` her kart görselini yüklenirken doldurur ve bir kart kendi yer tutucusunu belirleyebilir. Bir kartta `.low(src)` önce küçük bir dosya gösterir, `.fallback(src)` ise kart alt metnini bir simgeyle göstermeden önce bir kez başka bir dosyayı dener. `.priority(n)` ilk `n` kartın (en fazla 12) görsellerini `fetchpriority="high"` ile hemen yükler; diğer her kart görseli tembel kalır ve sonradan yüklenen sayfalar hiçbir zaman öne geçmez. [`Image`](display.md#image) seçenekleri gibi çalışırlar ve tüm kartlar için tek bir dinleyici vardır.

```rust
CardGrid::of(catalog_products)
    .card(|p: &Product| {
        Card::image(p.photo.clone(), p.name.clone())
            .low(p.photo_small.clone())
            .fallback("/static/no-photo.webp")
    })
    .placeholder(Placeholder::color("--z-surface-2"))
    .priority(4)
```
- **İlk sayfa sunucuda.** İlk sayfa (kart ızgarasında 24, şeritte 12 kart ya da `paging` ile verilen boyut) sayfanın HTML'inin parçasıdır; arama motorları ve JavaScript'i olmayan ziyaretçiler kartları ve bağlantılarını görür, görseller boyutlarını taşıdığı için yüklenirken hiçbir şey kaymaz. Sonraki sayfalar tarayıcıdan gelir. Public bir sayfa her ziyaretçi için aynı HTML'dir, bu yüzden önbelleğe alınabilir. Route'u olan bir ekrana bağlanan kart, yönlendirmeye değil o ekranın kanonik adresine (`/items/{slug}`) bağlanır.
- **Sayfalama.** `Paging::infinite(n)` (varsayılan, 24) ekranda göründüğünde de tetiklenen bir "Show more" düğmesi gösterir; düğmeye basıldığında odak ilk yeni karta geçer. `Paging::pages(n)` önceki ve sonraki düğmelerini ve sayfa numarasını gösterir: action bir toplam döndürürse "Page 2 of 5", döndürmezse "Page 2"; o zaman Next, action bir imleç döndürdüğü sürece çalışır. Sayfa boyutunu sunucu belirler; istemci yalnızca nereden devam edileceğini söyler.
- **Sıralama.** Her `sort_option`, kullanıcıların bir seçim listesinden seçebileceği bir sıradır; ilki kartların başladığı sıradır. Sunucu yalnızca bu sıraları kabul eder ve istemcinin gönderdiği süzgeçleri ve arama metnini atar.
- **Kapsam.** `.scope(field, value)`, her istekte yalnızca alanı bu değere sahip satırları tutar, [`DataGrid::scope`](datagrid.md) gibi: bir yazarın makaleleri için `.scope(Article::author_id, author.id)`. Kart ızgaraları, rail'ler ve veriye bağlı listeler istemcinin gönderdiği her süzgeci atar; böylece istemci kapsamı ne değiştirebilir ne de kaldırabilir. Metin, büyük küçük harf yok sayılarak karşılaştırılır.
- **Arama.** `.search(text)` yalnızca serbest metinle eşleşen satırları gösterir ([`DataQuery::q`](#free-text)); örneğin bir sonuç screen'inin metni.
- **Maskeli alanlar.** Kartlar, ham satırı gören bir closure ile oluşturulur. `#[mask]` alanları olan bir model için kartları yalnızca bu alanları açma yetkisi olan kullanıcılar alır; herkes `Forbidden` alır. Public kartlar için maskeli alanı olmayan bir model kullanın.

### Üzerine gelince önizleme

Bağlantısı olan bir `ImageCard` ya da `ListItem` üzerindeki `.preview(screen, params)`, o ekranı kartın yanındaki bir panelde gösterir: fareyle yaklaşık yarım saniye üzerinde durunca ya da kart klavye odağı alınca hemen. Ekran yalnızca o zaman, bir pencereyle aynı erişim denetimleriyle `GET /_zeyn/screen/...` adresinden yüklenir ve her önizleme sayfanın ömrü boyunca saklanır (en fazla 24, `nav.js` başka bir ekran açınca temizlenir). Çerçeveler, nesneler, medya oynatıcıları ve betikler panel gösterilmeden önce kaldırılır, bu yüzden bir önizleme asla iframe yüklemez. Panel üst katmanda yaşar, bu yüzden içeriğini kırpan rail'ler ve ızgaralar onu kesmez; daha çok yer olan tarafta açılır, ekran kenarlarında yön değiştirir, kendi içinde kayar ve sayfayı asla kaydırmaz. WCAG 1.4.13'ü izler: işaretçi panelin üzerine geçebilir, Escape kapatır ve işaretçi ya da odak ayrılana kadar kalır. Dokunmatik ekranlarda ve kaba işaretçilerde önizleme yoktur ve dokunmak bağlantıyı açar; betik (`preview.js`, 2,5 KB gzip) orada hiç yüklenmez, başka yerde böyle bir kartın ilk üzerine gelişinde ya da odağında yüklenir. Azaltılmış harekette panel kısa büyüme animasyonu olmadan görünür.

```rust
Rail::of(search_products).card(|p: &Product| {
    Card::image(p.image.clone(), p.name.clone())
        .link(product, ProductParams { id: p.id })
        .preview(product_summary, ProductParams { id: p.id })
})
```

@live preview


### Builder'lar

`.card` ve `.item` varsayılan olarak kalır: bağlantıları, görselleri ve ekran okuyucu metnini hazır verirler. Bir satır başka bir biçim istediğinde `.builder(|row| node)`, Flutter'ın `ListView.builder`'ı gibi her satırı herhangi bir node olarak kurar; `List::of`, `CardGrid` ve `Rail` üzerinde çalışır. `.separator(node)` satırların arasına bir node koyar (satırları kendileri sayan ekran okuyuculardan gizlenir), `Repeat::of(items, |item| node)` ise screen'in zaten bellekte tuttuğu bir liste için aynısını en fazla 500 öğeye kadar yapar.

```rust
List::of(search_people)
    .builder(|p: &Person| Row::new().compact().col(Avatar::new(p.name.clone())).col(Paragraph::new(p.city.clone())))
    .separator(Paragraph::new("·"))

Repeat::of(order.steps.iter(), |s| Badge::new(s.name.clone())).separator(Paragraph::new("→"))
```

- **Anahtarlar.** Kurulan satırlar ve sanal bir listenin satırları modelin anahtarını `data-zeyn-key` içinde taşır; böylece listenin bir yenilemesi ([Topic'ler](effects.md#topics)) odağı ve açık details öğelerini, nereye taşınmış olursa olsun aynı kayıtta tutar.
- **Kurulan satırlar göstermek içindir.** Node'ları node yollarıyla adreslenmez; bu yüzden bir grid, bir form ya da bir lookup başka yere konur. `Repeat`'in node'ları çocuktur (`p.0`, `p.1`, ...) ve çalışmaya devam eder.
- **Sanal kaydırma.** `.paging(Paging::virtual_scroll())` olan bir liste sayfada yalnızca görünen satırları tutar; bunlar tüm satırlar kadar yüksek bir kaydırma alanının içindedir ve kullanıcı kaydırdıkça bloklar offset ile getirilir. Satırların tek bir sabit yüksekliği vardır, satır başına yükseklik yoktur: bir builder ile `.row_height(px)` zorunludur ve `App::check_page` eksikliğini bildirir (`separator` orada kullanılmaz). Her satır `aria-posinset` ve `aria-setsize` taşır; böylece ekran okuyucular satırın tüm listedeki yerini duyar. Kaydırma alanı, başlıkla adlandırılan odaklanabilir bir bölgedir.

## Rail

Rail, yana kayan tek satıra dizilmiş bir kart ızgarasıdır; kaydırma yakalama (snap) ve bir görünüm genişliği kadar kaydıran önceki ve sonraki düğmeleri vardır (kullanıcı azaltılmış hareket istemediyse yumuşakça). Kullanıcı sonuna yaklaştıkça sonraki grubu, 12'şer 12'şer yükler. Kartlar sekme sırasında yer alan bağlantılardır ve odak aldıklarında görünüme kayarlar.

Kaydırma çubuğu gizlidir; dokunmatik ekran, dokunmatik yüzey ve Shift+tekerlek yine kaydırır, `.scrollbar(true)` ince bir çubuk bırakır. Fareyle basılı tutup şeridi yana sürükleyebilirsiniz. 5 pikselden fazla hareketten sonra bırakıştaki tıklama iptal edilir, böylece işaretçinin altındaki kart açılmaz ve şerit en yakın karta oturur; daha küçük bir hareket normal bir tıklamadır. Sürüklerken metin seçilmez, görseller dışarı sürüklenmez ve kart önizlemesi açılmaz. Dokunmatikte tarayıcının kendi kaydırması kalır. Önceki ve sonraki düğmeleri her zaman vardır ve uçlarda devre dışı kalır, çünkü gizli bir kaydırma çubuğu klavye için bir kaydırma yolu değildir. JavaScript olmadan düğmeler gizlenir ve şerit tarayıcının kendi kaydırmasıyla kayar.

```rust
Rail::of(catalog_products)
    .title(t!("shop.new_arrivals"))
    .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
    .sort_option(t!("shop.sort.newest"), Product::id, true)
```

## Arama kutusu

Öneriler sunan bir arama alanıdır; W3C APG combobox desenini bir liste kutusu açılır penceresiyle izler. Kullanıcı yazdıkça `instant` milisaniye bekler ve sunucudan öneri ister: action, metinle birlikte `DataQuery::q` olarak çalışır ve her satırın öneri metni geri gelir (`suggestion` başka türlü söylemedikçe maskelenmemiş ilk metin alanı). Enter ya da bir öneriyi seçmek, metinle birlikte sonuç screen'ini sayfa yenilenmeden açar.

```rust
#[screen(route = "/shop/search", public)]
pub fn shop_search(p: SearchParams) -> Screen {
    Screen::new(t!("shop.search.all"))
        .body(SearchBox::of(catalog_products).value(p.q.clone()).results(shop_search))
        .body(CardGrid::of(catalog_products).card(product_card).search(p.q).paging(Paging::pages(12)))
}

SearchBox::of(catalog_products)
    .label(t!("shop.search.label"))
    .instant(150)
    .suggestion(|p: &Product| p.name.clone())
    .results(shop_search)
```

Klavye: Aşağı ve Yukarı önerilerde gezinir (Aşağı onları açar), Enter vurgulanan öneriyle ya da yazılan metinle arar, Escape önce önerileri kapatır, sonra alanı temizler. Bir canlı bölge ekran okuyuculara kaç öneri olduğunu söyler.

Sonuç screen'i `SearchParams` alır (`q`, screen bir menüden açıldığında boştur). Sunucu metni `MAX_SEARCH_QUERY` (100) karaktere keser, kaç öneri göndereceğine karar verir (`limit` başka türlü söylemedikçe 8, en çok 20), screen'in erişimini, Feature'ın guard'larını ve action'ın iznini denetler ve her öneri çağrısını yazma hız sınırına sayar (`App::write_limit`).

`.drives(topic)` bir sonuç screen'i açmak yerine yerinde arar: Enter, bir öneriyi seçmek ya da alanı temizlemek metni screen'in `q` param'ına koyar ve topic'i dinleyen node'ları yeniler ([Topic'ler](effects.md#topics) bölümüne bakın). Screen'in params'ında, `SearchParams`'taki gibi bir `q: String` alanı gerekir.

### Zengin öneriler

`.item(|row| ListItem::new(title).subtitle(text).image(src).link(screen, params))` her öneriyi küçük bir görsel (40 x 56 piksel, tembel yüklenir, `img-src` kurallarını izler; satırı başlık adlandırdığı için alternatif metni boştur, görseli olmayan satırda yer tutucu yoktur), başlık ve alt başlıkla bir satır olarak gösterir. Sunucu, kaydın kanonik adresiyle `[{text, subtitle, image, href}]` döndürür. Bir satırı tıklamak ya da Enter ile seçmek bağlantısını `nav.js` üzerinden açar; her satır gerçek bir bağlantı olduğu için orta tık ve "yeni sekmede aç" da çalışır. Vurgulanan satır yokken Enter yine sonuç screen'ini açar. Combobox deseni aynı kalır: her satır, başlığı ve alt başlığıyla adlandırılmış bir seçenektir ve satırlar en az 44 piksel yüksekliktedir. `.item` olmadan öneriler yukarıdaki gibi düz metindir.

```rust
SearchBox::of(catalog_products)
    .item(|p: &Product| {
        ListItem::new(p.name.clone())
            .subtitle(p.code.clone())
            .image(p.image.clone())
            .link(shop_product, ProductPage { id: p.id })
    })
    .results(shop_search)
```

## Serbest metin

`DataQuery::q` serbest metindir: metin alanları onu içeren satırlar. Hem metin hem alanlar `zeyn::text::fold`'un verdiği biçimde karşılaştırılır: küçük harf, Türkçe noktalı ve noktasız `i` (`İ`, `ı`) `i` olarak, aksanlı Latin harfleri aksansız ve `ß`, `æ`, `œ`, `þ` açık yazılmış olarak. Böylece "istanbul", "İSTANBUL" kaydını, "nino" "Niño" kaydını ve "strasse" "Straße" kaydını bulur. `#[mask]` alanları asla aranmaz, boş metin her satırla eşleşir ve 100 karakterden uzun metin yok sayılır. `DataQuery::apply` ve SQL veri katmanları (sütun üzerinde `TRANSLATE`, `LOWER` ve `REPLACE`, bağlı parametreyle `LIKE`) bunu aynı şekilde okur.

```rust
use zeyn::text::fold;

assert_eq!(fold("İSTANBUL Işık"), "istanbul isik");
assert_eq!(fold("Año Pequeño"), "ano pequeno");
assert_eq!(fold("Straße MÜLLER"), "strasse muller");
```

Uygulamanın kendi yazdığı bir arama anahtarı sütunu için de, örneğin [elle yazılmış bir sorgunun](database.md) trigram sütunu için, `fold` kullanın; böylece anahtar kullanıcıların yazdığıyla eşleşir. Noktalama ve diğer yazı sistemleri olduğu gibi kalır; adresler için biçim `Slug::from_title`'dır.
