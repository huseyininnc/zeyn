# Güvenlik

Zeyn'de güvenlik, koruduğu kodun hemen yanında bildirilir ve her çağrıda sunucuda denetlenir. Varsayılanlar katıdır; birini gevşetmek `app.rs` içinde görünür bir satırdır.

## İzinler

Her action ve screen `permission = "feature.resource.verb"` ya da `public` bildirir. Biri yoksa kod derlenmez. Bir isteğin kimliği (identity) izinlerini taşır:

```rust
Identity::user("ada").with_permissions(["stock.product.read", "stock.product.write"])
```

Kullanıcının çalıştıramayacağı menü öğeleri, araç çubuğu düğmeleri, satır action'ları ve komutlar HTML'e konmaz. Bunları saklamak bir kolaylıktır; sunucu izni her çağrıda yeniden denetler.

## Oturum açma

Uygulama parola tutmaz. Bir `Authenticator`, çağıranın kim olduğunu bulur; bu genellikle uygulamanın önündeki bir tek oturum açma (single sign on) vekil sunucusundan gelir:

```rust
App::new(t!("app.name")).auth(
    ForwardAuth::new("x-forwarded-user")
        .groups("x-forwarded-groups")
        .require_header("x-proxy-secret", secret.expose())
        .role("stock-admins", ["stock.product.read", "stock.product.write"]),
)
```

`ForwardAuth` yalnızca her istek vekil sunucudan geçiyorsa kullanılmalıdır; uygulamaya doğrudan erişebilen herkes bu başlıkları gönderebilir. `require_header`, vekil sunucunun eklediği ortak bir sırrı denetler. `DevAuth::superuser(name)` yerel geliştirme için her isteğe tam erişim verir ve açıkken sunucu bir uyarı basar. Release derlemesi bununla başlamayı reddeder (`App::check` bunu bildirir); release modunda çalışması gereken bir demo `App::allow_dev_auth()` çağırabilir. `zeyn new` ile oluşturulan uygulamalar bunu yalnızca debug derlemelerinde açar.

## Guard'lar

Bir guard, bir Feature'ın herhangi bir screen'i açılmadan önce ve grid'lerinden, form'larından ya da komutlarından biri çağrılmadan önce çalışır:

```rust
fn warehouse_open(ctx: &Ctx) -> GuardResult {
    if ctx.identity().permissions.contains("stock.warehouse.read") { GuardResult::Allow } else { GuardResult::Deny(t!("stock.closed")) }
}

Feature::new("stock").guard(SignedIn).guard(warehouse_open)
```

Hazır guard'lar: `SignedIn`, `TenantRequired` ve `Licensed`.

## Interceptor'lar

Bir interceptor, erişim denetimlerinden sonra her screen, grid, form, API ve komut çağrısının çevresinde çalışır. `before` bir çağrıyı durdurabilir; `after` çağrının nasıl bittiğini görür. `AuditLog(sink)`, kimin ne yaptığını kaydeden hazır interceptor'dır.

## CSRF, CSP ve başlıklar

- Her sayfa rastgele bir `zeyn_csrf` çerezi (`HttpOnly`, `SameSite=Strict`) koyar ve aynı belirteci bir meta etiketine yazar. Runtime'ın yaptığı her çağrı belirteci `x-zeyn-csrf` başlığında gönderir ve sunucu ikisini sabit sürede karşılaştırır.
- Oturumsuz açılan public screen'ler istisnadır: çerez koymazlar ve belirteç taşımazlar, böylece önbelleğe alınabilirler. Runtime, ilk çağrısından önce belirteci `GET /_zeyn/csrf` ile alır ve her `POST` yine aynı şekilde denetlenir ([Public pages](public-pages.md) sayfasına bakın).
- Content Security Policy (CSP) betiklere, stillere ve görsellere yalnızca uygulamanın kendi kaynağından izin verir. Satır içi betik yoktur; temalar birer stil dosyasıdır, asla satır içi stil değildir.
- `App::csp` ilkeyi adı verilen sunucular için, her seferinde bir direktif olacak şekilde açar. Başlık başlangıçta bir kez oluşturulur.

```rust
App::new("Shop").csp(|c| {
    c.img_src(["https://images.example.com"])
        .frame_src(["https://www.youtube-nocookie.com", "https://*.vimeo.com"])
        .media_src(["https://media.example.com:8443"])
        .media_blob()
        .connect_src(["https://api.example.com"])
        .font_src(["https://fonts.example.com"])
})
```

  Yalnızca `https://host`, `https://host:port` ve `https://*.host` kabul edilir (joker karakter alt alan adlarıyla eşleşir, alan adının kendisiyle eşleşmez). `.media_blob()`, akış kütüphaneleri için `media-src` değerine `blob:` ekler. `App::check`, `*` değerini, `http:` değerini, `data:` ya da `https:` gibi yalın şemaları, `'unsafe-inline'` ya da `'unsafe-eval'` gibi tırnaklı anahtar sözcükleri, yolları, kullanıcı adlarını ve boşluk, virgül ya da noktalı virgül içeren değerleri reddeder; her sorun, onun yerine ne yazılacağını söyler. `script-src`, `object-src`, `base-uri` ve `frame-ancestors` değiştirilemez. `Embed` ve `Image` aynı listeleri okur, böylece tarayıcının engelleyeceği bir sunucu hiç çizilmez ([Display](display.md) sayfasına bakın).
- Yanıtlar `X-Content-Type-Options: nosniff`, `Referrer-Policy: same-origin` ve `X-Frame-Options: DENY` başlıklarını taşır.
- `App::secure_cookies(true)` çerezleri `Secure` olarak işaretler; HTTPS arkasında açın.

## Kaçışlama

HTML'deki kullanıcı verisi her zaman `render::escape` üzerinden geçer. Bileşenler çizdikleri her değeri kaçışlar, bu yüzden adı `<script>` olan bir ürün metin olarak görünür.

## Sınırlar

```rust
App::new(t!("app.name"))
    .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
    .write_limit(RateLimit::per_user(60, Duration::from_secs(60)))
    .trusted_proxies(["10.0.0.0/8"])
    .allow_only_ips(["10.0.0.0/8", "192.168.0.0/16"])
    .deny_ips(["203.0.113.7"])
```

- `rate_limit`, statik dosyalar ve public medya kaynaklarının okunması (`Feature::public_media`) dışındaki her isteğe uygulanır; `write_limit`, veriyi değiştiren çağrılar için daha sıkı bir sınır ekler: form gönderimleri, grid düzenlemeleri, satır, toplu ve menü action'ları, komutlar, içe aktarmalar, kayıtlı grid görünümleri, dock yerleşimleri ve temalar, API çağrıları, MCP çağrıları ve asistan. Reddedilen istekler `Retry-After` ile `429` alır.
- `X-Forwarded-For` yalnızca `trusted_proxies` içinden gelirse güvenilir sayılır. Başlığın her satırı sırasıyla hesaba katılır ve alınan adres, güvenilir vekil sunucu olmayan son adrestir.
- Açığa çıkarmalar (reveal), island ve veri kaynağı çağrıları da yazma sayılır; dışa aktarmalar ve yazdırılan belgeler de öyle (`GET /_zeyn/grid/export`, `GET /_zeyn/pdf`).
- Sayfa boyutları, satır sayıları, sorgu uzunlukları ve yükleme boyutları, istemci ne gönderirse göndersin sunucuda sınırlanır.

## Oturumlar

`App::idle_timeout(d)`, girdisiz geçen bir sürenin ardından, önce bir uyarı penceresi göstererek kullanıcıları sayfadan çıkarır. `reauth` işaretli action'lar yakın zamanda yapılmış bir oturum açma gerektirir (`App::reauth_within`).

## Veri koruma

Alan maskeleme, hassas kayıtları açmadan önce gerekçe sorma, alan düzeyinde denetim ve okuma denetimi [Audit, masking and jobs](enterprise.md) sayfasında anlatılır.

## API ve ajanlar

API çağrıları bir `Authorization` başlığı gerektirir; tarayıcı oturum çerezi tek başına API'yi ya da MCP uç noktasını asla çağırmaz, bu yüzden CSRF belirteci gerekmez. Ajanlar, adına çalıştıkları kullanıcının izinlerinin aynısına sahip olur. [API and OpenAPI](api.md) ve [MCP and AI agents](mcp.md) sayfalarına bakın.
