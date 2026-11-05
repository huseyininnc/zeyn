# Screen'ler ve parametreler

Screen, bir rotası olan bir sayfa ya da başka bir screen'in üzerinde açılan bir penceredir (Window). `#[screen]` erişimini bir action gibi bildirir.

## Bir sayfa

```rust
#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products"))
        .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).shortcut("Alt+N").opens(product_editor, ProductParams { id: None }))
        .body(DataGrid::of(search_products).column(Product::code).column(Product::name).flex(1))
}
```

- `Screen::new(title)` bir sayfadır; `Screen::window(title)` bir penceredir.
- Birkaç `.body(...)` çağrısı birkaç bileşeni alt alta yerleştirir.
- Bir screen `Screen` ya da `Result<Screen>` döndürür.

## Parametreler

Bir screen'in ilk argümanı parametreleridir: bir `#[params]` struct'ı, ya da hiç yoksa `_: ()`. Diğer argümanlar action'lardaki gibi enjekte edilir.

```rust
#[params]
pub struct ProductParams {
    pub id: Option<ProductId>,
}

#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::code).submit(t!("stock.save"), save_product)))
}
```

Parametreler kullanıldıkları her yerde tiplidir: `Window::open(product_editor, 7)` derlenmez, çünkü editör `ProductParams` alır.

`route` olmayan bir screen'in URL'si yoktur. Bir bağlantıdan, araç çubuğu düğmesinden, menüden ya da komuttan pencere olarak açılır.

## Yolda parametreler

Bir rota, params alanlarını yol segmentleri olarak adlandırabilir. Her `{name}` aynı adlı alana bağlanır ve o alanın türü olarak okunur:

```rust
#[params]
pub struct OrderParams {
    pub id: OrderId,
    #[serde(default)]
    pub tab: Option<String>,
}

#[screen(route = "/orders/{id}", permission = "sales.order.read")]
pub fn order(p: OrderParams, store: OrderStore) -> Result<Screen> {
    let order = store.get(p.id).ok_or(Error::NotFound)?;
    Ok(Screen::new(t!("sales.order")).body(DescriptionList::new().item(t!("sales.customer"), order.customer)))
}
```

- `/orders/42` 42 numaralı siparişi açar. Yolda olmayan alanlar eskisi gibi JSON sorgusunda taşınır: `/orders/42?params={"tab":"lines"}`.
- Segmentler tam sayıları, entity anahtarlarını (`OrderId`), `String` ve `Slug` değerlerini alır. Bir değer yalnızca framework'ün yazdığı biçimde ayrıştırılır (`7`, `007` ya da `+7` değil); başka her şey 404 yanıtlar.
- Params'ta hiçbir alana karşılık gelmeyen bir segment, params'sız bir screen üzerinde segmentli bir rota ya da segment olamayacak bir alan türü derlenmez ve hata neyin değişmesi gerektiğini söyler.
- Birkaç segment aynı şekilde çalışır: `slug: Slug` ve `n: u32` ile `#[screen(route = "/catalog/{slug}/{n}", public)]`.
- Framework'ün yazdığı bağlantılar yol biçimini kullanır: `Effect::navigate`, breadcrumb'lar, komutlar ve oturum açmadan sonraki `return_to`. Segmentsiz screen'ler `?params={json}` biçimini korur.
- Segmentli bir screen menü öğesi ya da ana screen olamaz, çünkü hiçbir şey onları dolduramaz.
- Erişim, her screen'de olduğu gibi her istekte denetlenir.

### Slug'lar

`Slug::from_title("Çay Bardağı")` değeri `cay-bardagi` olur: küçük harfli ASCII, sözcükler `-` ile birleşir, Türkçe harfler eşlenir (`ı` ve `İ` için `i`, `ş` için `s`, `ğ` için `g`, `ç` için `c`, `ö` için `o`, `ü` için `u`), diğer Latin harflerin aksanları atılır ve başka her şey dışarıda bırakılır. Bir slug en fazla 80 bayttır. Bir yolda yalnızca kanonik biçim ayrıştırılır, bu yüzden `/catalog/Cay-Bardagi/1` 404 yanıtlar.

### Bulunamadı ve taşındı

Kayıt yoksa `Err(Error::NotFound)` döndürün; kullanıcı 404 sayfasını görür. Adres değiştiğinde, örneğin bir ürünün adı değiştirildikten sonra, yenisiyle yanıtlayın. Tarayıcı kalıcı bir yönlendirme (301) alır ve uygulama içi gezinme onu sayfa yüklemeden izler:

```rust
#[screen(route = "/catalog/{slug}/{n}", public)]
pub fn catalog_page(p: CatalogParams, store: Catalog) -> Result<Screen> {
    let item = store.find(p.n).ok_or(Error::NotFound)?;
    if item.slug != p.slug {
        return Ok(Screen::moved_to(catalog_page, CatalogParams { slug: item.slug, n: p.n }));
    }
    Ok(Screen::new(item.name))
}
```

`Screen::redirect(screen, params)`, adresin kendisi geçerli kalırken bir isteği başka bir screen'e gönderir, örneğin rastgele bir seçime ya da en yeni öğeye. Tarayıcı, hedefin kanonik yolunu taşıyan `303 See Other` ve `Cache-Control: no-store` alır; uygulama içi gezinme onu sayfa yüklemeden izler ve adres çubuğu hedefi gösterir. Hedef her zaman route'u olan tipli bir screen'dir, bu yüzden bir yönlendirme uygulamanın dışındaki bir adrese asla götüremez ve uygulamanın sunmadığı bir hedef 404 yanıtlar.

```rust
#[screen(route = "/random", public)]
pub async fn random_item(store: Catalog) -> Result<Screen> {
    let item = store.random().await?.ok_or(Error::NotFound)?;
    Ok(Screen::redirect(catalog_page, CatalogParams { slug: item.slug, n: item.n }))
}
```

@live redirect

## Erişim

| Seçenek | Anlamı |
|---|---|
| `permission = "..."` | Kimliğin bu permission'a sahip olması gerekir |
| `public` | Herkes. Oturumu olmayan ziyaretçiler sayfayı çerezsiz alır ([Herkese açık sayfalar](public-pages.md) bölümüne bakın) |
| `cache = 300` | Yalnızca herkese açık screen'ler: tarayıcılar ve paylaşımlı önbellekler, oturumu olmayan bir ziyaretçinin sayfasını bu kadar saniye saklayabilir (en fazla 86400) |
| `sensitive` | Açılması, kişisel verinin okunması olarak kaydedilir |
| `reason` | Kullanıcı açılmadan önce bir gerekçe vermelidir ([Denetim, maskeleme ve işler](enterprise.md) bölümüne bakın) |

Bir sayfa, arama motorlarına ve bağlantı önizlemelerine kendisini `Screen` üzerindeki `.description(..)`, `.canonical(..)`, `.og_image(..)`, `.json_ld(..)` ve `.robots(..)` ile de tanıtır; [Herkese açık sayfalar](public-pages.md#search-engines-and-link-previews) bölümüne bakın.

Kullanıcının açamayacağı bir screen'i açan menü öğeleri, araç çubuğu düğmeleri ve komutlar HTML'den çıkarılır.

Oturum açmamış ziyaretçiler için bir screen `public` bildirir. Action'ları kendi erişimlerine ihtiyaç duyar: veri listeleyen herkese açık bir screen `public` bir action çağırır, yoksa liste ziyaretçiler için boş kalır.

```rust
#[action(public)]
pub async fn latest_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/catalog", public)]
pub fn catalog() -> Screen {
    Screen::new(t!("catalog.title")).body(DataGrid::of(latest_products).column(Product::code).column(Product::price))
}
```

`permission` ve `public` seçeneklerinin ikisini de atlamak bir derleme hatasıdır, bu yüzden bir screen hiçbir zaman kazara açık kalmaz.

## Araç çubuğu ve bağlam menüsü

```rust
Screen::new(t!("stock.products"))
    .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).opens(product_editor, ProductParams { id: None }).export(Export::Csv))
    .context_menu(ContextMenu::new().item(t!("stock.new_product"), Window::open(product_editor, ProductParams { id: None })).action(t!("stock.restock_low"), restock_low, ()))
```

`Alt+N` gibi kısayollar screen üzerinde çalışır ve palette'te gösterilir. Tarayıcının kullandığı kısayollar (`Ctrl+P`, `Ctrl+S`, `F5`), düğme `.override_browser()` çağırmadıkça bağlanmaz ve tarayıcının hiç iletmediği kısayollar (`Ctrl+N`, `Ctrl+T`, `Ctrl+W`) hiçbir zaman bağlanmaz; serbest kombinasyonlar `Alt+Shift+<harf>` ve `Ctrl+Alt+<harf>` biçimindedir.

## Feature'lar ve menü

Bir screen bir feature'a aittir. Feature'ın rota öneki rotadan önce gelir, guard'ları screen açılmadan önce çalışır ve menüsü screen'leri listeler:

```rust
Feature::new("stock")
    .route_prefix("/stock")
    .guard(SignedIn)
    .screen(product_list)
    .screen(product_editor)
    .menu(|m| m.group(t!("menu.stock"), Icon::Box).item(t!("stock.products"), product_list))
```

`App::home(screen)`, `/` adresinin bir screen açmasını sağlar.

## Hata sayfaları

`App::not_found(screen)` ve `App::server_error(screen)`, uygulamaya 404 ve sunucu hataları için kendi sayfalarını verir:

```rust
#[screen(public)]
pub fn not_found_page() -> Screen {
    Screen::new(t!("errors.not_found")).body(EmptyState::new(t!("errors.not_found_text")))
}

App::new(t!("app.name")).not_found(not_found_page).server_error(server_error_page)
```

- 404 screen'i bilinmeyen yolları ve `Err(Error::NotFound)` döndüren her screen'i yanıtlar; ayrıştırılamayan yol değerleri de buna dahildir.
- Sunucu hatası screen'i `Error::Internal`, zaman aşımlarını (504) ve handler'lardaki panic'leri yanıtlar. Hatanın ayrıntıları yalnızca sunucu günlüğüne gider; screen params almaz ve istek hakkında hiçbir şey öğrenmez.
- İkisi de doğru durum koduyla shell içinde gösterilir. Uygulama içi gezinme yalnızca screen'i alır ve sayfa yüklemeden yerine koyar. Framework endpoint'leri ve API JSON yanıtlamaya devam eder.
- Hata screen'leri params almaz, rota gerektirmez ve bir feature'ın parçası değildir. Onları `public` yapın: onu açma izni olmayan bir kullanıcı ya da kendisi başarısız olan bir hata screen'i, yerleşik kısa mesajı alır.

## Bir screen tarayıcıya nasıl ulaşır

İlk istek tam sayfayı alır: menü, screen ve varlıklar. Uygulama içindeki bağlantılar sonra yalnızca screen'i yükler ve yerine koyar, böylece menü, açık durum ve kaydırma konumu kalır. Geri düğmesi, yeni sekmeler ve yer imleri normal bağlantılardaki gibi çalışır.
