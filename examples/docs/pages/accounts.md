# User accounts

Apps that keep their own users turn on the Cargo feature `accounts`. It brings registration,
passwords, e-mail confirmation, password reset, lockout, roles, two step sign in, passkeys,
external logins, ready screens and JSON endpoints for mobile apps. Apps that sign in through a
single sign on proxy or an identity provider do not need it. The full reference, with a table for
people who know ASP.NET Core Identity, is `docs/accounts.md`.

```toml
zeyn = { version = "0.0.3", features = ["accounts"] }
```

## Turn it on

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

- The key comes from configuration, is at least 32 bytes and never changes.
- Use `SqlAccountStore` with a `Db` for PostgreSQL or Oracle; `accounts::create_tables` gives the
  migration.
- Roles become permissions through `role(...)`. Role claims of type `permission` add more from
  the administration screen.

## Use the managers in actions

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

## Own fields on the user

```rust
#[derive(Serialize, Deserialize, Default)]
struct Profile { display_name: String }

let mut user = users.find_by_id(&id).await?.ok_or(Error::NotFound)?;
user.set_data(&Profile { display_name: "Ada".into() })?;
users.update(&mut user).await?;
```

## Screens

`accounts::ui()` gives sign in, registration, password recovery, e-mail confirmation, two step
sign in, recovery codes, lockout, the account settings pages and, with `admin(true)`, users and
roles. `registration(false)` hides registration; `without("account_passkeys")` leaves out one
screen so the app can serve its own at the same route. Every move between them happens without a
page load, and both themes and both languages work.

## Rules worth knowing

- Sign in answers the same for an unknown user and a wrong password, and takes as long.
- Five wrong passwords lock the account for five minutes; attempts are also limited per address.
- Changing the password signs the user out on other devices within thirty minutes.
- Password hashes, stamps, keys, links and codes never reach a screen, an API answer, the log or
  an AI model.
- Users moved from a .NET app keep their passwords; the first sign in rehashes them.
