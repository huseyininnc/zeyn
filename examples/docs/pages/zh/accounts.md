# 用户账户

需要自行管理用户的应用请启用 Cargo feature `accounts`。它提供注册、密码、邮箱确认、密码重置、账户锁定、角色、两步登录、通行密钥（passkey）、外部登录、现成的界面，以及面向移动应用的 JSON 端点。通过单点登录代理或身份提供方登录的应用不需要它。完整参考见 `docs/accounts.md`，其中有一张面向熟悉 ASP.NET Core Identity 的开发者的对照表。

```toml
zeyn = { version = "0.0.9", features = ["accounts"] }
```

## 启用

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

- 密钥来自配置，至少 32 字节，且永远不要更改。
- 使用 PostgreSQL 或 Oracle 时，将 `SqlAccountStore` 与 `Db` 搭配使用；`accounts::create_tables` 提供迁移脚本。
- 角色通过 `role(...)` 转换为权限。类型为 `permission` 的角色声明（claim）可在管理界面中添加更多权限。

## 在 action 中使用管理器

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

## 用户的自定义字段

```rust
#[derive(Serialize, Deserialize, Default)]
struct Profile { display_name: String }

let mut user = users.find_by_id(&id).await?.ok_or(Error::NotFound)?;
user.set_data(&Profile { display_name: "Ada".into() })?;
users.update(&mut user).await?;
```

## 界面

`accounts::ui()` 提供登录、注册、密码找回、邮箱确认、两步登录、恢复码、账户锁定、账户设置页面，并且在使用 `admin(true)` 时提供用户和角色管理。`registration(false)` 会隐藏注册；`without("account_passkeys")` 会去掉某一个界面，让应用可以在同一路由上提供自己的界面。它们之间的所有跳转都不会加载整页，并且支持两种主题和两种语言。

## 值得了解的规则

- 对于未知用户和错误密码，登录给出相同的响应，耗时也相同。
- 连续输入五次错误密码会将账户锁定五分钟；同一地址的尝试次数也有限制。
- 修改密码后，用户在其他设备上的登录会在三十分钟内失效。
- 密码哈希、安全戳、密钥、链接和验证码绝不会出现在界面、API 响应、日志或 AI 模型中。
- 从 .NET 应用迁移来的用户保留原有密码；首次登录时会重新计算哈希。
