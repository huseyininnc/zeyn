# Metinler ve t!

Bir kullanıcının gördüğü her metin, `t!` aracılığıyla locale dosyalarından gelir. Derleyici her locale dosyasında anahtarın bulunduğunu ve argümanların yer tutucularla eşleştiğini denetler; bu yüzden eksik bir çeviri boş bir etiket değil, bir derleme hatasıdır.

## Locale dosyaları

`locales/<lang>.json` iç içe anahtarlar tutar. Tüm dosyalar aynı anahtarlara sahip olmalıdır.

```json
{
  "stock": {
    "products": "Products",
    "saved": "Product {code} saved."
  }
}
```

## t!

```rust
Screen::new(t!("stock.products"))
ui.toast(t!("stock.saved", code = p.code));
```

`t!` bir `Text` döndürür: argümanlarıyla bir anahtar; sayfa render edilirken kullanıcının dilinde çözülür. Yazım hataları derleme zamanında başarısız olur:

```sh
error: t!: key `stock.prodcts` is missing in locales/tr.json
  = help: add it to every file in locales/
```

Kelimeleri değiştiren bir sayı plural yer tutucusuna, bir değere göre seçilen kelime select yer tutucusuna yazılır. Plural durumu dilin kurallarından gelir (CLDR adlarıyla `one`, `few`, `many` ve diğerleri), `=0` gibi tam bir durum önce gelir, `#` sayının yerini tutar ve her yer tutucunun bir `other` durumu olmalıdır. Plural argümanı tam sayı olmalıdır: `n = "two"` veya ondalık bir sayı derlenmez.

```json
"orders": {
  "count": "{n, plural, =0 {No orders} one {# order} other {# orders}}",
  "by": "{role, select, admin {An administrator} other {A colleague}} changed it."
}
```

```rust
t!("orders.count", n = orders.len())
```

## Alan ve seçim etiketleri

Alan etiketleri `<Model>.<field>` anahtarını kullanır, örneğin `Product.code`. Seçim etiketleri `<Choice>.<Variant>` kullanır. Bir anahtar eksikse etiket `#[label("...")]` değerine ya da adın okunabilir bir biçimine döner. `zeyn check`, bazı dillerde olup diğerlerinde bulunmayan anahtarları bildirir.

```json
{
  "Product": { "code": "Code", "name": "Product name" },
  "Unit": { "Piece": "Piece", "Kg": "Kilogram" }
}
```

## Dilleri kaydetmek

```rust
let i18n = I18n::new("en")
    .catalog("en", include_str!("../locales/en.json"))
    .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))?;
App::new(t!("app.name")).i18n(i18n)
```

`I18n::new` işlevinin ilk argümanı varsayılan dildir. Birden fazla katalog olduğunda shell bir dil seçici gösterir. Framework'ün kendi metinleri (grid, formlar, shell) İngilizce ve Türkçe olarak gelir.

## Diğer dillerde framework metinleri

Framework'ün kendi metinleri Çince, Rusça, İspanyolca, Korece, Japonca, Hintçe ve Arapça olarak da vardır. Her dil bir Cargo feature'ıdır; uygulama yalnızca açtığı diller için öder (her biri ikili dosyaya 35 ile 60 KB arası metin ekler):

```toml
zeyn = { version = "0.0", features = ["lang-ru", "lang-ar"] }
```

`lang-all` bütün dilleri açar. Uygulama bir dili yine de yalnızca o dil için kendi kataloğunu yukarıdaki gibi kaydettiğinde sunar. Feature açık değilse framework metinleri varsayılan dilde gösterilir.

## Ekranların dışındaki metinler

- `ctx.t(&text)`, bir `Text` değerini kullanıcının dilinde bir `String` yapar, örneğin bir e-posta için.
- `Text::english()` günlükler ve testler için İngilizce metni verir.
- `Text::from("literal")` asla çevrilmeyen bir metindir, örneğin bir ürün kodu.

Dilin nasıl seçildiği ve sayıların, paranın ve tarihlerin nasıl biçimlendirildiği için bkz. [Zaman ve diller](time-and-languages.md).
