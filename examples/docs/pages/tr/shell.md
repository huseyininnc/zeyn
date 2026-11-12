# Shell ve komutlar

Shell, her screen'in etrafındaki çerçevedir: menü, dil, tema ve saat dilimi seçicileri, toast'lar ve komut paleti. Masaüstü araçları gibi çalışan uygulamalar bir menü çubuğu, bir araç kutusu, bir özellik denetçisi, bir durum çubuğu ve yerleştirilebilir paneller ekler.

## Menü

Menü, feature'ların `.menu(...)` çağrılarından gelir. Kullanıcının açamayacağı öğeler HTML'den çıkarılır ve guard'ları kullanıcıyı reddeden bir feature hiçbir şey göstermez. Geçerli screen'in bağlantısı `aria-current="page"` taşır ve markanın soluk tonu üzerinde çizilir.

`.item(...)` sonrasında `.icon(Icon::...)` öğenin etiketinin önüne bir simge koyar; bir grubun öğelerinin simgeleri olduğunda grubun etiketi kendi simgesi olmadan çizilir. Yan menü uygulamanın logosu ve adıyla başlar: `App::logo("/static/logo.svg")` (ve koyu tema için `App::logo_dark(...)`) uygulamanın sunduğu bir işareti ayarlar; aksi halde shell, `--z-brand` renginde, adın ilk harfini taşıyan yuvarlatılmış bir kare çizer. Shell nötr kalır; marka rengi geçerli öğeyi, birincil düğmeleri, odak halkasını ve logoyu işaretler.

Simgeler, 20 px'lik bir ızgarada 1,5 px yuvarlak çizgiyle çizilmiş 82 çizimlik tek bir setten gelir (`Icon::ALL`) ve geçerli metin renginde 16 ya da 20 px boyutunda gösterilir. Her biri `/_zeyn/icons.css` içinde bir CSS maskesidir; bu, `crates/zeyn/assets/icons/*.svg` dosyalarından üretilen, yaklaşık 25 KB (sıkıştırılmış 3 KB) boyutunda, önbelleğe alınan bir stil sayfasıdır. Böylece bir simge HTML'de tek bir sınıfa, betikte ise hiçbir şeye mal olmaz.

```rust
Feature::new("sales")
    .screen(orders)
    .menu(|m| m.group(t!("sales.menu"), Icon::Cart).item(t!("sales.orders"), orders).icon(Icon::Table))
```

### İç içe ve katlanabilir gruplar

`.sub(label, |m| ...)` son gruba iç içe bir grup ekler; içinde `item`, `icon`, `open` ve `sub` bir grupta olduğu gibi çalışır. İç içe gruplar kendi gruplarının öğelerinden sonra gelir ve en üst gruptan en fazla üç seviye aşağı iner (`MENU_DEPTH`); daha derin bir menü, uygulamayı başlangıçta grubun adını veren bir mesajla durdurur.

```rust
.menu(|m| {
    m.group(t!("sales.menu"), Icon::Cart)
        .item(t!("sales.orders"), orders)
        .sub(t!("sales.reports"), |m| m.item(t!("sales.by_city"), by_city).item(t!("sales.archive"), archive))
        .accordion()
})
```

- **Katlanabilir.** Her grup başlığı `aria-expanded` ve `aria-controls` taşıyan bir düğmedir (W3C APG açılır gezinme kalıbı). Sunucu, açık ekranı tutan grupları açık, diğerlerini kapalı çizer; `nav.js` her gezinmeden sonra yeni ekranın dalını açar. Betik yoksa bütün gruplar açık kalır, böylece hiçbir şeye ulaşılamaz hale gelmez.
- **Hep açık.** Bir gruptan sonra `.open()` onu açık tutar: başlığı düğmesiz düz metindir.
- **Akordeon.** App shell'deki herhangi bir feature'ın menüsünde `.accordion()` bütün yan menüyü akordeona çevirir: bir grubu açmak açık kardeşlerini kapatır.
- **Klavye.** Yukarı ve Aşağı görünen başlıklar ve bağlantılar arasında gezer, Home ve End ilkine ve sonuncusuna gider. Sağ kapalı bir grubu açar ya da açık bir grubun içine geçer; Sol açık bir grubu kapatır ya da çevreleyen grubun başlığına gider. Enter ve Space grubu açıp kapatır. Sağdan sola dillerde Sol ve Sağ yer değiştirir.
- **İzinler ve birleştirme.** Aynı etiketli gruplar her seviyede feature'lar arasında birleşir; kullanıcının açamayacağı öğelerden oluşan bir grup, iç içe gruplar dahil, dışarıda kalır. `App::header(|h| h.breadcrumb())` içerik yolu grubun bütün yolunu gösterir. Site shell'i iç içe grupların bağlantılarını en üst gruplarında listeler.

### İkon şeridi, rozetler, filtre ve telefonlar

```rust
.menu(|m| {
    m.group(t!("mail.menu"), Icon::Mail)
        .item(t!("mail.inbox"), inbox)
        .count(unread)
        .count_listens("mail")
        .item(t!("mail.rules"), rules)
        .badge(t!("menu.new"))
})
```

- **İkon şeridi.** 1024 px ve üstü ekranlarda menü çubuğundaki bir düğme menüyü üst düzey grupların ikonlarına (ya da ilk harflerine) katlar. Grubun adı ipucu olarak görünür ve grup şeridin yanında açılan bir panel olarak açılır; Escape, dışarıya tıklama ve gezinme onu kapatır. Seçim `zeyn_rail` çerezinde tutulur, böylece sonraki sayfa sıçrama olmadan aynı biçimde çizilir.
- **Rozetler ve sayılar.** Bir öğeden sonra `.badge(text)` "Yeni" gibi kısa, sabit bir metin ekler. `.count(action)`, `()` alıp `u64` döndüren bir action'ın saydığı bir sayı ekler: sayfa göründükten sonra menüsündeki tüm sayıları tek istekte ister (`GET /_zeyn/menu/counts`), böylece yavaş bir sayım sayfayı geciktirmez. Action'ın yetkisi olmayan kullanıcılar sayı görmez, 0 hiçbir şey göstermez ve 99'dan büyük sayılar "99+" olarak görünür. `.count_listens(topics)`, bir action `ui.refresh(..)` ile konulardan birini andığında ya da canlı socket haber verdiğinde yeniden sayar.
- **Filtre ve favoriler.** 10 veya daha fazla bağlantısı olan bir yan menü (`MENU_FILTER_FROM`) bir filtre kutusu alır: yazdıkça eşleşmeyen bağlantılar gizlenir (büyük küçük harf ve aksanlar yok sayılır), eşleşenlerin grupları açılır ve kaç bağlantı kaldığı söylenir; Escape kutuyu temizler. Her bağlantıya bir sabitleme düğmesi gelir ve sabitlenen bağlantılar üstteki Favoriler grubunda görünür; tarayıcıda kullanıcı başına tutulur.
- **Telefonlar.** 1024 px altında menü, çubuğundaki bir düğmenin arkasına katlanır ve ekranın üzerinde bir çekmece olarak açılır. Odak çekmeceye geçer ve çekmece ile düğme arasında kalır, arkadaki ekran etkisizdir ve kaymaz; düğme, Escape, arka plan, bir bağlantı ya da menünün kenarına doğru kaydırma onu kapatır ve odağı düğmeye geri verir.

### İstek başına kurulan menüler

`App::menu_source(source)` her istek için kurulan menü girdileri ekler; örneğin kullanıcının rollerine göre süzülen bir menü tablosundan. Bir kaynak, `Ctx`'i (kimlik, tenant, dil) alan ve `MenuNode` döndüren async bir fonksiyondur: en üstte `MenuNode::group(label, children)`, içinde `MenuNode::screen(label, "screen_name")` bağlantıları (`.params(json)` yol parçalarını ve sorguyu `screen_url` gibi doldurur), `MenuNode::path(label, "/reports/sales")` bağlantıları ve iç içe gruplar. `.icon(...)`, `.open()` ve `.permission("menu.reports")` her düğümde çalışır. Etiket, diğer metinler gibi kaçışlanan düz metin ya da `Text::key` ile bir katalog anahtarıdır.

Gruplar feature'ların statik menülerinden sonra gelir; statik bir grupla aynı etiketi taşıyan grup onunla birleşir. İçerik yolu, komut paleti ve `nav.js` bu bağlantıları diğer menü bağlantıları gibi ele alır.

Aşağıdaki tablo PostgreSQL ve Oracle'da çalışır. Tek bir düz `SELECT` onu okur, ağaç Rust'ta `parent_id`'ye göre kurulur:

```sql
CREATE TABLE menu_items (
    id          INTEGER PRIMARY KEY,
    parent_id   INTEGER REFERENCES menu_items (id),
    label       VARCHAR(100) NOT NULL,
    screen      VARCHAR(100),
    path        VARCHAR(500),
    icon        VARCHAR(40),
    permission  VARCHAR(100),
    sort_order  INTEGER DEFAULT 0 NOT NULL
);
```

```rust
#[model]
pub struct MenuRow {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub label: String,
    pub screen: Option<String>,
    pub path: Option<String>,
    pub icon: Option<String>,
    pub permission: Option<String>,
    pub sort_order: i64,
}

async fn menu(ctx: Ctx) -> Result<Vec<MenuNode>> {
    let db = ctx.service::<Db>().ok_or_else(|| Error::Internal("no database".into()))?;
    let rows: Vec<MenuRow> = db
        .query("SELECT id, parent_id, label, screen, path, icon, permission, sort_order FROM menu_items ORDER BY sort_order, id", vec![])
        .await?;
    Ok(tree(&rows, None))
}

fn tree(rows: &[MenuRow], parent: Option<i64>) -> Vec<MenuNode> {
    rows.iter()
        .filter(|r| r.parent_id == parent)
        .map(|r| {
            let mut node = match (&r.screen, &r.path) {
                (Some(screen), _) => MenuNode::screen(r.label.clone(), screen.clone()),
                (None, Some(path)) => MenuNode::path(r.label.clone(), path.clone()),
                (None, None) => MenuNode::group(r.label.clone(), tree(rows, Some(r.id))),
            };
            if let Some(icon) = Icon::ALL.iter().find(|i| Some(i.name()) == r.icon.as_deref()) {
                node = node.icon(*icon);
            }
            match &r.permission {
                Some(p) => node.permission(p.clone()),
                None => node,
            }
        })
        .collect()
}

App::new("Admin").menu_source(menu)
```

- **Asla güvenilmez.** Sunucu her istekte her bağlantıyı denetler: bilinmeyen screen'lere giden bağlantılar log'a bir uyarıyla düşürülür; kullanıcının açamayacağı screen'lere giden bağlantılar (erişim, feature guard'ları, düğümün kendi `permission`'ı) kaynak döndürmüş olsa da düşürülür; yol parçaları params ile dolmayan screen bağlantıları da öyle. Bir yol tek bir `/` ile başlamalıdır: şema, `//`, ters eğik çizgi ve kontrol karakteri olmaz. Gruplar en fazla `MENU_DEPTH` seviye iç içe geçer, bir kaynağın cevabı en fazla `MAX_MENU_NODES` (500) düğüm tutar ve etiketler `MAX_MENU_LABEL` (100) karakterde kesilir.
- **Önbellekli.** Cevaplar tenant, dil ve izin kümesi başına `MENU_TTL` (60 saniye) boyunca önbellekte tutulur; aynı rollere sahip kullanıcılar tek bir cevabı paylaşır ve kaynak onlar için dakikada bir çalışır. Önbellek en fazla `MAX_MENU_CACHE` (256) cevap tutar ve en eskisini atar. Cevabı yalnızca izinlere değil kullanıcıya da bağlı olan bir kaynak `App::menu_per_user()` ister. Bir action tabloyu düzenledikten sonra inject edilen `Menus` servisinde `invalidate()` çağırır.
- **Sayfayı asla düşürmez.** Kaynak hata döndürdüğünde, panic olduğunda ya da `MENU_TIMEOUT`'tan (2 saniye) uzun sürdüğünde sayfa statik menüyü gösterir ve sunucu hatayı log'a yazar. Hatalar önbelleğe alınmaz.
- **Nerede görünür.** Menü app shell'e aittir: screen'lerin ve hata sayfalarının tam sayfa yüklemeleri onu taşır; site shell ve giriş sayfaları yalnızca statik menüleri gösterir.

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
    .shell(Shell::site().header(|h| {
        h.center(SearchBox::of(search_products).results(shop_search))
            .center_on_phone(CenterOnPhone::Icon)
            .end(LinkButton::new(t!("shop.cart"), shop_cart, ()))
            .sign_in()
            .language()
            .theme()
    })
    .footer(|f| {
        f.column(t!("shop.menu.help"), |c| {
            c.link(t!("shop.delivery.title"), shop_delivery).url(t!("shop.footer.source"), "https://example.com")
        })
        .text(t!("shop.footer.note"))
    })
    .bottom_nav([
        BottomNavItem::new(t!("shop.menu.home"), shop_home).icon(Icon::Home),
        BottomNavItem::new(t!("shop.menu.help"), shop_delivery).icon(Icon::Box),
    ]))
```

- **Menü.** Üst menü, site shell'indeki her feature'ın menüsünü listeler. Tek öğeli bir grup düz bir bağlantıdır; daha büyük bir grup bağlantılarını açan bir düğmedir (W3C APG disclosure navigation deseni). Enter, Boşluk ya da bir tıklama grubu açıp kapatır, Aşağı ok açar ve ilk bağlantıya geçer, Yukarı ok, Aşağı ok, Home ve End bağlantıları arasında gezdirir, Escape kapatır ve düğmeye döner, gruptan Tab ile çıkmak ya da dışına tıklamak kapatır. Sol ve Sağ ok üst öğeler arasında gezdirir.
- **Dar ekranlar.** 1024 px altında menü, `aria-expanded` taşıyan bir menü düğmesinin arkasına katlanır; Escape kapatır. Betik olmadan menü açık kalır, böylece hiçbir şey erişilemez olmaz.
- **Üst bilgi slotları.** Üst çubuğun, mantıksal adlara sahip bir uygulama çubuğu gibi her bileşeni alan dört slotu vardır: markadan önce `start(node)`, menü ile uç arasında `center(node)`, çubuğun sonunda `end(node)` ve altında tam genişlikte bir satır olarak `bottom(node)`, örneğin chip'ler. Her slot birden çok düğümü sırayla tutar; start ve end, sağdan sola yazılan dillerde yer değiştirir. Slotlardaki bileşenler kullanıcının diliyle, izinleriyle ve maskeleriyle render edilir; verisini kendi action'ı ile yükleyenler, örneğin bir `SearchBox`, orada çalışır ve Feature'ın guard'larına ve action'ın iznine karşı denetlenir. Grid'ler ve formlar screen'e aittir. Telefonlarda start ve end slotları çubukta kalır, center slotu uygulamanın söylediği yere gider: `center_on_phone(CenterOnPhone::Row)` (varsayılan) onu çubuğun altında bir satıra koyar, `CenterOnPhone::Icon` onu açan bir büyüteç düğmesinin arkasına koyar (W3C APG disclosure) ve odağı içine taşır; Escape kapatır ve düğmeye döner. `center_button(icon, label)` bu düğmeyi değiştirir. Telefon yerleşimi betik çalışmadan önce CSS ile belirlenir, bu yüzden sayfa yüklenirken üst bilgi oynamaz.
- **Eylemler.** `sign_in()`, uygulamanın bir oturum açma sayfası olduğunda ziyaretçilere bir oturum açma bağlantısı, oturum açmış kullanıcılara bir oturum kapatma bağlantısı gösterir; `language()` ve `theme()` uygulama shell'indekiyle aynı seçicileri gösterir. Bunlar end slotundan sonra gelir ve telefonlarda menüye katlanır.
- **Alt çubuk.** `bottom_nav([...])`, telefonlarda ve tabletlerde (1024 px'ten dar) ekranın altına sabitlenmiş en fazla beş bağlantı ekler; insanların en çok gittiği yerler içindir, beşten fazlası derlenmez. Her öğe, bir etiket, rotası olan bir ekran ve isteğe bağlı bir simge içeren bir `BottomNavItem`'dır. Gösterilen sayfanın bağlantısında `aria-current="page"` bulunur (`nav.js` güncel tutar), kullanıcının açamayacağı ekranlara giden bağlantılar çıkarılır, çubuk `env(safe-area-inset-bottom)` ile ana ekran göstergesinden uzak durur ve sayfanın altında çubuğun hiçbir şeyi gizlememesi için boşluk bırakılır. Üst menü geri kalan yerler için kalır. Çubuk betik gerektirmez.
- **Alt bilgi.** Screen'lere bağlantılardan (`link`, açma izni olmayan kullanıcılar için çıkarılır) ya da adreslerden (`url`: bir yol, `https:`, `http:` ya da `mailto:`; `App::check` diğer şemaları ve kayıtlı olmayan screen'lere bağlantıları reddeder) oluşan sütunlar ve bir metin satırı.

Bir site shell screen'i ile bir uygulama shell screen'i arasında geçiş, sayfa yüklemeden `nav.js` üzerinden olur: kısmi yanıt hangi shell'e ait olduğunu söyler (`x-zeyn-shell`) ve bu geçerli çerçeveden farklı olduğunda runtime, bir dil değişikliğinin yaptığı gibi tüm çerçeveyi değiştirir ve diğer shell'in stil sayfasını ve betiğini yükler. `/_zeyn/route/{screen}` bağlantıları (breadcrumb'ınki) aynı şekilde izlenir.

## Oturum açma shell'i

Oturum açma sayfaları kendine ait bir çerçeve alır: `Shell::auth()` içinde menü, üst çubuk ve komut paleti yoktur. Uygulamanın logosu (`App::logo`) ve adı 400 px genişliğinde tek bir kartın üstünde durur; dil ve tema seçicileri sayfanın altında yer alır, böylece ikisi de oturum açmadan önce erişilebilir kalır. `.split()` 1024 px'ten itibaren formu bir yarıya, `--z-brand` renginde bir marka panelini diğer yarıya koyar; daha dar ekranlar ortalanmış kartı gösterir. Telefonlarda kart sayfanın kendisi olur ve girdileri ile düğmeleri 44 px yüksekliğindedir. Stilleri (`auth.css`, 3.5 KB) yalnızca bu sayfalarda yüklenir.

`zeyn::accounts::ui` içindeki her oturum açma ekranı (oturum açma, kayıt olma, parolayı unutma ve sıfırlama, iki adımlı oturum açma, kurtarma kodu, kilitlenme, onay ve oturum kapatma) bu shell'de açılır; hesap ve yönetim ekranları ise uygulama shell'ini korur. Özel bir oturum açma ekranı onu kendi feature'ından alır:

```rust
Feature::new("entry").screen(sign_in).shell(Shell::auth())
```

Oturum açma formu kullanıcı adını `Control::Username` (`autocomplete="username"`) ile, parolayı `current-password` ile doldurur, parolayı gösterme düğmesini alanın içinde bir göz olarak gösterir ve Caps Lock açıkken alanın altında "Caps Lock açık" der (kibar bir canlı bölge).

## Ekran içinde tema seçici

`ThemePicker::new()` tema seçiciyi bir ekranın içine koyar, örneğin bir ayarlar sayfasının Görünüm bölümüne. `App::themes` temalarını sunar (uygulama cihazı izliyorsa Sistem seçeneğiyle) ve kullanılan temayı gösterir.

```rust
#[screen(route = "/settings/appearance", permission = "settings.read")]
pub fn appearance() -> Screen {
    Screen::new(t!("settings.appearance")).body(Panel::new(t!("settings.theme")).add(ThemePicker::new()))
}
```

Kabuktaki seçiciyle aynı denetimdir: seçim sayfa yenilenmeden hemen uygulanır, `zeyn_theme` çerezinde (oturum açmış kullanıcılar için görünüm deposunda da) tutulur ve sayfadaki diğer seçici de onu gösterir. Uygulama seçim sunmuyorsa (`theme_picker(false)` ya da tek tema) hiçbir şey çizmez.

## Uygulama başlığı

`App::header(|h| ...)` uygulama kabuğundaki ekranların üstüne, site başlığının yuvalarıyla bir çubuk koyar: `start`, `center`, `end` ve `bottom`. Ortaya bir arama kutusu, sona bir bildirim bağlantısı gelir. `sign_in()`, `language()` ve `theme()` burada da çalışır, ama yan menüde seçiciler zaten vardır.

```rust
App::new(t!("app.name"))
    .header(|h| {
        h.center(SearchBox::of(search_all).results(search_results))
            .end(LinkButton::new(t!("app.notifications"), notifications, ()))
    })
```

Üç hazır parça çubuğu bir uygulama çerçevesine dönüştürür:

- `breadcrumb()` çubuğu menü grubu ve geçerli screen'in başlığıyla başlatır; `nav.js` her gezinmeden sonra onu yeniden yazar.
- `search()` komut paletini açan ve kısayolunu gösteren bir arama alanı ekler; palet düğmesi yan menüden kalkar. Telefonlarda bir simge düğmesidir.
- `account()` çubuğu kullanıcının baş harflerini taşıyan yuvarlak bir düğmeyle bitirir. Düğme bir diyalog açar (W3C APG non modal dialog; Escape ve kapatma düğmesi kapatır); diyalogda kimin oturum açtığı, tema ve dil seçicileri ve oturum kapatma bağlantısı, ziyaretçiler için ise bir oturum açma bağlantısı bulunur. Seçiciler yan menüden kalkar.

```rust
App::new(t!("app.name")).header(|h| h.breadcrumb().search().account())
```

Ekran kaydırılırken çubuk üstte kalır ve ekranlar sayfa yenilenmeden altında değişir. Telefonda menü düğmesinin altında durur ve orta yuva kendi satırına geçer. Çubuktaki bileşenler bir ekrandakiler gibi kullanıcı için çizilir; bir arama kutusu önerilerini kendi eylemiyle yükler ve sunucu o eylemin iznini denetler. `App::header` olmadan uygulama kabuğunda üst çubuk olmaz.

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

**Alt+Shift+P** paleti varsayılan olarak açar (`App::palette_shortcut` bunu değiştirir, bilerek `Ctrl+Shift+P` gibi bir tarayıcı tuşuna da): uygulamanın komutlarını, geçerli screen'in araç çubuğunu ve bağlam menüsünü ve menünün screen'lerini arayan bir ARIA combobox'ı. Son kullanılan komutlar önce gelir. Odak bir alanda değilken `/` de onu açar, `App::header(|h| h.search())` arama alanı da. Sonuçlar yerlerinin altında gruplanır ("Git · Satış", screen başlığı, "Komutlar"); her birinde simgesi, eşleşen harfler kalın ve sonda kısayolu bulunur. Menü öğeleri sayfanın kendisinde eşleştirilir, böylece ilk tuş vuruşu onları zaten bulur.

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
- `PropertyInspector::island(name)` bir adanın seçtiğini form olmadan düzenler ([Island inspector](#island-inspector) bölümüne bakın).
- `StatusBar` bir live region'a sahiptir, bu yüzden ona gönderilen mesajlar ekran okuyucular tarafından okunur.

## Island inspector

```rust
Row::new()
    .col(Island::new("designer"))
    .col(PropertyInspector::island("designer"))

let schema = FormSchema::new()
    .section("Position")
    .field(FieldSpec::length("x", "X", &[LengthUnit::Mm]).min(0))
    .section("Look")
    .field(FieldSpec::color("fill", "Fill", BRAND).required())
    .field(FieldSpec::choice("align", "Align", [("left", "Left"), ("right", "Right")]));
```

Ada, `detail: { title, schema, values }` ile `zeyn:inspect` gönderir; `schema`, serileştirilmiş bir `FormSchema`'dır. Alan türleri `text`, `multiline`, `integer`, `decimal`, `length`, `color`, `boolean`, `choice` ve `expression`'dır; her biri `required`, `min`, `max`, `max_len`, `default` ve `help` alır.

Denetçi alanları model formlarının kontrolleriyle çizer; adları, etiketleri ve değerleri HTML olarak değil, metin olarak doldurur. Bir alandan çıkıldığında, Enter'a basıldığında ya da bir seçici kapandığında değeri denetler ve ada köküne `detail: { name, value }` ile `zeyn:property` gönderir. Geçersiz bir değer hatasını gösterir ve gönderilmez. 200'den fazla alanı olan bir şema yok sayılır. `FieldSpec::check` aynı denetimleri sunucuda yapar.

## Yerleştirilebilir paneller

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

Kullanıcılar sekmeleri alanlar arasında sürükler, alanları imleçle ya da klavyeyle yeniden boyutlandırır ve panelleri kapatır; bir menüdeki `MenuList::panels` onları geri getirir. Yerleşim, kullanıcı ve screen başına uygulamanın görünüm deposunda, uygulamada yoksa tarayıcıda saklanır.
