# 運用

本番環境での Zeyn アプリの運用について説明します。タイムゾーンデータ、署名付きライセンス、複数サーバーでの共有状態、ログとトレース、PDF ドキュメント、メール送信を扱います。

## タイムゾーン

`TimeZone::named` は、すべての IANA ゾーン (`Europe/Istanbul`、`America/New_York`、`Australia/Sydney`。大文字小文字は問いません) と、固定オフセット (`UTC+03:00`、`+05:30`) を受け付けます。

- ゾーンデータは、`jiff` を通じてバイナリに組み込まれた IANA タイムゾーンデータベース (約 200 KB) です。結果は Windows を含むすべてのプラットフォームで同じになり、システムの zoneinfo は読み取りません。
- 履歴もモデル化されています。トルコの 2016 年までの夏時間とそれ以降の恒久的な `+03`、2007 年以前のアメリカのルール、そのほか記録されているすべての変更です。
- 存在しない壁時計の時刻 (時計が進められた場合) は `LocalTimeError::Skipped` で拒否されます。2 回ある時刻 (時計が戻された場合) は、早い方の時点を意味します。
- ゾーンは、その名前が最初に使われたときにバンドルから読み込まれ、プロセスの存続期間中保持されます。1 つあたり数キロバイトです。未知の名前は保持されないため、クライアントがキャッシュを肥大化させることはできません。
- `TimeZone::names()` は、シェルのゾーンピッカー向けの、よく使われるゾーンの短い一覧です。一覧にないユーザーのゾーンは、そのユーザーのピッカーに追加されます。
- 新しいゾーンのルールは、新しい `jiff` のリリース (`cargo update -p jiff-tzdb`) で届きます。ある国がルールを変更したときは、再ビルドしてデプロイしてください。

## 署名付きライセンス

ライセンスファイルは、ベンダーだけが持つ ed25519 鍵で署名されます。アプリは公開鍵を埋め込むため、顧客はライセンスを読めますが、変更することも新しく作ることもできません。Cargo feature の `signed-licenses` で有効にします。

```
zeyn license keygen                      # writes zeyn-license.key, prints the public key
zeyn license sign acme.json --out license.json
```

`acme.json` にはライセンスのフィールドを記述します。

```json
{ "holder": "Acme", "valid_until": "2027-12-31", "modules": ["pos", "stock"], "max_users": 50 }
```

```rust
const LICENSE_KEY: &str = "f414a89e9dae57598d60141df96dd1c9fb655fdcf49f5eaf24e093fda7d332e3";

App::new("Shop")
    .license(SignedLicenseFile::new("license.json", LICENSE_KEY), Duration::from_secs(3600))
    .feature(Feature::new("pos").guard(Licensed::module("pos")).screen(pos))
```

- ファイルは JSON で、ライセンスがテキストとして `payload` に、署名が `signature` に入っています。署名の対象は `zeyn-license-v1\n` に続けてペイロードのテキストを連結したものなので、署名されるバイト列は保存されているバイト列そのものです。
- 秘密鍵は署名を行うマシンに置いたままにします。`keygen` は既存の鍵ファイルを上書きせず、新しいプロジェクトの `.gitignore` には `*.key` が入っています。`sign` は鍵を `--key`、`ZEYN_LICENSE_KEY` 変数、または `zeyn-license.key` から読み取り、`holder` のないライセンスや、`YYYY-MM-DD` 形式でない `valid_until` を持つライセンスは拒否します。
- 変更されたファイル、別の鍵で署名されたファイル、ライセンスではないファイルは拒否されます。`License::from_signed` は `LicenseError` (`BadSignature`、`InvalidPublicKey`、`Malformed`) を返し、サーバーは理由をログに記録し、すべての `Licensed` ガードは「Your license does not cover this.」で拒否します。読み取れないファイルは、他の失敗するライセンスソースと同様に、直前のライセンスを保持します。
- 有効期限は、ガードがリクエストごとに、ユーザーにとっての今日の日付と照らして検査します。そのため、期限切れのライセンスは再起動なしで「Your license has expired.」とともにアプリをロックします。更新されたファイルは、次回の更新時に読み込まれます。
- ed25519 では、変更されたファイルと別の鍵で署名されたファイルを区別できません。どちらも一致しない署名であり、メッセージもそのように伝えます。
- `max_users` は保持され署名されますが、フレームワークは強制しません。何をユーザーとして数えるかはアプリが決めます。

## 複数サーバー: Redis での共有状態

Cargo feature の `redis` を有効にすると、`RedisStore` が、ロードバランサーの背後にあるすべてのインスタンスで同一でなければならない状態を保持します。

```rust
let redis = RedisStore::new(config.require("redis.url")?, "billing").map_err(Error::Internal)?;
App::new("Billing")
    .rate_limit(RateLimit::per_ip(300, Duration::from_secs(60)))
    .rate_store(redis.clone())
    .datagrid_views(redis)
```

サーバーがユーザーごとに保持する状態と、その保存先は次のとおりです。

| 状態 | 保存先 | 共有 |
|---|---|---|
| レート制限のカウンター | `RateStore` | `RedisStore` |
| 保存されたグリッドビューとドックのレイアウト | `DataGridViewStore` | `RedisStore` |
| アイドルタイムアウト | ブラウザー (アプリのすべてのタブ) | サーバー側には何もない |
| サインインと再認証 | シングルサインオンのプロキシ | サーバー側には何もない |
| 機密性の高い Screen の理由 | `HttpOnly` クッキー | サーバー側には何もない |
| フォームの下書き | `sessionStorage` | サーバー側には何もない |
| ライセンスのキャッシュ | プロセスごと。ソースから再読み込みされる | 各サーバーが自分で読み込む |
| バックグラウンドジョブ | プロセスごと | 共有されない。`enterprise.md` のジョブのセクションを参照 |

- レート制限は 1 つの Lua スクリプトで実装されたトークンバケットなので、異なるサーバー上の同時リクエストが制限を超えることはありません。時刻はアプリサーバーではなく Redis から取得します。カウンターは最後の使用から 1 ウィンドウ後に失効します。
- ビューは `zeyn:<prefix>:view:<user length>:<user>:<grid>` の下に JSON として保存され、失効しません。
- 接続は最初の使用時に確立され、自動的に復旧します。呼び出しは 500 ms でタイムアウトします。Redis がダウンしている間、レート制限はリクエストを通し、ビューは読み込みも保存もされず (グリッドは自身のレイアウトに、ドックは `localStorage` にフォールバックします)、障害と復旧はそれぞれ 1 回だけログに記録されます。また、リクエストがそれぞれタイムアウトを待つことがないよう、失敗のたびに 5 秒間は Redis に接続しません。
- `RateStore::take` は future を返すため、ストアはネットワークと通信できます。`MemoryRateStore::take` は、直接使う場合のために同期のままです。
- TLS (`rediss://`) は組み込まれていません。
- `ZEYN_TEST_REDIS` が設定されている場合、テストは実際のサーバーに対して実行されます。

  ```
  docker run -d --rm --name zeyn-redis -p 6379:6379 redis:7-alpine
  ZEYN_TEST_REDIS=redis://127.0.0.1:6379 cargo test -p zeyn --features redis --test redis
  docker stop zeyn-redis
  ```

## ログとトレース

フレームワークは `tracing` を通じて報告します。自分で出力することはありません。

- `App::serve` と `App::serve_until` は、アプリが先にサブスクライバーをインストールしていない限り、サブスクライバーをインストールします。イベントはテキスト行として標準エラーに出力され、`log.level` (既定は `info`。`warn,zeyn=debug` のような `tracing` のターゲットフィルターも使えます) で絞り込まれます。JSON 行や別の出力先が必要なアプリは、serve の前に独自のサブスクライバーをインストールします。Zeyn はそれに手を触れません。
- `info` では、出力は従来どおりです。待ち受けアドレス、起動時の警告 (翻訳の欠落、`DevAuth`、`dev_latency`)、失敗するライセンスソース、内部エラーです。`debug` では、すべての呼び出しについて、種別、Screen、action、ユーザー ID、テナント ID、結果、エラーコード、ミリ秒を含む `call finished` 行が追加されます。
- トレースバックエンド向けのスパン:

  | スパン | レベル | フィールド |
  |---|---|---|
  | `request` | info | メソッド、パス (クエリ文字列なし)、ステータス、ms。判明した時点で、種別、Screen、action、ユーザー、テナント、ok、エラー |
  | `run` | info | アプリケーションコード: action、データソース (グリッドの取得、行の参照、ビューストア)、Screen ビルダー |
  | `interceptor` | debug | 名前 (型名、または `Interceptor::name`)、ステージ (`before` または `after`) |

- Cargo feature の `otel` を有効にし、`Config` に `otel.endpoint` (`http://collector:4318`。スパンは `/v1/traces` に送られます) を設定すると、スパンが OTLP/HTTP でエクスポートされます。`otel.service_name` はサービス名を指定し、既定ではアプリの名前です。スパンはバックグラウンドスレッドでバッチ処理され、`serve_until` の停止時にフラッシュされます。
- エクスポートしない場合、標準エラーにはイベントだけが出力されるため、スパンは構築されず、スパンのコストはそれぞれキャッシュされたチェック 1 回だけです。別のサーバーの中で `App::router` を使ってマウントされたアプリや、テストでも同じです。
- 記録されないもの: フィールドの値 (マスク済みかどうかを問わず)、フォームと API の入力、クエリ文字列、クッキー、ヘッダー、シークレット。テストはトレースレベルのサブスクライバーの下で、グリッド、フォームの保存、API 呼び出し、リビールを実行し、マスクされた値が現れないことを検査します。アプリ側に残る点が 2 つあります。パスは記録されるため、ルートに個人データを含めないでください。また、`Error::Internal` のメッセージは書かれたとおりにログに記録されます。

## PDF ドキュメント

Cargo feature の `pdf` は、[Typst](https://typst.app) テンプレートから PDF ドキュメントを生成します。Typst は大きい (下の数値を参照) ため、既定ではオフです。

```rust
const INVOICE: PdfTemplate = PdfTemplate::new("invoice", include_str!("../templates/invoice.typ"));

#[action(permission = "sales.order.read")]
pub async fn invoice_data(p: OrderParams, store: Orders, ctx: Ctx) -> Result<InvoiceData> { ... }

Screen::window("Order").body(PdfLink::new("Print invoice", INVOICE, invoice_data, params))

App::new("Shop").service(Pdf::from_config(&config)?)
```

- `PdfLink` は Screen にダウンロードボタンを配置します。`GET /_zeyn/pdf` は、アクセス検査を行ったうえでその Screen を開き、ユーザーとして action を実行し (その action のパーミッションとインターセプターが適用され、呼び出しの種別は `Document` です)、action が返した内容でテンプレートを描画します。その action を実行できないユーザーには、ボタンは表示されません。
- `exportable()` を持つグリッドは、ツールバーの `Export::Pdf` (`GET /_zeyn/grid/export?format=pdf`) から一覧レポートも印刷できます。ユーザーの列、ラベル、フィルター、ソートが反映され、数値は右揃え、ヘッダー行は各ページで繰り返され、ページ番号が付きます。レポートに含まれる行は最大 2,000 行 (`MAX_PDF_ROWS`。グリッドのエクスポート上限の方が小さい場合はその上限) で、行が省かれた場合はその旨が表示されます。
- **データが Typst コードになることはありません。** action の出力は JSON にシリアライズされ、テンプレートはそれを `data` として読み取ります (レンダラーはテンプレートの前に `#let data = json("/data.json")` をバインドします)。`#panic()`、`]`、`$x$` のような値はテキストとして出力されます。これはテストで検査されています。テンプレートはそれ以外のものを一切読み取れません。ファイルもパッケージも使えません。
- 描画はブロッキングスレッドで実行されます。同時に描画されるドキュメントは最大 `pdf.max_concurrent` 件 (1 から 64。既定は 2、CPU が 1 つの場合は 1) で、残りはスロットが空くのを待ちます。Typst のメモ化キャッシュはドキュメントごとに空になるため、メモリは元の水準に戻ります。

| キー | 意味 |
|---|---|
| `pdf.fonts` | 追加の `.ttf`、`.otf`、`.ttc`、`.otc` フォントのディレクトリ。起動時に一度だけ読み込まれる |
| `pdf.max_concurrent` | 同時に描画されるドキュメントの数 |

フォント: Libertinus Serif (標準、太字、斜体、太字斜体。SIL Open Font License) が組み込まれており、トルコ語に対応しています。`₺` のように Libertinus にない記号のフォールバックとして、DejaVu Sans Mono が組み込まれています。ライセンスは `crates/zeyn/assets/fonts` にあります。数式用フォントはありません。テンプレートで数式を使う場合は、`pdf.fonts` で追加してください。

Windows (リリースビルド、x86_64) での測定結果:

| | 結果 |
|---|---|
| バイナリサイズ | `pdf` なしで 5.5 MB、ありで約 46 MB (Typst が約 39 MB、フォントが 1.6 MB 増える) |
| 1 ページのドキュメント | 4 から 7 ms、描画中のヒープは 1 MB 未満、ファイルは 10 KB |
| 1,000 行 (25 ページ) の一覧レポート | 0.54 から 0.74 秒、描画中のピークヒープは 51 MB、描画後に保持されるのは 0、ファイルは 0.87 MB |
| 起動後の最初のドキュメント | Typst 自身が遅延構築するテーブルのために、約 42 MB のヒープが残る |
| フォント | フォントインデックスのヒープは 0.25 MB |

デバッグビルドは約 10 倍遅くなります (showcase で 2,000 行のレポートに 71 秒かかりました)。

## メール

Cargo feature の `email` は、[lettre](https://lettre.rs) で通知を送信します。

```rust
let mailer = Mailer::from_config(&config)?;
App::new("People").mailer(mailer)

#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(r: TransitionRequest<LeaveRequestId>, store: Leaves, mailer: Mailer, ctx: Ctx) -> Result<LeaveState> {
    let state = leave_flow().apply(&ctx, current, &r.transition, r.comment.as_deref())?;
    mailer.send(&ctx, Mail::transition(&ctx, &manager_email, &employee)?)?;
    Ok(state)
}
```

- `Mailer::send` はメッセージを描画してメモリ内のキュー (最大 10,000 件) に入れ、すぐに戻ります。バックグラウンドジョブ `zeyn.mail` が、キューをすぐに送信し、失敗したメッセージをバックオフを付けて再試行し (`Mailer::retry` で指定しない限り、10 秒から始めて 5 回)、1 分ごとにキューを確認します。8 回失敗したメッセージは、ログにエラーを記録して破棄されます。すべてのジョブと同様に、プロセスの停止時にまだ送信されていないキューは失われます。耐久性のある配信には、データレイヤーのデータベースを使ったキューが必要です。
- メッセージは i18n カタログから作られます。`<template>.subject`、`<template>.text`、必要に応じて `<template>.html` を使い、`{name}` 形式の引数を取ります。受信者の言語は、`Mail::locale` で指定しない限り送信者の言語です。すべてのメッセージにはプレーンテキストと HTML の両方のパートがあります。HTML パートではすべての引数がエスケープされ、`.html` のエントリがない場合、HTML はエスケープされたテキストになります。件名からは改行が取り除かれ、アドレスは解析されるため、データによってヘッダーや受信者を追加することはできません。
- `Mail::transition` は、直前の呼び出しが `Workflow::apply` で行った遷移 (レコード、遷移前、遷移後、実行者、コメント) を、組み込みの `zeyn.mail.transition` テンプレートで書き出します。showcase では、休暇申請の状態が変わったときにマネージャーに通知します。

| キー | 意味 |
|---|---|
| `mail.from` | 送信者。`Northwind <no-reply@example.com>` のような形式 (必須) |
| `mail.transport` | `smtp`、`file` (メッセージごとに `.eml` ファイルを作る。開発とテスト向け)、`log` (受信者の数とサイズを含むログ行。アドレスや内容は決して含まない。既定) |
| `mail.smtp.host`, `mail.smtp.port` | SMTP サーバー |
| `mail.smtp.tls` | `starttls` (既定)、`tls` (暗黙の TLS。通常はポート 465)、`none` (ローカルのリレー専用) |
| `mail.smtp.user`, `mail.smtp.password` | 認証情報。パスワードは環境変数などのシークレットソースから読み取ってください |
| `mail.file.dir` | `file` トランスポートのディレクトリ |

TLS は、Mozilla のルート証明書 (`webpki-roots`) とともに rustls を使用します。
