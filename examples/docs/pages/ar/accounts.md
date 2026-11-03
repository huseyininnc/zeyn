# حسابات المستخدمين

التطبيقات التي تحتفظ بمستخدميها بنفسها تفعّل ميزة Cargo المسماة `accounts`. وهي توفر التسجيل وكلمات المرور وتأكيد البريد الإلكتروني وإعادة تعيين كلمة المرور والقفل والأدوار وتسجيل الدخول بخطوتين ومفاتيح المرور (passkeys) وتسجيل الدخول الخارجي وشاشات جاهزة ونقاط نهاية JSON لتطبيقات الجوال. أما التطبيقات التي تسجل الدخول عبر وكيل تسجيل دخول موحد أو مزود هوية فلا تحتاج إليها. المرجع الكامل، مع جدول لمن يعرفون ASP.NET Core Identity، هو `docs/accounts.md`.

```toml
zeyn = { version = "0.0.6", features = ["accounts"] }
```

## التفعيل

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

- يأتي المفتاح من الإعدادات، وطوله 32 بايت على الأقل، ولا يتغير أبدًا.
- استخدم `SqlAccountStore` مع `Db` لقاعدة PostgreSQL أو Oracle؛ وتوفر `accounts::create_tables` سكربت الترحيل (migration).
- تتحول الأدوار إلى صلاحيات عبر `role(...)`. وتضيف مطالبات الأدوار (role claims) من النوع `permission` صلاحيات أخرى من شاشة الإدارة.

## استخدام المديرين في الإجراءات

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

## حقول خاصة بالمستخدم

```rust
#[derive(Serialize, Deserialize, Default)]
struct Profile { display_name: String }

let mut user = users.find_by_id(&id).await?.ok_or(Error::NotFound)?;
user.set_data(&Profile { display_name: "Ada".into() })?;
users.update(&mut user).await?;
```

## الشاشات

توفر `accounts::ui()` شاشات تسجيل الدخول والتسجيل واستعادة كلمة المرور وتأكيد البريد الإلكتروني وتسجيل الدخول بخطوتين ورموز الاسترداد والقفل وصفحات إعدادات الحساب، ومع `admin(true)` شاشتي المستخدمين والأدوار. تخفي `registration(false)` شاشة التسجيل؛ وتستبعد `without("account_passkeys")` شاشة واحدة ليقدم التطبيق شاشته الخاصة على المسار نفسه. كل انتقال بينها يتم دون تحميل الصفحة، ويعمل السمتان واللغتان كلتاهما.

## قواعد تستحق المعرفة

- يعطي تسجيل الدخول الجواب نفسه لمستخدم غير معروف ولكلمة مرور خاطئة، ويستغرق الوقت نفسه.
- خمس كلمات مرور خاطئة تقفل الحساب خمس دقائق؛ كما تُحدّ المحاولات لكل عنوان.
- يؤدي تغيير كلمة المرور إلى تسجيل خروج المستخدم من الأجهزة الأخرى خلال ثلاثين دقيقة.
- لا تصل تجزئات كلمات المرور والأختام (stamps) والمفاتيح والروابط والرموز إلى شاشة أو جواب API أو السجل أو نموذج ذكاء اصطناعي.
- المستخدمون المنقولون من تطبيق .NET يحتفظون بكلمات مرورهم؛ ويعيد أول تسجيل دخول تجزئتها.
