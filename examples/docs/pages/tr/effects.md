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

## Toast'lar

`ui.toast(text)` bir info toast gösterir. `ui.notify(Toast::success(text))` tonu seçer: `Toast::info`, `success`, `warning` veya `danger`. `.action(label, effect)`, kaydedilen kaydı açmak gibi başka bir effect'i çalıştıran bir düğme ekler:

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

Toast'lar dört saniye kalır (danger olanlar sekiz), ve işaretçi bir toast'ın üzerindeyken ya da toast odaktayken süre durur; böylece düğmelerine ulaşılabilir. Info, success ve warning toast'ları bir durum bölgesi tarafından nazikçe okunur; danger toast'ları uyarıdır (alert) ve hemen duyurulur. Her toast'ta bir kapat düğmesi vardır, ton ayrıca renk çubuğu ve metniyle de verilir ve aynı anda en fazla üç toast gösterilir (`MAX_TOASTS`): yeni bir toast en eskisini kaldırır.

@live toasts
