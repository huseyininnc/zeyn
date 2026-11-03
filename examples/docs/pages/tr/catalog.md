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
- **Sayfalama.** `Paging::infinite(n)` (varsayılan, 24) ekranda göründüğünde de tetiklenen bir "Show more" düğmesi gösterir; düğmeye basıldığında odak ilk yeni karta geçer. `Paging::pages(n)` önceki ve sonraki düğmelerini ve sayfa numarasını gösterir. Sayfa boyutunu sunucu belirler; istemci yalnızca nereden devam edileceğini söyler.
- **Sıralama.** Her `sort_option`, kullanıcıların bir seçim listesinden seçebileceği bir sıradır; ilki kartların başladığı sıradır. Sunucu yalnızca bu sıraları kabul eder ve istemcinin gönderdiği süzgeçleri ve arama metnini atar.
- **Arama.** `.search(text)` yalnızca serbest metinle eşleşen satırları gösterir ([`DataQuery::q`](#free-text)); örneğin bir sonuç screen'inin metni.
- **Maskeli alanlar.** Kartlar, ham satırı gören bir closure ile oluşturulur. `#[mask]` alanları olan bir model için kartları yalnızca bu alanları açma yetkisi olan kullanıcılar alır; herkes `Forbidden` alır. Public kartlar için maskeli alanı olmayan bir model kullanın.

## Rail

Rail, yana kayan tek satıra dizilmiş bir kart ızgarasıdır; kaydırma yakalama (snap) ve bir görünüm genişliği kadar kaydıran önceki ve sonraki düğmeleri vardır (kullanıcı azaltılmış hareket istemediyse yumuşakça). Kullanıcı sonuna yaklaştıkça sonraki grubu, 12'şer 12'şer yükler. Kartlar sekme sırasında yer alan bağlantılardır ve odak aldıklarında görünüme kayarlar.

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

## Serbest metin

`DataQuery::q` serbest metindir: metin alanları onu içeren satırlar, büyük küçük harf yok sayılarak. Türkçe noktalı ve noktasız `i` (`İ`, `ı`), `i` ile eşleşir; böylece "istanbul", "İSTANBUL" kaydını bulur. `#[mask]` alanları asla aranmaz, boş metin her satırla eşleşir ve 100 karakterden uzun metin yok sayılır. `DataQuery::apply` ve SQL veri katmanları (bağlı parametrelerle `LOWER(TRANSLATE(column, 'İı', 'ii')) LIKE ...`) bunu aynı şekilde okur.
