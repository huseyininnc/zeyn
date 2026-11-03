# Public sayfalar

`public` işaretli screen'ler herkes tarafından açılabilir; hiç oturum açmamış ziyaretçiler de dahildir: bir açılış sayfası, bir fiyat listesi, bir yardım merkezi gibi. Zeyn bunları, teslim etmesi ucuz ve önbelleklerin ile arama motorlarının işlemesi kolay olacak şekilde sunar.

## Ziyaretçiler için çerez yok

Oturumsuz bir istekle açılan public screen çerez koymaz. Sayfa boş bir CSRF belirteci taşır; böylece aynı dildeki ve temadaki her ziyaretçi aynı baytları alır.

Tarayıcı runtime'ı, belirteç gerektiren ilk çağrıdan hemen önce (bir form, bir action, dil seçici) `GET /_zeyn/csrf` ile belirteç ister. Bu çağrı `zeyn_csrf` çerezini koyar ve asla önbelleğe alınmaz. Her `POST`, diğer her sayfadakiyle tamamen aynı şekilde denetlenir: eşleşen belirteç yoksa `403` ile reddedilir ve hiçbir şey sessizce yeniden denenmez.

Oturum açmış kullanıcılar, eskisi gibi, çerezi ve belirteci her zaman sayfada alır.

## HTTP önbellekleme

```rust
#[screen(route = "/pricing", public, cache = 300)]
pub fn pricing() -> Screen {
    Screen::new("Pricing")
}
```

`cache = N`, tarayıcıların ve paylaşılan önbelleklerin sayfayı `N` saniye (1 ile 86400 arası) tutmasına izin verir. Oturumsuz bir istek için sayfa şunları alır:

| Başlık | Değer |
|---|---|
| `Cache-Control` | `public, max-age=300` |
| `ETag` | Çizilen gövde üzerinde güçlü bir etiket |
| `Vary` | `x-zeyn-partial, Accept-Language, Cookie` |

Eşleşen bir `If-None-Match` taşıyan istek, gövdesiz `304 Not Modified` alır. Uygulama içi gezinmeyle yüklenen screen'ler aynı şekilde, kendi etiketleriyle önbelleğe alınır.

Oturumlu bir istek asla public önbellek başlıkları almaz: sayfa `Cache-Control: private, no-store` ile gönderilir. `public` olmayan bir screen üzerindeki `cache` derlenmez.

Screen, sunucuya ulaşan her istek için yine de oluşturulur ve çizilir; etiket aktarımdan tasarruf sağlar, uygulamanın önündeki bir önbellek ise isteği ortadan kaldırır. Önbelleğe alınan screen'leri ziyaretçiye bağlı hiçbir şeyden uzak tutun: günün saati, bir sayaç, rastgele bir seçim.

## Arama motorları ve bağlantı önizlemeleri

```rust
#[screen(route = "/pricing", public, cache = 300)]
pub fn pricing() -> Screen {
    Screen::new(t!("pricing.title"))
        .description(t!("pricing.description"))
        .canonical("/pricing")
        .og_image("https://cdn.example.com/pricing.png")
        .robots(Robots::Index)
        .json_ld(serde_json::json!({
            "@context": "https://schema.org",
            "@type": "Product",
            "name": "Team plan",
        }))
}
```

| Metot | Head etiketleri |
|---|---|
| `.description(text)` | `<meta name="description">` ve `og:description`, kullanıcının dilinde |
| `.canonical(url)` | `<link rel="canonical">` ve `og:url` |
| `.og_image(url)` | `og:image` |
| `.robots(Robots::Index)` veya `Robots::NoIndex` | `<meta name="robots">` |
| `.json_ld(value)` | `<script type="application/ld+json">` |

Her değer kaçışlanır. JSON-LD, `<`, `>` ve `&` karakterleri `<`, `>` ve `&` olarak yazılarak üretilir; böylece `</script>` içeren bir değer öğeyi bitiremez, tarayıcı onu asla çalıştırmaz ve content security policy diğer her sayfadaki kadar katı kalır. JSON-LD'nin adres özelliklerindeki (`url`, `@id`, `image`, `logo`, `item` ve benzerleri) uygulama yolları, arama motorlarının beklediği gibi her derinlikte `App::site_url` ile mutlak yazılır.

URL'ler `https` URL'leri ya da `/` ile başlayan uygulama yolları olmalıdır. Bunun dışındaki her şey (`http:`, `//host`, `javascript:`, boşluklar, tırnaklar) günlükte bir uyarıyla dışarıda bırakılır. Uygulama yolları, site URL'sinin ve taban yolun arkasına yazılır:

```rust
App::new(t!("app.name")).site_url("https://shop.example.com")
```

`site_url` olmadan head etiketleri uygulama yolunu olduğu gibi tutar; tarayıcılar bunu çözer ama bazı tarayıcı botları çözmez.

Uygulama içi gezinme, başlığı ve bu etiketlerin hepsini sayfa yüklemeden günceller.

## Site haritası

```rust
#[action(public)]
pub async fn site_pages(part: SitemapPart, db: Db) -> Result<Vec<SitemapUrl>> {
    let rows = db.products_after(part.offset, part.limit).await?;
    Ok(rows.into_iter().map(|p| SitemapUrl::new(format!("/products/{}", p.slug)).lastmod(p.changed)).collect())
}

Feature::new("site").sitemap(site_pages)
```

`/sitemap.xml`, action'dan her seferinde 1.000 URL ister (`part.offset`, `part.limit`) ve XML'i akıtır; böylece büyük bir site hiçbir zaman bellekte durmaz. 50.000 URL'ye kadar tek bir `urlset` olur. Bunun üstünde, her biri 50.000 URL içeren `/sitemaps/1.xml`, `/sitemaps/2.xml` ve benzerlerinden oluşan bir site haritası dizini olur. Action `public` olmalı ve URL'leri kararlı bir sırayla döndürmelidir. Site haritası mutlak URL'ler tuttuğu için uygulama yolları `App::site_url` gerektirir; `https` URL'si ya da uygulama yolu olmayan girdiler dışarıda bırakılır. Yalnızca public sayfaları listeleyin: arama motorları diğerlerini açamaz. Uygulama başına bir Feature'ın site haritası olur.

## robots.txt ve llms.txt

```rust
App::new(t!("app.name"))
    .robots_txt("User-agent: *\nAllow: /\nSitemap: https://shop.example.com/sitemap.xml\n")
    .llms_txt(include_str!("../llms.txt"))
```

İkisi de herkese düz metin olarak sunulur.
