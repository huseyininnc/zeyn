# 사용자 계정

자체 사용자를 관리하는 앱은 Cargo 기능 `accounts`를 켭니다. 이 기능은 회원 가입, 비밀번호, 이메일 확인, 비밀번호 재설정, 잠금, 역할, 2단계 로그인, 패스키, 외부 로그인, 준비된 화면, 모바일 앱용 JSON 엔드포인트를 제공합니다. 싱글 사인온 프록시나 ID 제공자를 통해 로그인하는 앱에는 필요하지 않습니다. ASP.NET Core Identity를 아는 사람을 위한 비교 표가 포함된 전체 레퍼런스는 `docs/accounts.md`입니다.

```toml
zeyn = { version = "0.0.10", features = ["accounts"] }
```

## 켜기

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

- 키는 설정에서 가져오며, 길이는 최소 32바이트이고 절대 바꾸지 않습니다.
- PostgreSQL 또는 Oracle에는 `Db`와 함께 `SqlAccountStore`를 사용합니다. 마이그레이션은 `accounts::create_tables`가 제공합니다.
- 역할은 `role(...)`을 통해 권한이 됩니다. `permission` 유형의 역할 클레임은 관리 화면에서 권한을 더 추가합니다.

## action에서 매니저 사용하기

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

## 사용자의 자체 필드

```rust
#[derive(Serialize, Deserialize, Default)]
struct Profile { display_name: String }

let mut user = users.find_by_id(&id).await?.ok_or(Error::NotFound)?;
user.set_data(&Profile { display_name: "Ada".into() })?;
users.update(&mut user).await?;
```

## Screen

`accounts::ui()`는 로그인, 회원 가입, 비밀번호 복구, 이메일 확인, 2단계 로그인, 복구 코드, 잠금, 계정 설정 페이지를 제공하며, `admin(true)`를 쓰면 사용자와 역할 화면도 제공합니다. `registration(false)`는 회원 가입을 숨기고, `without("account_passkeys")`는 화면 하나를 제외하여 앱이 같은 경로에서 자체 화면을 제공할 수 있게 합니다. 화면 사이의 모든 이동은 페이지 로드 없이 이루어지며, 두 테마와 두 언어 모두에서 동작합니다.

## 알아 둘 규칙

- 로그인은 존재하지 않는 사용자와 틀린 비밀번호에 같은 응답을 주고, 걸리는 시간도 같습니다.
- 비밀번호를 다섯 번 틀리면 계정이 5분 동안 잠기며, 시도는 주소별로도 제한됩니다.
- 비밀번호를 바꾸면 30분 안에 다른 기기에서 로그아웃됩니다.
- 비밀번호 해시, 스탬프, 키, 링크, 코드는 화면, API 응답, 로그, AI 모델에 절대 전달되지 않습니다.
- .NET 앱에서 옮겨 온 사용자는 비밀번호를 유지하며, 첫 로그인 때 다시 해시됩니다.
