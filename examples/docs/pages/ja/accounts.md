# ユーザーアカウント

独自のユーザーを保持するアプリは、Cargo の feature `accounts` を有効にします。これにより、登録、
パスワード、メール確認、パスワードリセット、ロックアウト、ロール、2 段階サインイン、パスキー、
外部ログイン、用意済みの画面、モバイルアプリ向けの JSON エンドポイントが使えるようになります。
シングルサインオンのプロキシや ID プロバイダー経由でサインインするアプリには不要です。ASP.NET
Core Identity を知っている人向けの対応表を含む完全なリファレンスは `docs/accounts.md` にあります。

```toml
zeyn = { version = "0.0.10", features = ["accounts"] }
```

## 有効にする

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

- キーは設定から取得し、32 バイト以上で、決して変更しません。
- PostgreSQL または Oracle には、`Db` とともに `SqlAccountStore` を使います。`accounts::create_tables`
  がマイグレーションを提供します。
- ロールは `role(...)` を通じて権限になります。`permission` 型のロールクレームを使うと、管理画面から
  さらに権限を追加できます。

## アクションでマネージャーを使う

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

## ユーザー独自のフィールド

```rust
#[derive(Serialize, Deserialize, Default)]
struct Profile { display_name: String }

let mut user = users.find_by_id(&id).await?.ok_or(Error::NotFound)?;
user.set_data(&Profile { display_name: "Ada".into() })?;
users.update(&mut user).await?;
```

## 画面

`accounts::ui()` は、サインイン、登録、パスワード回復、メール確認、2 段階サインイン、回復コード、
ロックアウト、アカウント設定ページを提供し、`admin(true)` を指定するとユーザーとロールの画面も
提供します。`registration(false)` は登録を非表示にします。`without("account_passkeys")` は
1 つの画面を除外し、アプリが同じルートで独自の画面を提供できるようにします。画面間の移動はすべて
ページを読み込まずに行われ、両方のテーマと両方の言語で動作します。

## 知っておくべきルール

- サインインは、存在しないユーザーでも誤ったパスワードでも同じ応答を返し、かかる時間も同じです。
- パスワードを 5 回間違えると、アカウントは 5 分間ロックされます。試行回数はアドレスごとにも制限されます。
- パスワードを変更すると、他のデバイスでは 30 分以内にサインアウトされます。
- パスワードのハッシュ、スタンプ、キー、リンク、コードが、画面、API の応答、ログ、AI モデルに届くことはありません。
- .NET アプリから移行したユーザーはパスワードを保持し、最初のサインイン時に再ハッシュされます。
