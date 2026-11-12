# フォーム

`Form::of(value)` は、モデルまたはエンティティを編集します。フィールドはフィールド参照から、ラベルはロケールファイルから、検証は `#[rule]` 属性から得られるため、ブラウザとサーバーは同じルールを検査します。

## フォーム

```rust
Form::of(Contact { quantity: 1, ..Contact::default() })
    .row(|r| r.field(Contact::name).field(Contact::email))
    .row(|r| r.field(Contact::quantity).field(Contact::size))
    .submit(t!("docs.live.send"), send_contact)
    .on_success(|_, ui| ui.toast(t!("docs.live.sent")))
```

@live form

名前を空のままにするか、誤ったメールアドレスを入力して *送信* を押してみてください。ブラウザはすぐにエラーを表示し、サーバーも同じ入力を同じメッセージで拒否します。

## レイアウト

| 呼び出し | 結果 |
|---|---|
| `.field(M::f)` | 1 つのフィールドを 1 行に置きます |
| `.row(\|r\| r.field(a).field(b))` | フィールドを横に並べます。狭い画面では縦に積まれます |
| `.section(title)` | 後続のフィールドをタイトル付きでグループにします |
| `.step(title)` | ウィザードのステップ。フォームは一度に 1 ステップずつ表示します |
| `.range(M::from, M::to)` | 2 つの日付を 1 つの範囲ピッカーにします |
| `Form::auto(value)` | すべてのフィールドを宣言順に並べます |

フォームの幅は最大 720 px で、フォームだけを含むカードやパネルはフォームと同じ幅になるため、カードの半分が空くことはありません。ラベルは 36 px の入力欄の 6 px 上に置かれ、フィールド同士の間隔は 20 px です。フォーカスされた入力欄にはフォーカス色の枠と柔らかい 3 px のリングが付き、無効な入力欄には危険色の枠が付きます。単位、通貨、`%` は入力欄の中に淡色で表示され、数値フィールドはマイナスとプラスのボタン（28 px）をフィールドの末尾の内側に置きます。矢印キーでも値を増減できます。

## 保存

`.submit(label, action)` は、保存を行うアクションを指定します。アクションはフォームのモデルの `Valid<M>` を受け取る必要があり、そうでなければコンパイルできません。ランタイムが値を送り、サーバーがすべてのルールを実行し、その後でアクションを実行します。

- ルール、またはアクション内の `ValidationErrors` によるフィールドのエラーは、フィールドの横に表示されます。
- `Error::Conflict`（古い `#[version]`）は、他の誰かがレコードを変更したというメッセージを表示します。
- `.on_success(|result, ui| ...)` は、保存の成功後にエフェクトを実行します（[エフェクトとウィンドウ](effects.md) を参照）。

## コントロール

フィールドの型ごとに既定のコントロールがあります。テキスト入力、数値入力、通貨付きの金額、日付と時刻のピッカー、`bool` のチェックボックス、選択肢のセレクトです。`.control(M::f, Control::...)` で別のものを選べ、`.lookup(M::f, action)` はサーバー上でレコードを検索します。[コントロールとピッカー](controls.md) を参照してください。

## 依存する lookup

`.depends_on(field, source)` は、lookup `field` の選択肢を別のフィールドに依存させます。国の都市のような場合です。検索のたびに `source` の値が `LookupQuery::parent` で送られ（`q.parent_id()` で id として読めます）、`source` が変わると `field` はクリアされるため、新しい値に合わせて選択肢が読み込み直されます。

```rust
#[action(permission = "geo.read")]
pub async fn find_cities(q: LookupQuery, store: Cities) -> Result<Page<LookupItem>> {
    Ok(q.apply(store.of_country(q.parent_id())))
}

Form::of(address)
    .lookup(Address::country, find_countries)
    .lookup(Address::city, find_cities)
    .depends_on(Address::city, Address::country)
    .submit(t!("save"), save_address)
```

この組み合わせはブラウザーに任せられないため、送信時にサーバーは、送られた国とともに選ばれた都市を lookup の action（その action の permission 付き）にもう一度問い合わせ、返されない都市はフィールドエラーで拒否します。id による問い合わせでも、フィルター後に `q.apply` を使うように、親で絞り込んでください。`App::check_page` は、フィールドがフォームの lookup でない `depends_on` や、依存元がフォームのほかのフィールドでない `depends_on` を報告します。

## フィルターフォーム

送信ボタンのないフォームで画面を絞り込めます。`.drives(topic)` はそのフィールドを画面の params にします。フィールドが変わると（セレクト、日付範囲、Enter を押したテキストフィールド）、その値は同じ名前の画面の param に入り、アドレスも同期され、`.listens(topic)` を持つノードがその場で更新されて自分の params から値を読みます（[トピック](effects.md#topics) を参照）。数値と真偽値はそのまま、空のフィールドは `null` として送られます。サーバーは params を画面の型付き params として読むため、合わない値は拒否されます。

```rust
#[params]
pub struct SalesParams {
    pub region: Option<Region>,
    pub from: Option<Date>,
    pub to: Option<Date>,
}

#[screen(route = "/sales", permission = "sales.read")]
pub fn sales(p: SalesParams, store: Sales) -> Screen {
    Screen::new(t!("sales.title"))
        .body(
            Form::of(SalesFilter { region: p.region, from: p.from, to: p.to })
                .field(SalesFilter::region)
                .range(SalesFilter::from, SalesFilter::to)
                .drives("sales"),
        )
        .body(Kpi::new(t!("sales.total"), store.total(&p)).listens("sales"))
}
```

`submit` を持つフォームは drive できません。代わりに、保存後に `on_success` の中で `ui.refresh(topic)` を使って更新してください。

## 読み取り専用フィールド

`#[readonly]` のフィールドはフォームに表示されますが、編集できません。在庫の増減（移動）でしか変わらない在庫数などがその例です。

## ファイルのアップロード

`File`、`Option<File>`、`Vec<File>` のフィールドは、`FileStore` が保持するファイルを指します。ルールを `const` として一度だけ宣言し、権限を付けてストアを Feature に登録し、同じルールをフォームで使います。

```rust
pub const ATTACHMENTS: FileRules = FileRules::new("attachments")
    .max_bytes(5 * 1024 * 1024)
    .max_files(5)
    .accept(&[FileType::Pdf, FileType::Png, FileType::Jpeg]);

Feature::new("support")
    .files(ATTACHMENTS, "support.ticket.files", DiskStore::new("data/attachments"))
    .scan_files(MyVirusScanner)

Form::of(ticket).field(Ticket::title).file(Ticket::attachments, ATTACHMENTS)
```

権限名を 1 つ指定すると、同じユーザーがアップロードとダウンロードの両方を行えます。追加はできないが閲覧はできるようにするには分けます: `.files(ATTACHMENTS, FileAccess::read("support.ticket.read").write("support.ticket.files"), store)`。

コントロールは、ドロップゾーン内のネイティブなファイル入力です。各ファイルは進捗バー付きで個別にアップロードされ、実行中はキャンセルでき、失敗したら再試行でき、完了したら削除できます。画像は、選択したファイルのオブジェクト URL から作られたプレビューを表示します。進捗と結果は、丁寧なライブリージョンで通知されます。showcaseの `/uploads` に動作する例があります。このドキュメントサイトにはサインインしたユーザーがいないため、ライブのアップロードは表示されません。

| キー | 動作 |
|---|---|
| Tab | ファイル入力へ移動し、続いて各ファイルのダウンロードリンクと、キャンセル、再試行、削除のボタンへ移動します |
| ファイル入力上の Enter または Space | システムのファイル選択を開きます（ドラッグ＆ドロップの代わりとなるキーボード操作） |
| キャンセル、再試行、削除上の Enter または Space | 実行します。キャンセル後は再試行に、削除後は次のファイルか入力にフォーカスが移ります |

`POST /_zeyn/files/{store}` に対してサーバーが行うこと:

1. CSRF ヘッダー、ストアの権限、Feature のガードを検査し、インターセプターを実行します（`CallKind::Upload`、新しいファイル ID とともに監査）。アップロードは書き込み制限にカウントされます。
2. 宣言された `Content-Length` が上限を超えていれば拒否し（`413`）、続いて最初の 4 KiB を読んで内容から型を判定します。拡張子が同じ型に属し、その型が `accept` に含まれている必要があり、そうでなければ `415` です。HTML、SVG、XML、スクリプトは決して受け付けられません。ブラウザのコンテンツタイプは無視されます。
3. 残りを、4 つ分のチャネルを通して最大 64 KiB のチャンクでストアへストリーミングし、バイト数を数えます。上限超過、接続の切断、30 秒間データが来ないことのいずれかでアップロードは停止し、ストアは書き込んだものを削除します。
4. Feature の `FileScanner` を実行します。拒否されたファイル、またはスキャナーの失敗があった場合、ファイルは削除され、応答は `422` です。
5. `{id, name, size, type}` とともに `201` で応答します。ID はサーバーが生成した 128 ビットの乱数で、名前からはパスと制御文字が取り除かれます。

ダウンロード（`GET /_zeyn/files/{store}/{id}`）は、ID の形式、権限、ガード、テナントを検査し、メディア配信（`docs/media.md` を参照）を通ります。範囲リクエスト、エンティティタグ、`nosniff` が使われます。インライン表示されるのはラスター画像、音声、動画だけです。それ以外はすべて `application/octet-stream` として `Content-Disposition: attachment`（引用符付きの ASCII 名と UTF-8 名）で、`Content-Security-Policy: sandbox` の下で送られます。

`DiskStore` は `{id}.bin` と `{id}.json` を 1 つのフォルダに保持し、アップロードはまず `{id}.part` に書き込みます。`MemoryStore` は、テストとデモ用に、バイト数の上限付きでファイルをメモリに保持します。フォームの値はファイルの名前を示すだけです。ファイルが存在することを確実にしたい保存アクションは、showcaseのように `store.meta(id)` を呼びます。

`FileRules::quota(bytes, window)` は、1 人のユーザーがウィンドウごとにストアへアップロードできる量を制限し、アプリの rate store で数えます。上限を超えたアップロード自体は保存され、次のアップロードが `429` を返します。`DiskStore` は 1 時間より古い `.part` ファイル (クラッシュで止まったアップロード) を 1 時間に最大 1 回削除します。`ZipGuard` は ZIP ベースのアップロード (ZIP、Word、Excel、PowerPoint) 用のスキャナーで、中央ディレクトリが 10,000 件を超えるエントリ、合計 1 GiB 超、またはファイルサイズの 200 倍超を宣言するアーカイブと、ZIP64 アーカイブを拒否します。受け入れたアーカイブを展開するコードも、これらの上限で止める必要があります。

予算: 256 MiB のアップロードでサーバーのヒープは約 0.4 MiB 増えます（`tests/files_memory.rs`）。コントロールは DOM ノードを 10 個と、ファイルごとに 5 個描画します。`files.js` は生で 12 KB、gzip で 4 KB、`files.css` は生で 3 KB、gzip で 1 KB 追加します。
