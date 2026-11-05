# セキュリティ

Zeyn のセキュリティは、保護するコードの隣で宣言され、すべての呼び出しでサーバー上で検査されます。既定の設定は厳格で、それを緩めるには `app.rs` に目に見える 1 行を書く必要があります。

## 権限

すべてのアクションと画面は、`permission = "feature.resource.verb"` か `public` を宣言します。どちらもなければコンパイルできません。リクエストの ID は、その権限を持ちます。

```rust
Identity::user("ada").with_permissions(["stock.product.read", "stock.product.write"])
```

ユーザーが実行できないメニュー項目、ツールバーのボタン、行アクション、コマンドは、HTML から省かれます。省くのは利便性のためであり、サーバーはすべての呼び出しで権限を改めて検査します。

## サインイン

アプリはパスワードを保持しません。`Authenticator` が、呼び出し元が誰かを調べます。たいていは、アプリの前段にあるシングルサインオンのプロキシから判断します。

```rust
App::new(t!("app.name")).auth(
    ForwardAuth::new("x-forwarded-user")
        .groups("x-forwarded-groups")
        .require_header("x-proxy-secret", secret.expose())
        .role("stock-admins", ["stock.product.read", "stock.product.write"]),
)
```

`ForwardAuth` は、すべてのリクエストがプロキシを経由する場合にのみ使ってください。アプリに直接到達できる人は、これらのヘッダーを送れてしまいます。`require_header` は、プロキシが付加する共有シークレットを検査します。`DevAuth::superuser(name)` は、ローカル開発向けにすべてのリクエストへ完全なアクセスを与え、有効な間はサーバーが警告を出力します。リリースビルドはこれが有効だと起動を拒否します（`App::check` が報告します）。リリースで動かす必要があるデモは `App::allow_dev_auth()` を呼べます。`zeyn new` で作られたアプリは、デバッグビルドでのみこれを有効にします。

## ガード

ガードは、Feature のどの画面が開く前にも、そのグリッド、フォーム、コマンドが呼び出される前にも実行されます。

```rust
fn warehouse_open(ctx: &Ctx) -> GuardResult {
    if ctx.identity().permissions.contains("stock.warehouse.read") { GuardResult::Allow } else { GuardResult::Deny(t!("stock.closed")) }
}

Feature::new("stock").guard(SignedIn).guard(warehouse_open)
```

組み込みのガード: `SignedIn`、`TenantRequired`、`Licensed`。

## インターセプター

インターセプターは、アクセスの検査の後で、すべての画面、グリッド、フォーム、API、コマンドの呼び出しの前後で実行されます。`before` は呼び出しを止められ、`after` は呼び出しがどう終わったかを確認できます。`AuditLog(sink)` は、誰が何をしたかを記録する組み込みのものです。

## CSRF、CSP、ヘッダー

- すべてのページは、ランダムな `zeyn_csrf` Cookie（`HttpOnly`、`SameSite=Strict`）を設定し、同じトークンを meta タグに入れます。ランタイムからのすべての呼び出しは、それを `x-zeyn-csrf` ヘッダーで送り、サーバーは両者を定数時間で比較します。
- セッションなしで開かれた公開画面は例外です。Cookie を設定せず、トークンも持たないため、キャッシュできます。ランタイムは最初の呼び出しの前に `GET /_zeyn/csrf` からトークンを取得し、すべての `POST` は引き続き同じ方法で検査されます（[公開ページ](public-pages.md) を参照）。
- Content Security Policy は、スクリプト、スタイル、画像をアプリ自身のオリジンからのみ許可します。インラインスクリプトはなく、テーマはスタイルシートであり、インラインスタイルではありません。
- `App::csp` は、名前を指定したホストに対してポリシーを、ディレクティブごとに開放します。ヘッダーは起動時に一度だけ構築されます。

```rust
App::new("Shop").csp(|c| {
    c.img_src(["https://images.example.com"])
        .frame_src(["https://www.youtube-nocookie.com", "https://*.vimeo.com"])
        .media_src(["https://media.example.com:8443"])
        .media_blob()
        .connect_src(["https://api.example.com"])
        .font_src(["https://fonts.example.com"])
})
```

  受け付けられるのは `https://host`、`https://host:port`、`https://*.host` だけです（ワイルドカードはサブドメインに一致し、ホスト自身には一致しません）。`.media_blob()` は、ストリーミングライブラリ向けに `media-src` に `blob:` を追加します。`App::check` は、`*`、`http:`、`data:` や `https:` のようなスキーム単独の値、`'unsafe-inline'` や `'unsafe-eval'` のような引用符付きのキーワード、パス、ユーザー名、空白、カンマ、セミコロンを含む値を拒否し、それぞれの問題について代わりに何を書くべきかを示します。`script-src`、`object-src`、`base-uri`、`frame-ancestors` は変更できません。`Embed` と `Image` は同じリストを読むため、ブラウザがブロックするホストがレンダリングされることはありません（[表示](display.md) を参照）。
- 応答には `X-Content-Type-Options: nosniff`、`Referrer-Policy: same-origin`、`X-Frame-Options: DENY` が付きます。
- `App::secure_cookies(true)` は Cookie に `Secure` を付けます。HTTPS の背後では有効にしてください。

## エスケープ

HTML に入るユーザーデータは、常に `render::escape` を通ります。コンポーネントはレンダリングするすべての値をエスケープするため、`<script>` という名前の商品はテキストとして表示されます。

## 制限

```rust
App::new(t!("app.name"))
    .rate_limit(RateLimit::per_ip(600, Duration::from_secs(60)))
    .write_limit(RateLimit::per_user(60, Duration::from_secs(60)))
    .trusted_proxies(["10.0.0.0/8"])
    .allow_only_ips(["10.0.0.0/8", "192.168.0.0/16"])
    .deny_ips(["203.0.113.7"])
```

- `rate_limit` は、静的アセットと公開メディアソース（`Feature::public_media`）の読み取りを除くすべてのリクエストに適用されます。`write_limit` は、データを変更する呼び出しに対してより厳しい制限を加えます。フォームの送信、グリッドの編集、行、一括、メニューのアクション、コマンド、インポート、保存済みのグリッドビュー、ドックのレイアウトとテーマ、API の呼び出し、MCP の呼び出し、アシスタントです。拒否されたリクエストには `Retry-After` 付きの `429` が返ります。
- `X-Forwarded-For` は、`trusted_proxies` からのものだけが信用されます。ヘッダーのすべての行が順番に数えられ、採用されるアドレスは、信頼されたプロキシではない最後のものです。
- 表示（reveal）、アイランド、データソースの呼び出しも書き込みとして数えられ、エクスポートと印刷されたドキュメント（`GET /_zeyn/grid/export`、`GET /_zeyn/pdf`）も同様です。
- ページサイズ、行数、クエリの長さ、アップロードのサイズには、クライアントが何を送ってもサーバー側で上限が設けられます。

## セッション

`App::idle_timeout(d)` は、入力のない時間が続くと、最初に警告ダイアログを出してから、ユーザーをページからサインアウトさせます。`reauth` と指定されたアクションは、最近のサインインを必要とします（`App::reauth_within`）。

## データ保護

フィールドのマスキング、機密レコードを開く前の理由の入力、フィールド単位の監査、読み取りの監査は、[監査、マスキング、ジョブ](enterprise.md) で扱います。

## クライアント設定

小さな選択の中には、サーバーではなくブラウザーに置くべきものがあります。たとえば、埋め込みコンテンツを読み込む前にユーザーが確認した通知や、最後に開いたタブです。Zeyn はこれらを 1 か所、`zeyn.state`（`/_zeyn/state.js`。使うページでのみ読み込まれます）に保持し、サーバーはそれを読み取らないため、キャッシュされた公開ページは全員にとって同じままです。テーマや言語のように HTML を変える設定は、Cookie に残ります。

キーは、種類、有効期間、目的とともに定数として一度だけ宣言され、アプリに登録され、`.remember(KEY)` でコンポーネントに結び付けられます。`Embed::remember` は consent キーを、`Tabs::remember` は choice キーを受け取ります。単なる文字列や、もう一方の種類のキーはコンパイルできず、ASCII の英字、数字、`-`、`_` の 1 から 40 文字の範囲外の名前や、1 秒から 400 日（`MAX_PREF_LIFETIME`）の範囲外の有効期間は、定数を構築するときに失敗します。アプリが登録していないキーに結び付けられたコンポーネントは記憶されないため、プライバシーの一覧は常に完全です。

```rust
pub const VIDEO_NOTICE: ClientPref<Consent> =
    ClientPref::consent("video-notice", Duration::from_secs(86_400), Purpose::Necessary);
pub const ORDER_TAB: ClientPref<Choice> =
    ClientPref::choice("order-tab", Duration::from_secs(30 * 86_400), Purpose::Preference);

App::new(t!("app.name")).client_prefs(|p| {
    p.key(VIDEO_NOTICE)
        .key(ORDER_TAB)
        .allowed_when(|headers| consent_cookie(headers) == Some("all"))
})
```

- **レコード。** 各値は `zeyn.v1.{key}` の下に `{v, exp, val}` として保存されます。読み取りのたびに、バージョン、値（`true` または 200 文字以内のテキスト）、終了時刻を確認します。終了時刻は過ぎておらず、キーの有効期間より先であってはなりません。手で書き換えられたものなど、検査に失敗したレコードは削除されます。新しいバージョンの形式は、古いレコードを無視します。
- **目的。** `Purpose::Necessary` は、他のサイトのコンテンツを読み込む前に通知を確認するなど、ユーザーが求めたもののためのもので、その有効期間のあいだ保持されます。`Purpose::Preference` はインターフェースの選択です。ePrivacy 指令（第 5 条 3 項）は、ユーザーが求めたサービスに厳密に必要な場合に限り、同意なしにブラウザーへデータを保存することを認めており、EU のデータ保護当局（第 29 条作業部会、意見 04/2012）は、インターフェースの選択を、セッションの間または数時間に限って必要なものと見なします。そのため、設定値は、アプリのフック `allowed_when(|headers| ...)` がユーザーの同意を示すまで `sessionStorage` に置かれます。ブラウザーはページごとに一度 `GET /_zeyn/prefs` を呼び、その答えはキャッシュされず、フックがなければ答えは「いいえ」です。
- **タブとブロックされたストレージ。** 他のタブは `storage` イベントで変更に追従します。ストレージがブロックされている場合（プライベートウィンドウ、厳格な設定）、値はそのページのメモリ上に置かれます。JavaScript がなければ、コンポーネントはデフォルトの状態を表示します。
- **公開性。** `App::client_prefs_table()` は、プライバシーのページ向けに、すべてのキーを種類、目的、有効期間（秒）とともに一覧にし、`ClearPrefs`（または `zeyn.prefs.clear()`）はそれらをすべてブラウザーから削除します。

## API とエージェント

API の呼び出しには `Authorization` ヘッダーが必要です。ブラウザのセッション Cookie だけで API や MCP エンドポイントが呼び出されることはないため、CSRF トークンは不要です。エージェントは、代理するユーザーとまったく同じ権限を持ちます。[API と OpenAPI](api.md) と [MCP と AI エージェント](mcp.md) を参照してください。
