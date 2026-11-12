# Kimlik doğrulama

Kim çağırıyor ve uygulama bundan ne kadar emin. Zeyn'de içeri girmenin dört yolu vardır:

| Yol | Ne için kullanılır | Cargo feature |
| --- | --- | --- |
| `ForwardAuth` | uygulamanın önündeki bir tek oturum açma (single sign on) proxy'si başlıkları ayarlar | varsayılan |
| `JwtAuth` | `Authorization: Bearer <JWT>` ile API, MCP ve servis çağrıları | `oidc` |
| `OidcAuth` | tarayıcı kullanıcıları bir OpenID Connect sağlayıcısında oturum açar | `oidc` |
| `Totp` | oturum açmasında ikinci faktör olmayan uygulamalar için ikinci faktör | `totp` |
| `App::accounts` | uygulama kendi kullanıcılarını tutar: parolalar, iki faktör, passkey'ler (`docs/accounts.md`) | `accounts` |

`RequireMfa` (her zaman kullanılabilir) bir Feature, bir screen veya bir action için ikinci faktör
ister. Varsayılan derlemede `oidc` ve `totp` yoktur, bu yüzden bunları kullanmayan uygulamalar
bunun bedelini ödemez.

```toml
zeyn = { version = "0.0.10", features = ["oidc"] }
```

Test edilen sağlayıcılar Keycloak ve Authentik'tir. OpenID Connect keşfini (discovery), PKCE ile
yetkilendirme kodu akışını (authorization code flow) ve JWKS'i izleyen her sağlayıcının çalışması
beklenir.

## API ve MCP için Bearer token'lar

```rust
use zeyn::{ClaimMap, JwtAuth};

App::new("Stock").auth(
    JwtAuth::new("https://sso.example.com/realms/main", "stock-api")
        .claims(
            ClaimMap::new()
                .roles("realm_access.roles")
                .role("stock-admins", ["stock.product.read", "stock.product.write"]),
        ),
)
```

- İlk argüman, token'ların `iss` alanında göründüğü haliyle issuer'dır. İmzalama anahtarları
  `<issuer>/.well-known/openid-configuration` içinde adı geçen JWKS'ten gelir; issuer'ın keşif
  belgesi yoksa `jwks_url` adresi ayarlar.
- İkinci argüman, token'ın `aud` içinde adlandırması gereken audience'tır. Keycloak'ta, API için
  token isteyen client'a bir "Audience" mapper ekleyin.
- `algorithms([...])` daha fazlasını söylemedikçe yalnızca `RS256` kabul edilir (`RS384`, `RS512`,
  `PS256`, `PS384`, `PS512`, `ES256`, `ES384`, `EdDSA`). `none` ve her `HS*` algoritması daima
  reddedilir: bunlara izin vermenin bir yolu yoktur. Bir anahtar yalnızca kendi türünün algoritma
  ailesi için kullanılır, bu yüzden genel anahtar üzerinden HMAC ile "imzalanmış" bir token reddedilir.
- `exp` zorunludur. `exp`, `nbf` ve `iat` bir dakikalık tolerans (leeway) alır (`leeway(...)`, en
  fazla beş dakika).
- Anahtarlar bir saat önbellekte tutulur (`key_ttl`). Bilinmeyen bir `kid` adlandıran bir token
  anahtarları yeniden çeker, en fazla 30 saniyede bir (`min_key_refresh`); böylece uydurma anahtar
  kimlikleri taşıyan token'lar sağlayıcıyı boğamaz. En fazla 16 anahtar tutulur ve 256 KB'ı aşan
  yanıtlar reddedilir. Başarısız bir çekme eski anahtarları korur.
- Reddedilen bir token yalnızca oturum açmamış sayılır; neden yalnızca debug düzeyinde günlüğe yazılır.

### Tenant'lar

Her authenticator `Identity` içindeki aynı iki alanı doldurur: `tenant` (kullanıcının kendi
tenant'ı) ve `tenants` (kullanıcının çalışabildiği diğer her tenant). `Identity::member_of(id)` her
ikisi için de doğrudur; bir süper kullanıcı (`DevAuth`) her tenant'a aittir.

| Authenticator | Tenant'lar nereden gelir |
| --- | --- |
| `JwtAuth`, `OidcAuth` | `ClaimMap::tenant` ile adlandırılan claim, bir dize veya liste |
| `accounts` | kullanıcının `tenant` türündeki claim'leri (`Claim::new("tenant", "acme")`), eklendikleri sırayla |
| `ForwardAuth` | `ForwardAuth::tenant` ile adlandırılan başlık, tek bir tenant |
| kendi `Authenticator`'ınız | `identity.tenant` ve `identity.tenants` ayarlayın |

`App::tenancy(resolver)` ile bir isteğin tenant'ı isteğin kendisinden gelir (host veya başlık).
Oturum açmış bir kullanıcı ona ait olmalıdır; aksi halde istek oturumsuz çalışır, böylece herkese
açık screen'ler yine çalışır ve geri kalan her şey anonim bir kullanıcıya olduğu gibi yanıt verir.
Hiç tenant'ı olmayan oturum açmış bir kullanıcı hiçbirine ait değildir. Resolver tenant bulamazsa
kullanıcının kendi tenant'ı geçerli olur.

### Claim'ler

`ClaimMap`, claim'leri `Identity` haline getirir:

| Yöntem | Varsayılan | Notlar |
| --- | --- | --- |
| `user("email")` | `preferred_username`, yoksa `sub` | bunu içermeyen token reddedilir |
| `tenant("tenant")` | yok | kullanıcının tenant'ı; bir liste (dizi veya boşlukla ayrılmış) kullanıcının ait olduğu her tenant'ı adlandırır, ilki kullanıcının kendisidir. Aşağıdaki "Tenant'lar" bölümüne bakın |
| `roles("realm_access.roles")` | yok | noktalı yol; Keycloak realm rolleri veya Authentik'te `groups` |
| `role("clerks", [...])` | yok | bir rolün sahipleri veya bir grubun üyeleri için izinler |
| `permissions("scope")` | yok | değerleri izin olan claim; bir liste veya boşlukla ayrılmış dize |
| `time_zone("zoneinfo")` | `zoneinfo` | kullanıcının saat dilimi |
| `mfa_acr(["2"])` | yok | ikinci faktör anlamına gelen `acr` değerleri; aşağıya bakın |

`OidcAuth` ID token'ın claim'lerini okur. Keycloak, client'ın "realm roles" mapper'ında "Add to ID
token" açık olmadıkça realm rollerini yalnızca access token'a koyar; bunu açın (veya `id.token.claim`
ile bir `oidc-usermodel-realm-role-mapper` ekleyin). Authentik, `profile` scope'u ile `groups`
değerini ID token'a koyar.

`auth_time` (yoksa `iat`), `reauth` action'larının okuduğu `Identity::authenticated_at` olur.

## Tarayıcıdan oturum açma

```rust
use zeyn::{ClaimMap, OidcAuth};

let oidc = OidcAuth::new(
    "https://sso.example.com/realms/main",
    "stock-web",
    "https://stock.example.com",
    config.secret("auth.session_key")?,
)
.client_secret(config.secret("auth.client_secret")?)
.claims(ClaimMap::new().roles("realm_access.roles").role("stock-admins", ["stock.product.write"]));

App::new("Stock").oidc(oidc)
```

`App::oidc` bunu authenticator, `login_url`, boşta oturum kapatma adresi ve yeniden kimlik doğrulama
adresi yapar. Üç route ekler:

| Route | Ne yapar |
| --- | --- |
| `GET /_zeyn/auth/login?return_to=/path` | bir oturum açma başlatır ve sağlayıcıya yönlendirir |
| `GET /_zeyn/auth/callback` | sağlayıcının geri yönlendirmesi |
| `POST`, sonra `GET /_zeyn/auth/logout` | CSRF token'lı `POST` oturumu burada sonlandırır, ardından `GET` sağlayıcıda oturumu kapatır |

Sağlayıcıda, `<public url>/_zeyn/auth/callback` adresini yönlendirme URI'si (redirect URI) olarak ve
`<public url>/` adresini oturum kapatma sonrası yönlendirme URI'si olarak kaydedin. Genel URL
yapılandırılır, hiçbir zaman isteğin `Host` başlığından alınmaz. Bir temel veya genel yol (base or
public path) varsa route'lar onun altına taşınır.

### Akış

1. Oturumu kapalı bir kullanıcı izin gerektiren bir sayfa açar ve `/_zeyn/auth/login` adresine gider.
2. Sunucu yeni bir `state`, `nonce` ve PKCE verifier üretir (her biri 32 rastgele bayt), bunları
   `return_to` yoluyla birlikte on dakika yaşayan bir cookie içine mühürler ve
   `code_challenge_method=S256` ile sağlayıcının yetkilendirme uç noktasına yönlendirir.
3. Sağlayıcı kullanıcıyı bir kod ve `state` ile geri gönderir. Sunucu cookie'yi açar, `state`
   değerini sabit zamanlı karşılaştırır, cookie'yi siler ve kodu PKCE verifier ile takas eder
   (ayarlıysa client secret HTTP basic authentication üzerinden).
4. ID token'ın imzası sağlayıcının JWKS'ine karşı denetlenir, ardından `iss`, birden çok audience
   varsa `azp`, `aud`, `exp`, `nbf`, `iat` ve `nonce` denetlenir.
5. Oturum cookie'si ayarlanır ve kullanıcı `return_to` adresine döner. Orada yalnızca yerel bir yol
   kabul edilir; başka her şey uygulamanın başlangıç sayfasına gider.

### Oturum cookie'si

Sunucuda bir oturum deposu yoktur. Oturum tek bir cookie içinde yaşar, `zeyn_session`:

- AES-256-GCM ile şifrelenmiş ve doğrulanmıştır. Anahtar oturum anahtarından türetilir
  (`Config::secret`, en az 32 bayt; daha kısası uygulamayı başlangıçta durdurur). Cookie adı işin
  içine bağlıdır, bu yüzden bir cookie için mühürlenen değer başka biri olarak asla açılmaz.
  Değişen bir bayt veya başka bir anahtarla mühürlenmiş bir cookie yalnızca oturum açmamış sayılır.
- Kullanıcıyı, tenant'ı, saat dilimini, `ClaimMap`'in eşlediği rolleri, doğrudan izinleri, oturum
  açma zamanını, ikinci faktörün kullanılıp kullanılmadığını, oturum sonunu ve refresh token'larla
  birlikte refresh token'ı tutar. ID token'ı veya access token'ı tutmaz.
- `HttpOnly`, `SameSite=Lax` (sağlayıcının geri yönlendirmesi siteler arası bir gezinmedir) ve genel
  URL `https://` ise (veya `App::secure_cookies(true)` ile) `Secure`.
- 3800 bayttan büyük olacak bir oturum, bir günlük iletisiyle oturum açılırken reddedilir, asla
  kesilmez. Daha az rol eşleyin veya refresh token'ları kapatın.
- `session_lifetime` kadar sürer (ayarlanmadıkça sekiz saat). Oturum anahtarını değiştirmek herkesin
  oturumunu kapatır.

CSRF koruması değişmez: oturumu hangi cookie açarsa açsın, runtime'ın her `POST` isteği yine
`x-zeyn-csrf` başlığında `zeyn_csrf` token'ını ister.

### Refresh token'lar

`refresh_tokens(true)` refresh token'ı cookie içinde tutar. Token'ların süresi dolduğunda, bir
sonraki istek çalışmadan önce onları yeniler ve yenilenmiş cookie'yi geri gönderir. Reddedilen bir
yenileme (kullanıcı devre dışı bırakıldı veya sağlayıcıda oturumu kapatıldı) oturumu sonlandırır.
Refresh token'lar olmadan bir oturum, sağlayıcıda ne olursa olsun `session_lifetime` kadar sürer.
Yenileme tüm uygulamanın üzerinde tek bir katman olarak çalışır, bu yüzden isteğe bağlı özelliklerin
route'ları da dahil her route kapsanır.

Sağlayıcı refresh token'ları döndürürse (rotation) ve eskisini hemen iptal ederse, aynı anda yenileme
yapan iki istek kullanıcının oturumunu kapatabilir. Rotation'ı kapalı tutun veya kısa bir süre
yeniden kullanıma izin verin.

### Oturum kapatma

Oturum kapatma iki adımdır; böylece başka bir sitedeki link veya resim kullanıcının oturumunu
kapatamaz. CSRF token'lı `POST /_zeyn/auth/logout` oturum cookie'sini siler ve uygulamanın
tarayıcı depolamasını temizler. Ardından `GET /_zeyn/auth/logout` tarayıcıyı `client_id` ve
`post_logout_redirect_uri` ile sağlayıcının `end_session_endpoint` adresine gönderir; bu uç nokta
yoksa kullanıcı uygulamanın başlangıç sayfasına gider. Hâlâ oturumu olan bir tarayıcının `GET`
isteği yalnızca uygulamaya geri döner.

Site kabuğundaki oturum kapatma linki ve boşta kalma ile oturum kapatma iki adımı da yapar. Kendi
linkiniz de `data-zeyn-signout` ile aynısını yapar: runtime önce o adrese `POST` gönderir, sonra
linki izler.

### Aynı uygulamanın API ve MCP client'ları

`OidcAuth::accept_bearer("stock-api")` aynı issuer'ın o audience için bearer access token'larını da
aynı claim eşlemesiyle kabul eder. `Authorization: Bearer` başlığı olan bir istek yalnızca token'ına
göre değerlendirilir.

## İkinci faktör ve step up

`Identity::mfa`, kullanıcı ikinci bir faktörü kanıtladığında doğrudur:

- `OidcAuth` ve `JwtAuth` ile, `amr` `mfa`, `otp`, `hwk`, `swk`, `sc`, `sms`, bir biyometrik veya
  başka bir ikinci faktörü adlandırdığında (RFC 8176) ya da `acr` `ClaimMap::mfa_acr` değerlerinden
  biri olduğunda;
- `Totp` ile, doğrulanmış bir koddan sonra.

`RequireMfa` bunu ister:

```rust
Feature::new("payroll").guard(RequireMfa::new())

App::new("Back office").interceptor(RequireMfa::new().screen("bank_accounts").action("approve_payment"))
```

Bir Feature guard'ı olarak tüm Feature'ı kapsar. Bir interceptor olarak listelenen screen'leri ve
action'ları, ya da her iki liste de boşsa her çağrıyı kapsar.

Faktör eksik olduğunda, bir sayfa yüklemesi `return_to` ile step up adresine yönlendirilir. Diğer
çağrılar `mfa_required` koduyla `401` yanıtlar (API'de problem türü `urn:zeyn:problem:mfa_required`);
runtime step up sayfasını yeni bir sekmede açar ve kullanıcı geri geldiğinde isteği tekrarlar,
`reauth_required` için yaptığı gibi.

`OidcAuth` ile step up adresi `/_zeyn/auth/login?mfa=1` olur. `prompt=login` ve `max_age=0` ile
birlikte `acr_values` (`mfa_acr` değerleri) gönderir. Yeni ID token hâlâ ikinci faktör göstermiyorsa,
callback döngüye girmek yerine onu reddeder.

- **Keycloak**: tarayıcı akışına kimlik doğrulama seviyeleri verin (seviye 2'de koşullu bir OTP veya
  WebAuthn adımı) ve `mfa_acr(["2"])` kullanın. Keycloak `acr` için ulaşılan seviyeyle yanıt verir;
  client adları seviyelere eşliyorsa (`acr.loa.map`) adıyla yanıt verir, o durumda bu adları listeleyin.
- **Authentik**: Authentik, akış bu aşamaları kullandığında kullanıcının nasıl oturum açtığını `amr`
  üzerinden bildirir. Uygulama için bir authenticator validation aşaması içeren bir akış kullanın.

`OidcAuth` altındaki `reauth` action'ları `/_zeyn/auth/login?reauth=1` adresine gider; bu adres
`prompt=login` ve `max_age=0` gönderir, böylece yeni `auth_time` günceldir.

## Yerleşik TOTP

Oturum açmasında ikinci faktör olmayan uygulamalar için (MFA'sız bir proxy, uygulamaya özel bir
authenticator):

```rust
use zeyn::{Totp, TotpStore};

App::new("Back office")
    .auth(ForwardAuth::new("x-forwarded-user"))
    .totp(Totp::new(TotpTable::new(pool), config.secret("auth.totp_key")?).issuer("Back office"))
```

- `/_zeyn/auth/totp`, ikinci faktörü olmayan bir kullanıcıya kaydı (enrollment) gösterir: sunucuda
  SVG olarak çizilen bir QR kod (betik yok, görüntü isteği yok), elle yazılacak anahtar ve bir kod
  alanı. Sır (secret), bir kod uygulamanın ona sahip olduğunu kanıtlayana kadar on dakika boyunca
  mühürlü bir cookie içinde bekler.
- Kayıttan sonra kullanıcı on kurtarma kodunu bir kez görür. Store yalnızca anahtarlı özetlerini
  (HMAC-SHA256, TOTP anahtarından türetilen bir anahtarla) tutar, bu yüzden yalnızca store'un bir
  kopyası çevrimdışı tahmin edilemez. Her biri bir kez çalışır.
- Saklanan sır TOTP anahtarıyla mühürlenir ve kullanıcıya bağlanır; bu yüzden yalnızca store kod
  üretemez ve kayıtlar kullanıcılar arasında değiştirilemez.
- Kayıtlı kullanıcılar orada bir kodla veya bir kurtarma koduyla doğrular. Bir kod bir kez kabul edilir
  (son kullanılan zaman adımı saklanır) ve her iki yönde bir adım saat sapması tolere edilir.
- Denemeler kullanıcı başına sınırlıdır (`attempts` başka söylemedikçe beş dakikada beş), uygulamanın
  rate store'u üzerinden; böylece birden çok sunucu sayımı `RedisStore` ile paylaşır.
- Doğrulanmış bir kod, `lifetime` kadar (ayarlanmadıkça on iki saat) mühürlü bir `zeyn_mfa` cookie'si
  ayarlar; bu cookie kimliği `mfa` ile işaretler ve `reauth` action'ları için yeni bir oturum açma
  sayılır.
- Kayıtlı bir faktör yalnızca bir oturumdan değiştirilemez; sıfırlamak store'da bir yönetici işidir.
- Formlar aynı origin'e `zeyn_csrf` token'ı bir form alanı olarak gönderilerek post edilir.

`TotpStore` iki async yöntemdir, `load(user)` ve `save(user, record)`; `MemoryTotpStore` testler
içindir. TOTP anahtarını sabit tutun: başka bir anahtarla mühürlenen kayıtlar artık açılmaz.

## Test

`crates/zeyn/tests/oidc.rs` her akışı, testin içinde çalışmak için üretilmiş bir anahtarla imzalayan
küçük bir issuer'a karşı çalıştırır ve süresi dolmuş token'ların, yanlış audience ve issuer'ların,
`alg=none`'ın, genel anahtarla anahtarlanmış bir HMAC token'ın, değiştirilmiş claim'lerin,
kurcalanmış cookie'lerin, yeniden kullanılmış veya uyuşmayan `state` değerinin ve yanlış bir `nonce`
değerinin reddedildiğini denetler. `crates/zeyn/tests/totp.rs` kaydı, doğrulamayı, tekrar oynatmayı
(replay), kurtarma kodlarını ve rate limit'i kapsar.

Geliştirme kipinde bir Keycloak ile aynı akışlar ona karşı da çalışır:

```
docker run --rm -p 8080:8080 -e KC_BOOTSTRAP_ADMIN_USERNAME=admin -e KC_BOOTSTRAP_ADMIN_PASSWORD=admin quay.io/keycloak/keycloak start-dev
ZEYN_TEST_OIDC=http://127.0.0.1:8080 cargo test -p zeyn --features oidc --test oidc
```

Test kendi client'ı ve kullanıcısıyla bir `zeyn-test` realm'i oluşturur ve sonunda siler.
`ZEYN_TEST_OIDC` olmadan atlanır.
