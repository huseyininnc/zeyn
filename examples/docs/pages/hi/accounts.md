# उपयोगकर्ता खाते

जो ऐप अपने उपयोगकर्ता खुद रखते हैं, वे Cargo feature `accounts` चालू करते हैं। यह पंजीकरण, पासवर्ड, ईमेल पुष्टि, पासवर्ड रीसेट, लॉकआउट, भूमिकाएँ, दो चरणों में साइन इन, पासकी, बाहरी लॉगिन, तैयार स्क्रीन और मोबाइल ऐप के लिए JSON एंडपॉइंट लाता है। जो ऐप सिंगल साइन ऑन प्रॉक्सी या आइडेंटिटी प्रोवाइडर से साइन इन करते हैं, उन्हें इसकी ज़रूरत नहीं। पूरा संदर्भ, ASP.NET Core Identity जानने वालों के लिए एक तालिका के साथ, `docs/accounts.md` में है।

```toml
zeyn = { version = "0.0.6", features = ["accounts"] }
```

## इसे चालू करें

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

- कुंजी कॉन्फ़िगरेशन से आती है, कम से कम 32 बाइट की होती है और कभी नहीं बदलती।
- PostgreSQL या Oracle के लिए `Db` के साथ `SqlAccountStore` इस्तेमाल करें; `accounts::create_tables` माइग्रेशन देता है।
- भूमिकाएँ `role(...)` से अनुमतियाँ बन जाती हैं। `permission` प्रकार के Role claim प्रशासन स्क्रीन से और अनुमतियाँ जोड़ते हैं।

## action में मैनेजर इस्तेमाल करें

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

## उपयोगकर्ता पर अपने फ़ील्ड

```rust
#[derive(Serialize, Deserialize, Default)]
struct Profile { display_name: String }

let mut user = users.find_by_id(&id).await?.ok_or(Error::NotFound)?;
user.set_data(&Profile { display_name: "Ada".into() })?;
users.update(&mut user).await?;
```

## स्क्रीन

`accounts::ui()` साइन इन, पंजीकरण, पासवर्ड रिकवरी, ईमेल पुष्टि, दो चरणों में साइन इन, रिकवरी कोड, लॉकआउट, खाता सेटिंग पृष्ठ और, `admin(true)` के साथ, उपयोगकर्ता और भूमिकाएँ देता है। `registration(false)` पंजीकरण छिपाता है; `without("account_passkeys")` एक स्क्रीन हटा देता है ताकि ऐप उसी रूट पर अपनी स्क्रीन दे सके। इनके बीच हर बदलाव बिना पृष्ठ लोड के होता है, और दोनों थीम और दोनों भाषाएँ काम करती हैं।

## जानने योग्य नियम

- अज्ञात उपयोगकर्ता और गलत पासवर्ड पर साइन इन एक जैसा उत्तर देता है, और उतना ही समय लेता है।
- पाँच गलत पासवर्ड खाते को पाँच मिनट के लिए लॉक कर देते हैं; प्रयास प्रति पता भी सीमित हैं।
- पासवर्ड बदलने पर उपयोगकर्ता तीस मिनट के भीतर अन्य डिवाइसों पर साइन आउट हो जाता है।
- पासवर्ड हैश, स्टैम्प, कुंजियाँ, लिंक और कोड कभी स्क्रीन, API उत्तर, लॉग या AI मॉडल तक नहीं पहुँचते।
- .NET ऐप से लाए गए उपयोगकर्ता अपने पासवर्ड रखते हैं; पहला साइन इन उन्हें फिर से हैश करता है।
