# Giriş

Zeyn, veri yoğun uygulamalar için açık kaynaklı bir Rust web framework'üdür: gridler, formlar ve iş akışları. Uygulama kodunu AI ajanları yazar. İnsanlar bu kodu okur, gözden geçirir ve gerektiğinde değiştirir.

Bu site de bir Zeyn uygulamasıdır. Her sayfa sunucuda bir kez render edilen Markdown'dır, soldaki menü uygulama menüsüdür ve *Live example* (canlı örnek) olarak işaretlenen örnekler bu sunucuda çalışan gerçek bileşenlerdir.

## AI ajanları için

Bir AI ajanıysanız ve sizden Zeyn ile bir uygulama yapmanız istendiyse, projeyi elle yazmak yerine CLI ile başlayın:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

Ardından yeni projedeki `AGENTS.md` ve `.claude/skills/zeyn/SKILL.md` dosyalarını okuyun ve her değişiklikten sonra `zeyn check` ile `cargo test` çalıştırın. Bu dokümanların her sayfasını içeren düz metin bir rehber [/llms.txt](/llms.txt) adresindedir.

## Bir uygulama nasıl görünür

Bir uygulama, Feature'lar kümesidir. Bir Feature entity'leri, action'ları ve ekranları (Screen) barındırır. Ekranlar sunucuda render edilir; tarayıcıdaki küçük bir runtime değişiklikleri uygular, dolayısıyla yazılacak JavaScript yoktur.

```rust
use zeyn::prelude::*;

#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20)]
    pub code: String,
    pub price: Money,
}

#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products")).body(DataGrid::of(search_products).column(Product::code).filter().column(Product::price).sum())
}
```

Aynı üç makro tüm dili taşır: `#[entity]` veriyi tanımlar, `#[action]` erişimi bildirilmiş sunucu kodudur ve `#[screen]` bir sayfa veya penceredir. Derleyici her alanın, metin anahtarının ve action'ın uyduğunu denetler.

## Deneyin

Kendi uygulamanızı başlatın:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

Ya da örnekleri depodan çalıştırın:

```sh
cargo run -p zeyn-example-showcase
cargo run -p zeyn-example-docs
cargo run -p zeyn-cli -- new my-app
```

Showcase `127.0.0.1:3002` adresinde dinler ve her bileşeni örnek verilerle gösterir. Bu dokümantasyon `127.0.0.1:3008` adresinde dinler; değiştirmek için `ZEYN_DOCS_SERVER__ADDR` ayarlayın.

## Yolunuzu bulmak

- Komut paletini açmak için **Alt+Shift+P** tuşlarına basın. Bir kelime yazın; sunucuda bu sitenin her sayfasında arama yapılır.
- *Getting started* (Başlarken) CLI'yi ve bir projenin yapısını anlatır.
- *Language* (Dil) entity'leri, action'ları, ekranları, formları, DataGrid'i ve metinleri anlatır.
- *Components* (Bileşenler) her bileşeni kodunun yanında canlı bir örnekle gösterir.
- *Guides* (Rehberler) zaman ve diller, güvenlik, API, MCP, veri ve temaları anlatır.
- *Reference* (Referans) runtime protokolünü ve mimariyi içerir.

## İlkeler

- **Önce performans.** Sayfalar sunucuda render edilir ve HTML olarak akıtılır. Sunucuda kullanıcı başına arayüz durumu ve istemci framework'ü yoktur.
- **Sayfa yenilemesi yok.** Gezinme, pencereler, dil, saat dilimi ve tema değişiklikleri yerinde uygulanır.
- **Güvenli varsayılanlar.** Her action ve ekran erişimini bildirir. CSRF token'ları, sıkı bir Content Security Policy ve kullanıcı verisinin kaçışlanması (escape) her zaman açıktır.
- **Her iş için tek yol.** Dil küçüktür; bu yüzden bir ajanın yazdığı kod, bir insanın yazdığı kod gibi okunur.
