# Effect ve pencereler

Effect'ler, bir tıklamadan veya başarılı bir action'dan sonra tarayıcının yaptığı şeylerdir: bir pencere açmak, bir ekrana gitmek, toast göstermek, onay istemek ya da bir pencereyi kapatıp altındaki ekranı yenilemek. Rust'ta bildirilir ve runtime tarafından çalıştırılır; bu yüzden JavaScript yazılmaz.

## Effect'ler

| Effect | Nasıl oluşturulur | Sonuç |
|---|---|---|
| Pencere açmak | `Window::open(screen, params)` | Ekranı modal bir pencerede açar |
| Gezinmek | `Effect::navigate(screen, params)` | Bir bağlantı gibi, ekrana yerinde gider |
| Toast | `Effect::Toast { message }` | Kısa bir mesaj gösterir |
| Tonlu toast | `Toast::success(message).action(label, effect).into()` | Bir tonda (info, success, warning, danger) toast; isteğe bağlı olarak başka bir effect'i çalıştıran bir düğmeyle |
| Onay | `Effect::confirm(message, then)` | Kullanıcı onayladıktan sonra `then` çalıştırır |
| Kapat ve yenile | `Effect::CloseAndRefresh` | Pencereyi kapatır ve onu açan ekranı yeniden yükler |
| Topic'leri yenile | `Effect::refresh("orders")` ya da `ui.refresh(["orders", "stock"])` | Sayfada bir topic'i dinleyen node'ları yerinde yeniler ([Topic'ler](#topics) bölümüne bakın) |

Parametreler türlüdür: yanlış parametrelerle bir ekran için yazılan effect derlenmez.

Rotasında yol bölümleri olan bir ekrana `Effect::navigate` yapıldığında `/orders/42` gibi yol biçimine inilir (bkz. [Yolda parametreler](screens.md)); yolda olmayan parametreler `params` sorgusuna gider.

## Bir action'dan sonra

Formlar ve action düğmeleri bir `on_success` işleyicisi alır. İşleyici, action'ın sonucunu ve effect'leri toplayan bir `UiCtx` alır:

```rust
Form::of(product)
    .field(Product::name)
    .submit(t!("stock.save"), save_product)
    .on_success(|saved, ui| {
        if let Ok(p) = saved {
            ui.toast(t!("stock.saved", code = p.code));
        }
        ui.close_and_refresh();
    })
```

`UiCtx` içinde `toast`, `notify`, `close_and_refresh`, `navigate` ve bir onay gibi diğer her şey için `effect` bulunur:

```rust
ActionButton::new(t!("ui.check_stock"), check_stock, ()).on_success(|_, ui| {
    ui.effect(Effect::confirm(t!("ui.low_stock_confirm"), Effect::navigate(product_list, ())));
})
```

## Pencereler

Pencere, rotası olmayan ve `Screen::window(title)` ile oluşturulan bir ekrandır. Pencereler üst üste biner: bir pencere başka birini açabilir. Escape veya kapat düğmesi en üstteki pencereyi kapatır ve odak, onu açan öğeye geri döner. `.width(px)` genişliğini belirler.

```rust
#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::name).submit(t!("stock.save"), save_product)))
}
```

## Effect'ler nereden gelir

- Grid bağlantıları ve satır açmalar: `.link(|p| Window::open(...))`, `.on_open(|p| ...)`.
- Araç çubuğu düğmeleri: `.opens(screen, params)`.
- Bağlam menüleri: `.item(label, effect)`.
- Komutlar: `Command::open`, `Command::navigate`, `Command::effect`.
- Formların ve action düğmelerinin `on_success` işleyicileri.

Effect'ler sayfayı asla yeniden yüklemez. Bir gezinme yalnızca yeni ekranı yükler; bir yenileme yalnızca pencerenin altındaki ekranı yeniden yükler.

## Topic'ler

Bir screen'in bir parçası değiştiğinde, başka parçalar çoğu zaman aynı veriyi gösterir: kaydedilen bir sipariş toplamı, bir filtre grid'i ve grafiği değiştirir. Bir topic bunları, veri tarayıcıdan geçmeden birbirine bağlar. Veriyi gösteren node'ları `.listens(topic)` ile işaretleyin; topic adlandırıldığında yerinde yenilenirler.

```rust
Screen::new(t!("sales.title"))
    .body(Kpi::new(t!("sales.total"), total).listens("sales"))
    .body(Chart::bar(t!("sales.by_month")).labels(months).series(t!("sales.amount"), amounts).listens("sales"))
    .body(DataGrid::of(search_sales).column(Sale::amount).listens("sales"))
```

Bir topic üç yolla adlandırılır:

- Bir action veriyi değiştirdikten sonra, bir `on_success` işleyicisinde `ui.refresh("sales")` (ya da `Effect::refresh`).
- `.drives("sales")` diyen bir filtre kontrolü: filtre çipleri, bir arama kutusu ya da submit düğmesi olmayan bir form (seçim listeleri, tarih aralıkları, metin). [Filtre formları](forms.md#filter-forms) bölümüne bakın.
- Başka biri veriyi değiştirdiğinde realtime kanalından `hub.notify(&ctx, "sales")` (`realtime` özelliği). `docs/realtime.md` belgesine bakın.

`.listens` her bileşende çalışır: bileşenin kendi builder metotlarından sonra, en son çağırın. Grid tutan bir dinleyici grid'ini grid endpoint'i üzerinden yeniden yükler; sayfası, sıralaması ve filtreleri korunur (filtre ile tetiklenen bir değişiklik ilk sayfadan başlar). Diğer her dinleyici sunucuda yeniden render edilir; hepsi için tek bir `POST /_zeyn/nodes` yapılır, yani bir KPI ve bir grafik tek bir istek tutar. Yalnızca topic adları taşınır; sunucu screen'i her istekte olduğu gibi kullanıcının erişimi ve params değerleriyle açar, bu yüzden bir yenileme kullanıcının göremeyeceği bir şeyi asla göstermez.

Bir animasyon karesi içinde adlandırılan topic'ler tek seferde işlenir, aynı screen'de yeni bir yenileme hâlâ süren eskisini iptal eder ve yalnızca sayfada olan node'lar yenilenir. Yeni içerikte aynı öğe varsa (id, test id ya da bir listenin satır anahtarıyla) odak ve açık `<details>` oldukları yerde kalır.

Topic adları 1 ile 64 arası harf, rakam, `-`, `_` ya da `.` karakterinden oluşur. `App::check_page`, bir sayfada sürülen ama o sayfada kimsenin dinlemediği bir topic'i bildirir; `zeyn check` ise yenilenen ya da sürülen ama hiç dinlenmeyen, ya da dinlenen ama hiç yenilenmeyen, sürülmeyen veya bildirilmeyen bir topic için uyarır.

@live topics

## Toast'lar

`ui.toast(text)` bir info toast gösterir. `ui.notify(Toast::success(text))` tonu seçer: `Toast::info`, `success`, `warning` veya `danger`. `.action(label, effect)`, kaydedilen kaydı açmak gibi başka bir effect'i çalıştıran bir düğme ekler:

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

Toast'lar dört saniye kalır (danger olanlar sekiz), ve işaretçi bir toast'ın üzerindeyken ya da toast odaktayken süre durur; böylece düğmelerine ulaşılabilir. Info, success ve warning toast'ları bir durum bölgesi tarafından nazikçe okunur; danger toast'ları uyarıdır (alert) ve hemen duyurulur. Her toast'ta bir kapat düğmesi vardır, ton ayrıca renk çubuğu ve metniyle de verilir ve aynı anda en fazla üç toast gösterilir (`MAX_TOASTS`): yeni bir toast en eskisini kaldırır.

@live toasts
