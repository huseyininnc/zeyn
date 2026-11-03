# Учётные записи пользователей

Приложения, которые хранят собственных пользователей, включают Cargo-функцию (feature) `accounts`. Она приносит регистрацию, пароли, подтверждение по e-mail, сброс пароля, блокировку, роли, двухэтапный вход, passkeys, внешние входы, готовые экраны и JSON-конечные точки для мобильных приложений. Приложениям, которые входят через прокси единого входа или провайдер идентификации, она не нужна. Полное описание с таблицей для тех, кто знает ASP.NET Core Identity, находится в `docs/accounts.md`.

```toml
zeyn = { version = "0.0.7", features = ["accounts"] }
```

## Включение

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

- Ключ берётся из конфигурации, имеет длину не менее 32 байт и никогда не меняется.
- Для PostgreSQL или Oracle используйте `SqlAccountStore` с `Db`; `accounts::create_tables` даёт миграцию.
- Роли становятся разрешениями через `role(...)`. Утверждения (claims) роли типа `permission` добавляют новые разрешения на экране администрирования.

## Менеджеры в действиях

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

## Собственные поля пользователя

```rust
#[derive(Serialize, Deserialize, Default)]
struct Profile { display_name: String }

let mut user = users.find_by_id(&id).await?.ok_or(Error::NotFound)?;
user.set_data(&Profile { display_name: "Ada".into() })?;
users.update(&mut user).await?;
```

## Экраны

`accounts::ui()` даёт вход, регистрацию, восстановление пароля, подтверждение e-mail, двухэтапный вход, коды восстановления, блокировку, страницы настроек учётной записи и, с `admin(true)`, пользователей и роли. `registration(false)` скрывает регистрацию; `without("account_passkeys")` убирает один экран, чтобы приложение могло предоставить собственный по тому же маршруту. Каждый переход между ними происходит без загрузки страницы, и обе темы и оба языка работают.

## Что стоит знать

- Вход отвечает одинаково для неизвестного пользователя и неверного пароля и занимает столько же времени.
- Пять неверных паролей блокируют учётную запись на пять минут; попытки также ограничиваются по адресу.
- Смена пароля завершает сеансы пользователя на других устройствах в течение тридцати минут.
- Хеши паролей, штампы, ключи, ссылки и коды никогда не попадают на экран, в ответ API, в журнал или в ИИ-модель.
- Пользователи, перенесённые из .NET-приложения, сохраняют свои пароли; при первом входе они перехешируются.
