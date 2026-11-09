# エンタープライズ機能

業務アプリ向けの横断的な機能です。データ監査、個人データの保護、安全なデプロイ、ヘルスチェック、セッションの安全性、CSV と XLSX のインポート、バックグラウンドジョブ、承認ワークフローを扱います。すべてサーバー側で検査され、ブラウザーのランタイムは使い心地を良くするだけです。KVKK と GDPR を念頭に置いたプライバシー法として意識していますが、ここにある機能に特定の国や業種に固有のものはありません。

## フィールド単位のデータ監査

`AuditLog` は、フォームの送信ごと、および `audit` を付けた action の呼び出しごとに 1 つの `AuditEntry` を書き込みます。保存の場合、エントリにはレコードと、変更されたすべてのフィールドも記録されます。

```json
{"kind":"form","screen":"customer","action":"save_customer","ok":true,"user":"ayse",
 "model":"Customer","record":"2","changes":[{"field":"name","old":"Customer 2","new":"Renamed"}]}
```

- フォームは自動でこれを行います。古い値は screen がサーバー側で読み込んだレコードから、新しい値は検証済みの `Valid<M>` から取得します。クライアントから送られた値が古い値として使われることはなく、フォームがクライアントから受け取らないフィールドが変更されたものとして現れることもありません。
- API の保存では、`Valid<M>` の入力からモデルとキーを記録します。データベースの内容を知っているのは action だけなので、差分は action 自身が報告します。

  ```rust
  #[action(permission = "crm.customer.write", audit)]
  pub async fn save_customer(c: Valid<Customer>, store: Customers, ctx: Ctx) -> Result<Customer> {
      let current = store.get(c.id)?;
      ctx.record_changes(&current, &c);
      store.save(c.into_inner())
  }
  ```

  フォームの保存 action で同じことをしても問題ありません。その場合、action の報告がフォームの報告に置き換わります。
- 値は、言語に依存しない平文の文字列です (`1234.56 TRY`、`2024-05-01`、choice の値)。行バージョンは含まれません。マスクされたフィールドはマスクされたままです (下を参照)。
- 保存に失敗した場合も、変更なしでレコードが記録されます。

## フィールドのマスキング

個人データを保持するテキストフィールドには `#[mask]` を付けます。次の例では、国民 ID は `•••• 8901`、メールは `p••••@example.com`、メモは `••••` と表示されます。

```rust
#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    pub name: String,
    #[mask(last4)]
    pub national_id: String,
    #[mask(email, permission = "hr.contact.unmask")]
    pub email: Option<String>,
    #[mask]
    pub notes: Option<String>,
}
```

- マスキングはサーバー側で行われ、grid のセル、フォーム、API 出力、エージェントツール、そしてセルから作られる CSV エクスポートにも適用されます。権限がない限り、生の値がブラウザーや API クライアントに届くことはありません。
- マスク解除の権限は、属性で指定しない限り、スネークケースの `<model>.<field>.unmask` です (`person.national_id.unmask`)。
- 権限を持つユーザーにも、最初はマスクされたテキストが表示され、表示ボタンがあります。表示すると `POST /_zeyn/reveal` が呼ばれ、権限が再度検査されて、監査レコード (kind は `reveal`、フィールドは `revealed`) が書き込まれます。拒否された試行も監査されます。
- 権限を持つ API クライアントは生の値を受け取ります。そのようなフィールドはすべて、呼び出しの監査レコードの `revealed` に列挙されます。
- フォームはマスクされたフィールドを読み取り専用のテキストとして表示します。保存時には、ユーザーが表示でき、かつ表示した後に変更した場合を除き、保存済みの値が保持されます。空のマスクフィールドは通常の入力欄なので、データ入力担当者は一度入力できますが、後からそれを読み戻すことはできません。
- マスクされたフィールドに対するフィルターと並べ替えは、それを表示できないユーザーに対しては無視されるため、検索によって値を推測されることはありません。これは grid のすべての読み取り (ページ、エクスポート、全行に対する一括 action、保存済みビュー、エージェントツール)、および `DataQuery` を受け取る API の action に適用されます。マスクされた列に `.filter()` を追加しないでください。
- ページングのカーソルには、マスクされたフィールドの値が含まれません。マスクされたフィールドで並べ替えたリストは、キーではなく位置でページングされます。
- grid はマスクされたフィールドでグループ化しません。そのフィールドへの `group_by` は無視され、ログに記録されます。
- island やその他のデータソースの出力は、API 出力と同様にどの深さでもマスクされます。内部にあるすべてのモデルのマスクされたフィールドはマスクされたテキストとして出力され、機微なモデルのキーは読み取りとして記録されます。
- grid の reveal は、キーに対するフィルターを付けて grid の action を通して行を検索するため、その action はキーフィールドに対するフィルターを尊重しなければなりません。
- `#[mask]` は `String` と `Option<String>` にのみコンパイルできます。

## 機微なレコードの読み取り

誰がどの個人データをいつ見たかが、誰がそれを変更したかと同じように記録されます。

- `#[entity(table = "cases", sensitive)]` (または `#[entity]` の下に `#[sensitive]`) でモデルに印を付けます。ユーザーにそのようなレコードが表示されるたびに、呼び出しの監査レコードの `reads` にそれが列挙されます (`{"model":"Case","id":"2"}`)。対象は grid ページの行、フォーム、エージェントツールの結果、API 出力です。読み取りはデータが描画される場所で記録されるため、レイアウト内のコンポーネントも対象になります。
- `#[screen(..., sensitive)]` は、screen を開くたびに、`model` が screen 名で `id` がそのパラメーターである読み取りとして記録します。1 つのモデルに結び付かない個人データを表示する screen のためのものです。
- `#[screen(..., reason)]` は、データを表示する前に、その理由もユーザーに尋ねます。理由が与えられるまで、サーバーはそのページ、フラグメント、grid、フォーム、reveal の呼び出しに対して、コード `reason_required` の `428` を返します (フルページの読み込みでは理由入力フォームを持つページ)。ランタイムはダイアログで尋ね、`POST /_zeyn/reason {screen, reason}` (3 から 300 文字) を送って、リクエストを再実行します。理由は `HttpOnly`、`SameSite=Strict` の cookie に 15 分間保持され、与えられた screen だけを開き、その screen のすべての監査レコードの `reason` に書き込まれます。エージェントと API クライアントは理由を与えられないため、そのような screen を開けません。
- 機微なモデルに対する grid のエクスポート (CSV、XLSX、PDF) は、ファイルごとに 1 つの監査レコードを書き込み、`export` にモデル、形式、行数、要求された並べ替えとフィルターを設定します。`reads` にすべての行は列挙されないため、大きなエクスポートでもコストは小さく、他のモデルのエクスポートに追加のコストはかかりません。`sensitive` または `reason` の screen からエクスポートした場合は、screen の読み取りも記録されます。
- screen の遅延部分 (遅延タブが使う `POST /_zeyn/node`) は、screen を開くのとまったく同じようにインターセプターを実行し、読み取りを記録します。
- `ctx.record_read(&record)` は、アプリケーションコードから読み取りを記録します。
- 読み取りレコードは、機微なレコードが表示された時点で、ページの読み込みを含むあらゆる呼び出しについて `AuditLog` が書き込みます。監査のインターセプターがなければ何も記録されません。個人データを扱うアプリは、必ず 1 つ登録してください。

## 承認ワークフロー

ワークフローは、型付けされた小さなステートマシンです。状態は `#[choice]` の enum で、各遷移は開始状態、終了状態、必要な権限を指定します。

```rust
#[choice]
pub enum LeaveState { Draft, Submitted, Approved, Rejected }

pub fn leave_flow() -> Workflow<LeaveState> {
    Workflow::new()
        .transition("submit", LeaveState::Draft, LeaveState::Submitted, "people.leave.request")
        .transition("approve", LeaveState::Submitted, LeaveState::Approved, "people.leave.approve")
        .transition("reject", LeaveState::Submitted, LeaveState::Rejected, "people.leave.approve")
}

#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(req: TransitionRequest<LeaveRequestId>, store: LeaveStore, ctx: Ctx) -> Result<LeaveState> {
    let mut leave = store.get(req.id)?;
    leave.state = leave_flow().apply(&ctx, leave.state, &req.transition, req.comment.as_deref())?;
    store.save(leave)
}
```

- `apply` はサーバー側で、ストアから読み取った状態を使って実行されます。未知の遷移は `404`、レコードがすでに開始状態にない場合 (誰かが先に動かした) は `409`、権限がない場合は `403`、1000 文字を超えるコメントは `422` です。
- 各遷移は、監査レコードの `transition` に名前、from、to、コメントとして書き込まれます。ユーザーと時刻はレコード自体にあります。
- `Transitions::of(flow, state, id, action)` は、現在の状態、任意のコメント、そして現在の状態からユーザーが行える遷移ごとに 1 つのボタン (`Workflow::allowed`) を表示します。レコードのキーはサーバー側で screen から取得されるため、クライアントが別のレコードを動かすことはできません。エージェントからはフォームツールとして見えます。
- ボタンのラベルはカタログの `workflow.<name>` から取得されます。`.label(...)` で手動設定もできます。
- `.confirm(name, message)` は遷移 `name` の実行前に `ActionButton::confirm` のダイアログで確認し、`.tone(name, Tone::Danger)` はそのボタンを赤で描きます。どちらの場合もサーバーが遷移と権限を確認します。
- `Workflow::check` は、無効な権限と、同じ状態から 2 回宣言された遷移を報告します。テストで呼び出してください。
- showcase の休暇申請はこれを使っています。下書きとして保存し、申請して、承認または却下します。

## 安全なデプロイ

- すべてのレスポンスに `x-zeyn-build` が付き、すべてのページに `<meta name="zeyn-build">` が付きます。この ID は、設定されていれば `ZEYN_BUILD_ID` (英字、数字、`.`、`_`、`-`、最大 64 文字)、なければ実行ファイルのハッシュです。`zeyn::server::build_id()` がこれを返すので、ログに使えます。
- ランタイムが別のビルドからのレスポンスを検出すると、丁寧な通知を表示します。新しいバージョンが利用可能であり、準備ができたら再読み込みしてください、という内容です。自動で再読み込みすることはありません。
- フォームにユーザーが入力した内容は、screen、パラメーター、フォームごとに `sessionStorage` に保持され、再読み込み後に、通知と破棄ボタンとともに復元されます。マスクされた入力、パスワード、ファイル、hidden の入力は保持されません。下書きは入力された時点の行バージョンを保持するため、その間に誰かが変更したレコードの下書きを保存しようとすると、競合として拒否されます。保存に成功すると下書きは消去され、1 日より古い下書きは破棄されます。`sessionStorage` はそのブラウザーのタブの中にしか存在しません。

## ヘルスチェック

- `GET /_zeyn/health` は、プロセスが動いている間 `200 ok` を返します。liveness に使います。
- `GET /_zeyn/ready` は、`App::readiness(name, check)` で追加したすべてのチェックを、1 つあたり 5 秒で同時に実行し、`200 {"status":"ready"}` または `503 {"status":"not_ready","failed":["database"]}` を返します。表示されるのはチェック名だけで、エラーは表示されません。
- どちらも `base_path` の下に置かれ (`/billing/_zeyn/health`)、`Cache-Control: no-store` で応答し、サインインは不要です。
- readiness の結果は 1 秒間再利用され (`READY_CACHE`)、チェックを実行するのは同時に 1 つのリクエストだけなので、大量のプローブがデータベースに負荷をかけることはありません。
- ヘルスはレート制限されないため、忙しいプローブが自分自身を締め出して健全なアプリを再起動させることはありません。readiness は、`probe_ips` のアドレスからのものでない限り、レート制限にカウントされます。
- IP ルールは引き続き適用されます。`deny_ips` は常に、`allow_only_ips` もプローブのアドレスが `App::probe_ips([...])` に列挙されていない限り適用されます。`App::probe_ips([...])` は、そのアドレスに対してこの 2 つのエンドポイントだけを開放します。

## 開発時のレイテンシ

`App::dev_latency(Duration::from_millis(400))` は、すべての呼び出し (ページ、フラグメント、grid、フォーム、API、MCP、ランタイムの呼び出し。静的アセットとヘルスプローブは対象外) を遅延させ、低速なネットワークのユーザーと同じように、開発者がローディング状態とダブルクリックを確認できるようにします。これはデバッグビルドでのみ動作します。リリースビルドではこのメソッドは何もせず、遅延のコードはコンパイルされないため、誤って残してしまっても本番を遅くすることはありません。有効な間、起動時に警告が出力されます。

## セッションの安全性

サインインは、シングルサインオンのプロキシ、組み込みの OpenID Connect サインイン、または Bearer トークンから行われます (`docs/auth.md` を参照)。したがってこれらの機能は、authenticator がユーザーが誰であるかを伝えること、そして再認証の場合はいつサインインしたかを伝えること (`Identity::authenticated_at`。`ForwardAuth::auth_time("x-auth-time")` は Unix 秒または RFC 3339 を読み取ります。`DevAuth` はすべてのリクエストを、サインインしたばかりとして報告します) だけを必要とします。

- `App::idle_timeout(Duration)` は、終了の `idle_warning` 前 (デフォルトは 1 分) に、カウントダウンと「サインインを維持する」ボタンを持つアラートダイアログでユーザーに警告します。このボタンは `POST /_zeyn/keepalive` を呼び出すので、前段のプロキシにも操作があったことが伝わります。アプリのどのタブでの入力もカウントされます。終了時、ユーザーは `logout_url` に移動し、なければ `return_to` を付けて `login_url` に移動し、それもなければサインアウト画面が表示されます。フォームの下書きは保持されます。アイドルタイムアウトは、放置された画面を閉じるだけです。実際のセッションを終了させるのは、アイデンティティプロバイダーの役割です。
- `#[action(permission = "...", reauth)]` は、`App::reauth_within` (デフォルトは 5 分) 以内のサインインを必要とします。そうでない場合、クライアントが何を送っても、サーバーはコード `reauth_required` の `401` を返します (API では problem type `urn:zeyn:problem:reauth_required`)。ランタイムは新しいタブでユーザーに再度のサインインを求め (`reauth_url`、なければ `login_url`)、ユーザーが戻ってきたらリクエストを再実行するため、入力した内容は失われません。サインイン時刻を持たない identity は決して通過できません。

## CSV インポート

```rust
#[action(permission = "people.leave.import", audit)]
pub async fn import_leaves(rows: Vec<Valid<LeaveRequest>>, store: LeaveStore) -> Result<usize> { ... }

Screen::new("Import").body(Import::of(import_leaves).max_rows(1000))
```

- ユーザーが CSV ファイルを選びます。ランタイムはそれを UTF-8 として読み取り、有効な UTF-8 でない場合は Windows-1254 として読み取り (トルコ語のスプレッドシート)、テキストを JSON ボディで `POST /_zeyn/import` に送ります (multipart は不要で、最大 8 MB)。サーバーは、サマリー、最初の 10 行のプレビュー、行ごとのエラー、ダウンロード可能なエラーレポートを返します。
- パース: バイトオーダーマークは取り除かれ、`,`、`;`、タブはヘッダー行から検出され、区切り文字や改行を含む `"` によるクォートに対応し、`\r\n` と `\n` の両方を扱います。
- 上限は、ファイルを読み取った後ではなく、読み取りの最中に検査されます。行数の上限で読み取りが止まり、512 セルを超えるヘッダーは拒否され、データ行はヘッダーより多くても 1 セルまでしか保持されません (そのため余分な値も報告されます)。XLSX ファイルは、ヘッダーにある列数までしか列を読み取らず、そのヘッダーに指定できるのは、モデルがインポートできる列数までで、16 MB を超える shared strings パートは、何かを展開する前に拒否されます。
- 列は、フィールド名またはアプリのいずれかの言語のラベルで、大文字小文字とトルコ語のドット付き i とドットなし i を区別せずに対応付けられます。未知の列、読み取り専用の列、キー列、行バージョンの列は拒否されます。
- すべての行は、そのモデルのフォームとまったく同じように読み取られ、検証されます。その言語のユーザーが入力するとおりの値 (`1.234,56`、`05.12.2026`)、フィールドのルール、モデル自身の `validate` が使われます。
- インポートは、有効な行だけを action に送ります。`.all_or_nothing()` を使うと、すべての行が有効でない限り何もインポートされません。インポート時、サーバーはファイルを再度読み取って検証します。プレビューの内容は一切信頼されません。どちらのステップも action の権限を必要とし、インポートは監査されます。
- `.max_rows(n)` は行数を制限します (デフォルトは 5000、50000 を超えることはありません)。エラーレポートはスプレッドシートの数式を無効化します。マスクされたフィールドは、プレビューでマスクされたまま表示されます。

### XLSX インポート

`xlsx` Cargo feature を使うと、同じコンポーネントが Excel ブック (`.xlsx`) も受け付けます。ランタイムはファイルを base64 で送り (同じ JSON ボディで `"csv"` の代わりに `"xlsx"`)、サーバーは `calamine` で最初のワークシートを読み取り、CSV と同じ行ごとの検証、プレビュー、エラーレポートに渡します。

- セルは型を保持します。ユーザーの言語に関係なく、数値は数値として、日付セルは日付として読み取られるため、トルコ語のセッションでも `1234.5` は 1.234,50 のままです。日付セルの時刻 (壁時計の時刻) は、フォームが読み取る場合と同じように、ユーザーのタイムゾーンで読み取られます。choice は値またはラベルで一致します。
- 上限: ファイルは最大 8 MB で、アーカイブは最大 1000 パートで最大 64 MB まで展開できます。これは、何かを展開する前に、アーカイブが宣言するサイズから検査されます。セルは 1 つずつ読み取られ、行数の上限で読み取りが止まるため、100 万行のシートでも、上限を超えるコストはかかりません。ブックでないファイル、パスワード保護されたファイル、途中で切れたファイルには、分かりやすいエラーが返されます。
- 読み取られるのは最初のワークシートだけです。数式はキャッシュされた値として扱われます。

## バックグラウンドジョブ

```rust
App::new("Billing")
    .job_concurrency(2)
    .job("send_reminders", Schedule::every(Duration::from_secs(3600)).retry(3, Duration::from_secs(30)), send_reminders)
    .job("rebuild_index", Schedule::manual(), rebuild_index)
    .jobs_permission("ops.jobs.read")
    .serve_until("0.0.0.0:3000", shutdown_signal())
```

- ジョブは `async fn(Ctx) -> Result<()>` です。そのコンテキストには、アプリのサービス、設定、デフォルト言語と、権限を持たない identity `job:<name>` があります。
- `Schedule::every(d)` は間隔ごとに実行され、`Schedule::manual()` はトリガーされたときだけ実行されます。action は、注入可能な `Jobs` サービスを通じてジョブをトリガーします: `jobs.trigger("rebuild_index")`。実行中にトリガーされたジョブは、その後にもう 1 回実行されます。ジョブが同時に 2 つ実行されることはありません。
- `.retry(times, first_delay)` は、失敗した実行を再試行し、待ち時間を 2 倍にしながら最大 10 分まで延ばします。
- 同時に実行されるジョブは最大 `job_concurrency` 個です (デフォルトは 4)。
- ステータス (状態、実行回数、失敗回数、最後の開始と終了、最後のエラーコード) は、コードでは `jobs.status()`、`jobs_permission` を持つユーザーには `GET /_zeyn/jobs` で取得できます。エラーの詳細はサーバーログにのみ出力されます。
- `App::serve_until(addr, signal)` は、シグナルが完了するとリクエストの受け付けを停止し、実行中のリクエストを完了させ、再試行の待機をキャンセルして、実行中のジョブに 30 秒を与えます。
- ストアがない場合、ジョブはメモリ上にあります。プロセスが停止した時点で待機中または実行中の作業は失われ、複数のサーバーがある場合は、それぞれが独自のスケジュールを実行します。

### 永続ジョブ

```rust
App::new("Billing")
    .job_store(SqlJobStore::new(db))
    .queued_job("send_invoice", Schedule::manual().retry(5, Duration::from_secs(10)), send_invoice)
    .job("close_day", Schedule::every(Duration::from_secs(86_400)), close_day)

async fn send_invoice(ctx: Ctx, invoice_id: String) -> Result<()> { ... }
jobs.enqueue("send_invoice", "42").await?;
```

- `JobStore` は実行を保持します: `enqueue`、`schedule`、リース (ワーカー ID と終了時刻) 付きの `claim`、`heartbeat`、`complete`、`fail`。デフォルトは `MemoryJobStore` (最大 10,000 件の実行で、最も古い失敗した実行から破棄されます) です。`SqlJobStore` (feature `postgres`、`oracle`) は、実行を `zeyn_jobs` に保持します。`create_job_tables(dialect, prefix)` は、マイグレーション用のステートメントを返します。
- ストアを設定すると、`Schedule::every` のジョブは次の実行をストアに保持するため、再起動しても実行時刻は動かず、各間隔でサーバー 1 台だけが実行します。`trigger` もそこに実行をキューします。
- `queued_job` のジョブは、各実行のペイロードを受け取ります (最大 `MAX_JOB_PAYLOAD`、4000 バイト)。通常のジョブもキューでき、その場合ペイロードは無視されます。`enqueue_after` は実行を遅らせます。
- claim は、このサーバーが知っているジョブの期限が来た実行を、古い順に取得し、`JOB_LEASE` (60 秒) の間保持して、実行が動いている間は 20 秒ごとに更新します。`complete` と `fail` は、ワーカーがまだそれを保持している間だけ実行を変更します。ワーカーが停止した実行は、リースが終了すると再び claim されます。失われた試行はカウントされるため、サーバーをクラッシュさせる実行は最終的に failed になります。
- 失敗した試行は、試行回数が残っている間、バックオフ (2 倍にしながら最大 `MAX_BACKOFF_MS`、10 分) を待ち、その後はエラーコードとともに `failed` として残ります。スケジュールは次の間隔に進みます。
- ワーカーは `JOB_POLL` (1 秒) ごとに、また同じサーバーで `enqueue` された直後に、期限が来た実行を探します。各 claim が取得するのは空きジョブスロットの数までなので、メモリがキューとともに増えることはありません。
- PostgreSQL は 1 つの `UPDATE ... WHERE id IN (SELECT ... FOR UPDATE SKIP LOCKED) RETURNING` で claim します。Oracle 11g 以降は、トランザクション内で `ROWNUM` と `FOR UPDATE SKIP LOCKED` で ID を選択し、更新して、コミットの前に読み戻します。Oracle ではロックを伴う select を並べ替えられないため、そこでは期限が来た実行が決まった順序なしで取得されます。時刻は、`BIGINT` または `NUMBER(19)` 列の Unix ミリ秒です。
- ストアがある場合、同じジョブのスケジュールされた実行とトリガーされた実行が重なることがあります。同時に 2 つ実行してはいけないジョブは、自分で保護してください (たとえば行ロック)。
