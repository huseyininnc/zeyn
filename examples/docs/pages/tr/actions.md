# Action'lar

Action, bir ekranın, gridin, formun, komutun, JSON API'nin veya bir ajanın çağırabileceği sunucu kodudur. `#[action]` onu kimin çağırabileceğini bildirir; framework bunu kod çalışmadan önce denetler.

## Bir action bildirmek

```rust
#[action(permission = "stock.product.read")]
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}
```

- İlk argüman girdidir. İstemciden gelir; bu yüzden framework onu güvenilmeyen JSON olarak okur.
- Diğer her argüman enjekte edilir: `Ctx` veya kayıtlı bir `Service`.
- Dönüş türü `Result<T>` değeridir.

## Erişim

Her action şunlardan tam olarak birini bildirir:

| Seçenek | Anlamı |
|---|---|
| `permission = "feature.resource.verb"` | Kimliğin bu izne sahip olması gerekir |
| `public` | Oturum açmamış kullanıcılar dahil herkes |

Erişimi olmayan bir action derlenmez. İzin adları küçük harflidir ve noktalıdır, örneğin `sales.order.write`.

Diğer seçenekler:

| Seçenek | Anlamı |
|---|---|
| `audit` | Her çağrı için bir denetim kaydı yazar |
| `reauth` | Kullanıcının yakın zamanda oturum açmış olması gerekir (`App::reauth_within`) |

## Hatalar

| Hata | Durum | Kullanım |
|---|---|---|
| `Error::NotFound` | 404 | Kayıt yok ya da kullanıcının varlığını bilmemesi gerekir |
| `Error::Forbidden` | 403 | Erişim reddedildi |
| `Error::Denied(t!("..."))` | 403 | Kullanıcının okuyabileceği bir gerekçeyle reddedildi |
| `Error::Conflict` | 409 | Satır sürümü, form yüklendiğinden beri değişti |
| `Error::Validation(errors)` | 422 | Alanların yanında gösterilen alan hataları |
| `Error::Internal("detail")` | 500 | Ayrıntı yalnızca sunucu günlüğüne gider |

Koddan bir alan hatası:

```rust
let mut errors = ValidationErrors::default();
errors.add(Product::code, t!("stock.code_taken", code = product.code));
return Err(errors.into());
```

## Listeler

Listeler bir `DataQuery` alır ve `Page<M>` döndürür. Bir liste action'ı asla her satırı döndürmez; grid sayfa ister ve istemci ne gönderirse göndersin sunucu sayfa boyutunu sınırlar.

## Bağlam

`Ctx` istektir: `ctx.identity()`, `ctx.locale()`, `ctx.time_zone()`, `ctx.now()`, `ctx.today()`, `ctx.tenant()`, `ctx.license()`, `ctx.config()`, `ctx.theme()` ve `ctx.t(&text)`. Testler zamanı `App::fixed_now` ile sabitleyebilsin diye sistem saati yerine `ctx.now()` ve `ctx.today()` kullanın.

## Action'lar nerede kullanılır

- `DataGrid::of(action)` satırları yükler; `.edit_with(action)`, `.row_action(...)` ve `.bulk_action(...)` onları değiştirir.
- `Form::submit(label, action)` bir formu kaydeder.
- `ActionButton::new(label, action, input)` bir düğmeden action çalıştırır.
- `Command::action(...)` komut paletinden veya bir kısayoldan çalıştırır.
- `Feature::api(action)` onu JSON olarak yayımlar (bkz. [API ve OpenAPI](api.md)).
- `Feature::search(action)` komut paletine sunucu araması ekler.

Her yol aynı denetimleri çalıştırır: tarayıcı çağrıları için CSRF, Feature'ın guard'ları, action'ın erişimi, ardından interceptor'lar.
