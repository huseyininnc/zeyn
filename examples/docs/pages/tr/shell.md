# Shell ve komutlar

Shell, her screen'in etrafındaki çerçevedir: menü, dil, tema ve saat dilimi seçicileri, toast'lar ve komut paleti. Masaüstü araçları gibi çalışan uygulamalar bir menü çubuğu, bir araç kutusu, bir özellik denetçisi, bir durum çubuğu ve yerleştirilebilir paneller ekler.

## Menü

Menü, feature'ların `.menu(...)` çağrılarından gelir. Kullanıcının açamayacağı öğeler HTML'den çıkarılır ve guard'ları kullanıcıyı reddeden bir feature hiçbir şey göstermez. Geçerli screen'in bağlantısı `aria-current="page"` taşır.

## Site shell'i

İş screen'leri, yan menülü uygulama shell'inde açılır. Mağaza ya da ürün kataloğu gibi herkese açık sayfalar bunun yerine site shell'inde açılabilir: marka bağlantısı olarak uygulamanın adı, yatay bir menü, dar ekranlarda bir menü düğmesi, isteğe bağlı eylemler ve bir alt bilgi. Bunu bir feature seçer; diğer feature'ların screen'leri uygulama shell'ini korur, böylece ikisi tek bir uygulamada yaşar.

```rust
Feature::new("shop")
    .screen(shop_home)
    .screen(shop_delivery)
    .screen(shop_returns)
    .menu(|m| {
        m.group(t!("shop.menu.home"), Icon::Home)
            .item(t!("shop.menu.home"), shop_home)
            .group(t!("shop.menu.help"), Icon::Box)
            .item(t!("shop.delivery.title"), shop_delivery)
            .item(t!("shop.returns.title"), shop_returns)
    })
    .shell(Shell::site().header(|h| h.sign_in().language().theme()).footer(|f| {
        f.column(t!("shop.menu.help"), |c| {
            c.link(t!("shop.delivery.title"), shop_delivery).url(t!("shop.footer.source"), "https://example.com")
        })
        .text(t!("shop.footer.note"))
    }))
```

- **Menü.** Üst menü, site shell'indeki her feature'ın menüsünü listeler. Tek öğeli bir grup düz bir bağlantıdır; daha büyük bir grup bağlantılarını açan bir düğmedir (W3C APG disclosure navigation deseni). Enter, Boşluk ya da bir tıklama grubu açıp kapatır, Aşağı ok açar ve ilk bağlantıya geçer, Yukarı ok, Aşağı ok, Home ve End bağlantıları arasında gezdirir, Escape kapatır ve düğmeye döner, gruptan Tab ile çıkmak ya da dışına tıklamak kapatır. Sol ve Sağ ok üst öğeler arasında gezdirir.
- **Dar ekranlar.** 800 px altında menü, `aria-expanded` taşıyan bir menü düğmesinin arkasına katlanır; Escape kapatır. Betik olmadan menü açık kalır, böylece hiçbir şey erişilemez olmaz.
- **Eylemler.** `sign_in()`, uygulamanın bir oturum açma sayfası olduğunda ziyaretçilere bir oturum açma bağlantısı, oturum açmış kullanıcılara bir oturum kapatma bağlantısı gösterir; `language()` ve `theme()` uygulama shell'indekiyle aynı seçicileri gösterir.
- **Alt bilgi.** Screen'lere bağlantılardan (`link`, açma izni olmayan kullanıcılar için çıkarılır) ya da adreslerden (`url`: bir yol, `https:`, `http:` ya da `mailto:`; `App::check` diğer şemaları ve kayıtlı olmayan screen'lere bağlantıları reddeder) oluşan sütunlar ve bir metin satırı.

Bir site shell screen'i ile bir uygulama shell screen'i arasında geçiş, sayfa yüklemeden `nav.js` üzerinden olur: kısmi yanıt hangi shell'e ait olduğunu söyler (`x-zeyn-shell`) ve bu geçerli çerçeveden farklı olduğunda runtime, bir dil değişikliğinin yaptığı gibi tüm çerçeveyi değiştirir ve diğer shell'in stil sayfasını ve betiğini yükler. `/_zeyn/route/{screen}` bağlantıları (breadcrumb'ınki) aynı şekilde izlenir.

## Komutlar

Bir komut, etiketi, kısayolu ve ne yaptığıyla Rust'ta bir kez bildirilir. Bir feature'a kaydedildiğinde palet onu listeler ve kısayolu her sayfada çalışır.

```rust
pub fn new_product() -> Command {
    Command::open("stock.new_product", t!("pro.cmd_new_product"), product_editor, ProductParams { id: None })
        .shortcut("Ctrl+Alt+N")
        .icon(Icon::Plus)
}

pub fn restock() -> Command {
    Command::action("stock.restock_low", t!("pro.cmd_restock"), restock_low, ()).shortcut("Ctrl+Alt+R")
}

Feature::new("pro").command(new_product()).command(restock())
```

| Kurucu | Ne çalıştırır |
|---|---|
| `Command::open(id, label, screen, params)` | Bir screen'i pencere olarak açar |
| `Command::navigate(id, label, screen, params)` | Bir screen'e gider |
| `Command::effect(id, label, effect)` | Tarayıcıda bir effect, örneğin bir toast |
| `Command::action(id, label, action, input)` | Sunucuda bir action; girdi hiçbir zaman tarayıcıya gitmez |

`App::check`, okunamayan, iki komutun paylaştığı ya da paletin kendine ayırdığı kısayolları bildirir.

## Komut paleti

**Alt+Shift+P** paleti varsayılan olarak açar (`App::palette_shortcut` bunu değiştirir, bilerek `Ctrl+Shift+P` gibi bir tarayıcı tuşuna da): uygulamanın komutlarını, geçerli screen'in araç çubuğunu ve bağlam menüsünü ve menünün screen'lerini arayan bir ARIA combobox'ı. Son kullanılan komutlar önce gelir.

Bir feature ona sunucu araması ekleyebilir. Kullanıcı yazdıkça palet action'ı çağırır ve sonuçlarını komutların altında listeler; birini seçmek sayfayı yerinde açar. Bu site kendi sayfalarını bu şekilde arar:

```rust
#[action(public)]
pub async fn search_docs(query: String, docs: Docs) -> Result<Vec<SearchHit>> {
    Ok(docs.search(&query))
}

Feature::new("docs").search(search_docs)
```

Sunucu, kullanıcının çalıştırabileceği her aramayı feature'ın guard'ları, action'ın permission'ı ve interceptor'lar ile çalıştırır, sorguyu 100 karakterde keser, uygulamanın dışına çıkan sonuçları atar ve en fazla 20 sonuç döndürür.

## Menü çubuğu, araç kutusu ve durum çubuğu

```rust
MenuBar::new(t!("pro.menubar"))
    .menu(t!("pro.menu_file"), |m| m.command(new_product()).separator().command(restock()))
    .menu(t!("pro.menu_view"), |m| m.panels())

Toolbox::new("Tools")
    .tool("select", "Select", Some(Icon::Search))
    .tool("draw", "Draw", Some(Icon::Edit))

StatusBar::new("Status").left("Ready").right("UTF-8")
```

@live shell

- `MenuBar` ARIA menubar desenini izler ve palet ile aynı komutları tutar.
- `Toolbox` dikey bir toggle düğmeleri grubudur; seçilen araç bir `zeyn:tool` olayı olarak gönderilir.
- `PropertyInspector::new(title, form)`, `.section(...)` gruplarına bölünmüş bir formla seçili nesneyi düzenler.
- `StatusBar` bir live region'a sahiptir, bu yüzden ona gönderilen mesajlar ekran okuyucular tarafından okunur.

## Yerleştirilebilir paneller

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

Kullanıcılar sekmeleri alanlar arasında sürükler, alanları imleçle ya da klavyeyle yeniden boyutlandırır ve panelleri kapatır; bir menüdeki `MenuList::panels` onları geri getirir. Yerleşim, kullanıcı ve screen başına uygulamanın görünüm deposunda, uygulamada yoksa tarayıcıda saklanır.
