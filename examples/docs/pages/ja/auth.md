# 認証

誰が呼び出しているのか、そしてアプリがそれをどの程度確信しているのかを扱います。Zeyn には 4 つのサインイン方法があります。

| 方法 | 用途 | Cargo feature |
| --- | --- | --- |
| `ForwardAuth` | アプリの前段にあるシングルサインオンのプロキシがヘッダーを設定する | デフォルト |
| `JwtAuth` | `Authorization: Bearer <JWT>` を使う API、MCP、サービス呼び出し | `oidc` |
| `OidcAuth` | ブラウザーのユーザーが OpenID Connect プロバイダーでサインインする | `oidc` |
| `Totp` | サインインに第 2 要素がないアプリのための第 2 要素 | `totp` |
| `App::accounts` | アプリが独自のユーザーを管理する: パスワード、2 要素認証、パスキー (`docs/accounts.md`) | `accounts` |

`RequireMfa` (常に利用可能) は、feature、screen、action に対して第 2 要素を要求します。
デフォルトのビルドには `oidc` も `totp` も含まれないため、これらを使わないアプリは何も負担しません。

```toml
zeyn = { version = "0.0.9", features = ["oidc"] }
```

動作確認済みのプロバイダーは Keycloak と Authentik です。OpenID Connect の discovery、PKCE を使う認可コードフロー、JWKS に従うプロバイダーであれば動作するはずです。

## API と MCP のための Bearer トークン

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

- 最初の引数は、トークンの `iss` に現れるとおりの issuer です。署名鍵は `<issuer>/.well-known/openid-configuration` に記載された JWKS から取得します。issuer に discovery ドキュメントがない場合は `jwks_url` でアドレスを指定します。
- 2 番目の引数は、トークンが `aud` に含めなければならない audience です。Keycloak では、API 向けのトークンを要求するクライアントに "Audience" マッパーを追加します。
- `algorithms([...])` で追加しない限り、受け付けるのは `RS256` のみです (追加できるのは `RS384`、`RS512`、`PS256`、`PS384`、`PS512`、`ES256`、`ES384`、`EdDSA`)。`none` とすべての `HS*` アルゴリズムは常に拒否され、許可する方法はありません。鍵はその種類に対応するアルゴリズムファミリーでのみ使われるため、公開鍵を使って HMAC で「署名」されたトークンは拒否されます。
- `exp` は必須です。`exp`、`nbf`、`iat` には 1 分の猶予があります (`leeway(...)`、最大 5 分)。
- 鍵は 1 時間キャッシュされます (`key_ttl`)。未知の `kid` を持つトークンが来ると鍵を再取得しますが、30 秒に 1 回までです (`min_key_refresh`)。そのため、でたらめな鍵 ID のトークンでプロバイダーを溢れさせることはできません。保持する鍵は最大 16 個で、256 KB を超えるレスポンスは拒否されます。取得に失敗した場合は古い鍵を保持します。
- 拒否されたトークンは単にサインインしていない扱いになり、理由は debug レベルでのみログに記録されます。

### テナント

すべての authenticator は `Identity` の同じ 2 つのフィールドを設定します。`tenant` (ユーザー自身のテナント) と `tenants` (ユーザーが作業できるそれ以外のすべてのテナント) です。`Identity::member_of(id)` はどちらに対しても true を返します。スーパーユーザー (`DevAuth`) はすべてのテナントに所属します。

| Authenticator | テナントの取得元 |
| --- | --- |
| `JwtAuth`, `OidcAuth` | `ClaimMap::tenant` で指定したクレーム (文字列またはリスト) |
| `accounts` | ユーザーの `tenant` 型のクレーム (`Claim::new("tenant", "acme")`)、追加された順 |
| `ForwardAuth` | `ForwardAuth::tenant` で指定したヘッダー (テナントは 1 つ) |
| 独自の `Authenticator` | `identity.tenant` と `identity.tenants` を設定する |

`App::tenancy(resolver)` を使うと、リクエストのテナントはリクエスト (ホストまたはヘッダー) から決まります。
サインイン済みのユーザーはそのテナントに所属している必要があります。所属していない場合、リクエストはサインアウト状態で実行されるため、公開 screen は引き続き動作し、それ以外は匿名ユーザーと同じ応答になります。テナントを一切持たないサインイン済みユーザーは、どのテナントにも所属しません。resolver がテナントを見つけられない場合は、ユーザー自身のテナントが適用されます。

### クレーム

`ClaimMap` はクレームを `Identity` に変換します。

| メソッド | デフォルト | 備考 |
| --- | --- | --- |
| `user("email")` | `preferred_username`、なければ `sub` | これがないトークンは拒否される |
| `tenant("tenant")` | なし | ユーザーのテナント。リスト (配列またはスペース区切り) はユーザーが所属するすべてのテナントを表し、最初がユーザー自身のもの。下の「テナント」を参照 |
| `roles("realm_access.roles")` | なし | ドット区切りのパス。Keycloak の realm ロール、または Authentik の `groups` |
| `role("clerks", [...])` | なし | ロールの保持者またはグループのメンバーへの権限 |
| `permissions("scope")` | なし | 値が権限であるクレーム。リストまたはスペース区切りの文字列 |
| `time_zone("zoneinfo")` | `zoneinfo` | ユーザーのタイムゾーン |
| `mfa_acr(["2"])` | なし | 第 2 要素を意味する `acr` の値。下を参照 |

`OidcAuth` は ID トークンのクレームを読み取ります。Keycloak は、クライアントの "realm roles" マッパーで "Add to ID token" をオンにしない限り、realm ロールをアクセストークンにのみ入れます。これをオンにしてください (または `id.token.claim` を指定した `oidc-usermodel-realm-role-mapper` を追加します)。Authentik は `profile` スコープで `groups` を ID トークンに入れます。

`auth_time` (なければ `iat`) は `Identity::authenticated_at` になり、`reauth` の action がこれを読み取ります。

## ブラウザーでのサインイン

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

`App::oidc` は、これを authenticator、`login_url`、アイドル時のサインアウトのアドレス、再認証のアドレスとして設定します。さらに 3 つのルートを追加します。

| ルート | 動作 |
| --- | --- |
| `GET /_zeyn/auth/login?return_to=/path` | サインインを開始し、プロバイダーへリダイレクトする |
| `GET /_zeyn/auth/callback` | プロバイダーからのリダイレクト先 |
| `GET /_zeyn/auth/logout` | こことプロバイダーの両方でセッションを終了する |

プロバイダー側では、リダイレクト URI として `<public url>/_zeyn/auth/callback` を、ログアウト後のリダイレクト URI として `<public url>/` を登録します。公開 URL は設定で指定するもので、リクエストの `Host` ヘッダーから取得することはありません。ベースパスまたは公開パスがある場合、ルートはその下に移動します。

### フロー

1. サインアウト状態のユーザーが権限を必要とするページを開くと、`/_zeyn/auth/login` に移動します。
2. サーバーは新しい `state`、`nonce`、PKCE verifier (それぞれ 32 バイトの乱数) を作り、`return_to` のパスとともに 10 分間有効な cookie に封入して、`code_challenge_method=S256` を付けてプロバイダーの認可エンドポイントへリダイレクトします。
3. プロバイダーは code と `state` を付けてユーザーを戻します。サーバーは cookie を開き、`state` を定数時間で比較し、cookie を削除して、PKCE verifier を使って code を交換します (クライアントシークレットが設定されている場合は HTTP basic 認証で送ります)。
4. ID トークンの署名をプロバイダーの JWKS で検証し、続いて `iss`、`aud`、複数の audience がある場合は `azp`、`exp`、`nbf`、`iat`、`nonce` を検証します。
5. セッション cookie を設定し、ユーザーを `return_to` に戻します。受け付けるのはローカルパスのみで、それ以外はアプリのスタートページに移動します。

### セッション cookie

サーバー側にセッションストアはありません。セッションは `zeyn_session` という 1 つの cookie に保持されます。

- AES-256-GCM で暗号化および認証されます。鍵はセッション鍵 (`Config::secret`、32 バイト以上。これより短いとアプリは起動時に停止します) から導出されます。cookie 名が組み込まれているため、ある cookie 用に封入された値が別の cookie として開かれることはありません。1 バイトでも変更された cookie や、別の鍵で封入された cookie は、単にサインインしていない扱いになります。
- 保持する内容は、ユーザー、テナント、タイムゾーン、`ClaimMap` が対応付けたロール、直接付与された権限、サインイン時刻、第 2 要素を使ったかどうか、セッションの終了時刻、そして refresh token がある場合はその refresh token です。ID トークンとアクセストークンは含まれません。
- `HttpOnly`、`SameSite=Lax` (プロバイダーからの戻りはクロスサイトのナビゲーションであるため)、そして公開 URL が `https://` の場合 (または `App::secure_cookies(true)`) は `Secure` が付きます。
- 3800 バイトより大きくなるセッションは、サインイン時にログメッセージとともに拒否され、切り詰められることはありません。対応付けるロールを減らすか、refresh token をオフにしてください。
- 有効期間は `session_lifetime` です (設定しない場合は 8 時間)。セッション鍵を変更すると、全員がサインアウトされます。

CSRF 保護は変わりません。どの cookie でユーザーがサインインしていても、ランタイムのすべての `POST` には、`x-zeyn-csrf` ヘッダーに `zeyn_csrf` トークンが必要です。

### Refresh token

`refresh_tokens(true)` は refresh token を cookie に保持します。トークンの期限が切れると、次のリクエストが実行される前にトークンが更新され、更新された cookie が返されます。更新が拒否された場合 (ユーザーが無効化された、またはプロバイダーでサインアウトされた)、セッションは終了します。refresh token がない場合、プロバイダー側で何が起きてもセッションは `session_lifetime` の間続きます。更新はアプリ全体にかかる 1 つのレイヤーとして実行されるため、オプション機能のルートを含むすべてのルートが対象になります。

プロバイダーが refresh token をローテーションし、古いものをすぐに失効させる場合、同時に更新を行った 2 つのリクエストによってユーザーがサインアウトされることがあります。ローテーションをオフにするか、短時間の再利用を許可してください。

### ログアウト

`/_zeyn/auth/logout` はセッション cookie を削除し、`client_id` と `post_logout_redirect_uri` を付けて、ユーザーをプロバイダーの `end_session_endpoint` に移動させます。このエンドポイントがない場合、ユーザーはアプリのスタートページに移動します。

### 同じアプリの API と MCP クライアント

`OidcAuth::accept_bearer("stock-api")` は、同じ issuer が発行した、その audience 向けの Bearer アクセストークンも同じクレームマッピングで受け付けます。`Authorization: Bearer` ヘッダーを持つリクエストは、そのトークンだけで判断されます。

## 第 2 要素とステップアップ

`Identity::mfa` は、ユーザーが第 2 要素を証明した場合に true になります。

- `OidcAuth` と `JwtAuth` では、`amr` に `mfa`、`otp`、`hwk`、`swk`、`sc`、`sms`、生体認証、またはその他の第 2 要素 (RFC 8176) が含まれる場合、または `acr` が `ClaimMap::mfa_acr` のいずれかである場合。
- `Totp` では、コードの検証が成功した後。

`RequireMfa` がこれを要求します。

```rust
Feature::new("payroll").guard(RequireMfa::new())

App::new("Back office").interceptor(RequireMfa::new().screen("bank_accounts").action("approve_payment"))
```

feature のガードとして使うと、feature 全体が対象になります。インターセプターとして使うと、列挙した screen と action が対象になり、両方のリストが空の場合はすべての呼び出しが対象になります。

第 2 要素がない場合、ページの読み込みは `return_to` を付けてステップアップのアドレスへリダイレクトされます。
それ以外の呼び出しは、コード `mfa_required` の `401` を返します (API では problem type `urn:zeyn:problem:mfa_required`)。ランタイムは新しいタブでステップアップのページを開き、ユーザーが戻ってきたらリクエストを再実行します。これは `reauth_required` の場合と同じです。

`OidcAuth` では、ステップアップのアドレスは `/_zeyn/auth/login?mfa=1` です。これは `prompt=login` と `max_age=0` を付けて `acr_values` (`mfa_acr` の値) を送ります。新しい ID トークンにまだ第 2 要素が示されていない場合、callback はループせずにそれを拒否します。

- **Keycloak**: ブラウザーフローに認証レベルを設定し (レベル 2 の条件付き OTP または WebAuthn ステップ)、`mfa_acr(["2"])` を使います。Keycloak は `acr` に到達したレベルを返します。クライアントが名前をレベルに対応付けている場合 (`acr.loa.map`) は名前を返すので、その場合はその名前を列挙します。
- **Authentik**: フローがそれらのステージを使っている場合、Authentik はユーザーがどのようにサインインしたかを `amr` で報告します。アプリケーションには、authenticator validation ステージを含むフローを使ってください。

`OidcAuth` の下での `reauth` action は `/_zeyn/auth/login?reauth=1` に移動し、そこで `prompt=login` と `max_age=0` が送られるため、新しい `auth_time` は最新になります。

## 組み込みの TOTP

サインインに第 2 要素がないアプリ (MFA のないプロキシ、アプリ固有の authenticator) のための機能です。

```rust
use zeyn::{Totp, TotpStore};

App::new("Back office")
    .auth(ForwardAuth::new("x-forwarded-user"))
    .totp(Totp::new(TotpTable::new(pool), config.secret("auth.totp_key")?).issuer("Back office"))
```

- `/_zeyn/auth/totp` は、第 2 要素を持たないユーザーに登録画面を表示します。サーバー側で SVG として描画された QR コード (スクリプトも画像リクエストも不要)、手で入力するための鍵、コード入力欄です。シークレットは、コードによってアプリがそれを持っていることが証明されるまでの 10 分間、封入された cookie の中で待機します。
- 登録後、ユーザーには 10 個のリカバリーコードが 1 度だけ表示されます。ストアが保持するのはそのキー付きハッシュ (TOTP 鍵から導出した鍵による HMAC-SHA256) だけなので、ストアのコピーだけではオフラインで推測できません。各コードは 1 回だけ使えます。
- 保存されるシークレットは TOTP 鍵で封入され、ユーザーに結び付けられるため、ストアだけではコードを生成できず、レコードをユーザー間で入れ替えることもできません。
- 登録済みのユーザーは、ここでコードまたはリカバリーコードを使って検証します。コードは 1 回だけ受け付けられ (最後に使った時間ステップが保存されます)、前後 1 ステップの時計のずれが許容されます。
- 試行回数はユーザーごとに制限され (`attempts` で変更しない限り 5 分間に 5 回)、アプリのレートストアを通じて管理されます。複数のサーバーでカウントを共有するには `RedisStore` を使います。
- 検証されたコードは、封入された `zeyn_mfa` cookie を `lifetime` の間 (設定しない場合は 12 時間) 設定します。この cookie は identity に `mfa` を付け、`reauth` の action に対しては新しいサインインとして扱われます。
- 登録済みの要素は、セッションだけでは置き換えられません。リセットはストアに対する管理者の作業です。
- フォームは、`zeyn_csrf` トークンをフォームフィールドとして付けて、同一オリジンに送信されます。

`TotpStore` は 2 つの async メソッド `load(user)` と `save(user, record)` です。`MemoryTotpStore` はテスト用です。TOTP 鍵は変えないでください。別の鍵で封入されたレコードは開けなくなります。

## テスト

`crates/zeyn/tests/oidc.rs` は、テスト内の小さな issuer に対してすべてのフローを実行します。この issuer は実行用に作られた鍵で署名します。そして、期限切れのトークン、誤った audience と issuer、`alg=none`、公開鍵を鍵とした HMAC トークン、改ざんされたクレーム、改ざんされた cookie、再利用された、または一致しない `state`、誤った `nonce` が拒否されることを確認します。`crates/zeyn/tests/totp.rs` は、登録、検証、リプレイ、リカバリーコード、レート制限をカバーします。

開発モードの Keycloak があれば、同じフローを Keycloak に対しても実行できます。

```
docker run --rm -p 8080:8080 -e KC_BOOTSTRAP_ADMIN_USERNAME=admin -e KC_BOOTSTRAP_ADMIN_PASSWORD=admin quay.io/keycloak/keycloak start-dev
ZEYN_TEST_OIDC=http://127.0.0.1:8080 cargo test -p zeyn --features oidc --test oidc
```

テストは独自のクライアントとユーザーを持つ `zeyn-test` realm を作成し、終了時に削除します。
`ZEYN_TEST_OIDC` がない場合はスキップされます。
