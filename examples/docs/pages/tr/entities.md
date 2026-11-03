# Entity ve model

Veri bir kez, öznitelikli (attribute) bir Rust struct'ı olarak tanımlanır. Aynı tanım formları, grid sütunlarını, sunucuda ve tarayıcıda doğrulamayı, JSON API'yi ve OpenAPI şemasını yönlendirir.

## Entity

`#[entity(table = "...")]` bir tablo satırını işaretler. Bir `#[key]` alanı gerektirir; `ProductId` gibi türlenmiş bir id ve her alan için `Product::code` gibi bir alan referansı üretir.

```rust
#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20, unique)]
    pub code: String,
    #[rule(required, max_len = 200)]
    pub name: String,
    pub unit: Unit,
    #[rule(min = 0)]
    pub price: Money,
    #[readonly]
    pub stock: i64,
    pub active: bool,
    #[version]
    pub version: i64,
}
```

Alan referansları türlüdür: `Product::price` bir `Field<Product, Money>` değeridir. Metin sütununda bir grid toplamı ya da başka bir modelden bir form alanı derlenmez.

## Model

`#[model]`, tablo satırı olmayan veridir: filtreler, sihirbaz adımları, form girdisi.

```rust
#[model]
pub struct Visit {
    #[rule(required, min = 2026_01_01, max = 2027_12_31)]
    pub day: Option<Date>,
    pub arrives: Time,
}
```

## Choice

`#[choice]`, birim varyantlı bir enum'u seçim kutusuna, radyo grubuna veya segmentli denetime dönüştürür. Etiketler yerel ayar (locale) dosyalarından gelir.

```rust
#[choice]
pub enum Unit {
    Piece,
    Kg,
    Liter,
}
```

## Alan öznitelikleri

| Öznitelik | Anlamı |
|---|---|
| `#[key]` | Bir entity'nin birincil anahtarı |
| `#[version]` | `i64` satır sürümü. Eski sürümle yapılan kayıtlar `Error::Conflict` ile başarısız olur |
| `#[readonly]` | Gösterilir, formlar tarafından asla yazılmaz |
| `#[label("...")]` | Locale dosyalarında etiket yoksa yedek etiket |
| `#[rule(...)]` | Doğrulama kuralları, aşağıda |

## Kurallar

| Kural | Uygulandığı tür | Anlamı |
|---|---|---|
| `required` | herhangi | Değer içermelidir |
| `min_len = n`, `max_len = n` | metin | Karakter olarak uzunluk |
| `email` | metin | Bir e-posta adresi |
| `embed` | metin | `Embed` ile gömülecek bir adres: uygulamanın `frame-src` değerindeki (`App::csp`) bir sunucuda `https`. Formlar yapıştırılan `<iframe>` kodunu da kabul eder ve izleme parametreleri olmadan yalnızca kanonik adresini saklar. Bkz. [Görüntüleme](display.md#embed) |
| `min = n`, `max = n` | sayılar, para, ondalıklar, tarihler | Sınırlar. Tarihler `2026_01_01` biçiminde yazılır |
| `scale = n`, `precision = n` | `Decimal` | En fazla ondalık basamak (kural yoksa 2) ve toplamda en fazla basamak (kural yoksa 18). Sütun türünü de belirler: `NUMERIC(precision, scale)` |
| `mask = "..."` | metin | Ham değer bir kalıbı doldurur: `9` rakam, `a` harf, `A` büyük harf, `*` ikisi de; diğer karakterler yazarken gösterilir ama saklanmaz. Bkz. [Denetimler](controls.md#masked-input) |
| `unique` | herhangi | Store tarafından denetlenir; form, action'dan gelen hatayı gösterir |

Alan türüne uymayan bir kural, ne yapılacağını söyleyen bir mesajla derleme hatasıdır. Alanlar arası kurallar bir fonksiyona konur:

```rust
#[model(validate = check_stay)]
pub struct Stay {
    pub from: Option<Date>,
    pub to: Option<Date>,
}

fn check_stay(s: &Stay, errors: &mut ValidationErrors) {
    if let (Some(a), Some(b)) = (s.from, s.to) {
        if b < a {
            errors.add(Stay::to, t!("stay.before_start"));
        }
    }
}
```

## Valid

Kaydeden bir action `Valid<M>` alır. Bir `Valid` değeri yalnızca her kural çalıştırılarak üretilebilir; böylece bir action doğrulamayı unutamaz:

```rust
#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

## Alan türleri

`String`, `i32`, `i64`, `bool`, `Money`, `Decimal`, `Date`, `Time`, `DateTime`, `LocalDateTime`, `#[choice]` enum'ları, entity id'leri ve bunların herhangi birinin `Option<T>` hali. Bir modelin `Vec<M>` hali formlarda satır editörü olur (bkz. [Satır editörü](lines.md)).

Para, para birimiyle birlikte tam sayı küçük birimlerle saklanır. Para için asla `f64` kullanmayın. `Decimal`, kesirli diğer her şey için kesin bir sayıdır: miktarlar, ikiden fazla ondalıklı birim fiyatlar, oranlar. 38 basamağa kadar tutar, asla bir float'tan geçmez ve JSON'da bir dize olarak taşınır (`"12.50"`). Tarih ve saatler [Zaman ve diller](time-and-languages.md) sayfasında anlatılır.
