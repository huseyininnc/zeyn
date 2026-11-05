# Kullanıcı hesapları

Kendi kullanıcılarını tutan uygulamalar `accounts` Cargo feature'ını açar. Kayıt, parolalar,
e-posta onayı, parola sıfırlama, kilitleme, roller, iki adımlı oturum açma, passkey'ler, harici
girişler, hazır ekranlar ve mobil uygulamalar için JSON endpoint'leri getirir. Oturumu bir
single sign on proxy'si veya bir kimlik sağlayıcı üzerinden açan uygulamalara gerekmez. ASP.NET
Core Identity bilenler için bir tabloyla birlikte tam başvuru `docs/accounts.md` dosyasındadır.

```toml
zeyn = { version = "0.0.8", features = ["accounts"] }
```

## Açmak

```rust
use zeyn::accounts::{self, Accounts, MailSender, MemoryAccountStore};

let accounts = Accounts::new(MemoryAccountStore::new(), config.secret("accounts.key")?)
    .public_url("https://shop.example.com")
    .sender(MailSender(mailer.clone()))
    .role("admins", [accounts::USERS_READ, accounts::USERS_WRITE, accounts::ROLES_WRITE])
    .role("clerks", ["shop.order.read", "shop.order.write"]);

App::new("Shop")
    .mailer(mailer)
    .accounts(accounts)
    .feature(accounts::ui().admin(true).build())
```

- Anahtar yapılandırmadan gelir, en az 32 bayttır ve asla değişmez.
- PostgreSQL veya Oracle için bir `Db` ile `SqlAccountStore` kullanın; `accounts::create_tables`
  migration'ı verir.
- Roller `role(...)` ile izinlere dönüşür. `permission` türündeki rol claim'leri yönetim
  ekranından daha fazlasını ekler.

## Action'larda yöneticileri kullanmak

```rust
use zeyn::accounts::{SignInManager, UserManager};

/// Sends the signed in user a new confirmation link.
#[action(public, ai = false)]
pub async fn welcome_again(_: (), users: UserManager, ctx: Ctx) -> Result<()> {
    let name = ctx.identity().user.clone().ok_or(Error::Forbidden)?;
    let user = users.find_by_name(&name).await?.ok_or(Error::NotFound)?;
    let _link_token = users.generate_email_confirmation_token(&user);
    Ok(())
}
```

## Kullanıcıda kendi alanlarınız

```rust
#[derive(Serialize, Deserialize, Default)]
struct Profile { display_name: String }

let mut user = users.find_by_id(&id).await?.ok_or(Error::NotFound)?;
user.set_data(&Profile { display_name: "Ada".into() })?;
users.update(&mut user).await?;
```

## Ekranlar

`accounts::ui()` oturum açma, kayıt, parola kurtarma, e-posta onayı, iki adımlı oturum açma,
kurtarma kodları, kilitleme, hesap ayarları sayfaları ve `admin(true)` ile kullanıcılar ve
roller ekranlarını verir. `registration(false)` kaydı gizler; `without("account_passkeys")` bir
ekranı dışarıda bırakır, böylece uygulama aynı rotada kendi ekranını sunabilir. Aralarındaki her
geçiş sayfa yüklemeden olur ve iki tema ile iki dil de çalışır.

## Bilmeye değer kurallar

- Oturum açma, bilinmeyen bir kullanıcı ve yanlış bir parola için aynı yanıtı verir ve aynı
  süreyi alır.
- Beş yanlış parola hesabı beş dakika kilitler; denemeler adres başına da sınırlanır.
- Parolayı değiştirmek kullanıcının diğer cihazlardaki oturumunu otuz dakika içinde kapatır.
- Parola karmaları (hash), damgalar, anahtarlar, bağlantılar ve kodlar asla bir ekrana, bir API
  yanıtına, günlüğe veya bir AI modeline ulaşmaz.
- Bir .NET uygulamasından taşınan kullanıcılar parolalarını korur; ilk oturum açmada parolalar
  yeniden karmalanır.
