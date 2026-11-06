# 管理画面

バックオフィス向けのコンポーネントです。レコードで何が変わったか、読み取り専用のコードとデータ、バーコード、通知、ボード、カレンダーを扱います。どれもサーバーで描画され、使うページでだけスタイルシートとスクリプトを読み込みます。

## DiffView

`DiffView` はレコードの二つの値の間で変わったフィールドを表示します。古い値には取り消し線が引かれ、新しい値は追加として示されます。監査レコードからは `DiffView::changes(entry.changes)`、モデルの二つの値からは `DiffView::of(&before, &after)` で作ります。後者は監査証跡と同じ方法で比較します（行バージョンは除きます）。

```rust
DiffView::of(&before, &after).title(t!("orders.history"))

DiffView::changes(entry.changes.clone())
    .model::<Order>()
    .layout(DiffLayout::Inline)
```

`.model::<M>()` を指定すると、フィールドにはユーザーの言語のラベルが、選択肢には選択肢のラベルが付きます。横並びのレイアウトは古い値の列と新しい値の列を持つ表です。`DiffLayout::Inline` は新しい値を古い値の後ろに置くので、ウィンドウのような狭い場所に向いています。表示するのは最大 `MAX_DIFF_FIELDS`（200）フィールドで、残りは注記で知らせます。

マスクされたフィールドはマスクされたままです。値は監査証跡が保持するマスク済みのテキストで、モデルが `#[mask]` を付けたフィールドは表示の前にもう一度マスクされます。そのため手で組み立てた変更リストでも、生の値がページに入ることはありません。このコンポーネントにスクリプトは不要で、`diff.css`（1.9 KB）はこれと一緒の場合だけ読み込まれます。

## CodeView と JsonView

`CodeView` はソーステキストを読み取り専用で表示します。サーバーで色付けされ、行番号、折りたたみ、コピーボタンがあります。`JsonView` は JSON の値を同じようにインデントして表示します。

```rust
CodeView::new(CodeLang::Sql, query_text).title("report.sql")

JsonView::new(&payload).title(t!("webhooks.last_payload"))

JsonView::of(&customer, &ctx)
```

ハイライター（`zeyn::ui::highlight`）は Rust、JSON、TOML、SQL、JavaScript、シェル向けに手で書かれています。`CodeLang::Plain` やそれ以外の名前はエスケープしたテキストを表示します。キーワード、文字列、数値、コメント、型、キーを `z-hl-*` の span で囲み、色はテーマのトークンから取るので、両方のテーマで動き、ブラウザーは何もしません。複数行にわたる括弧のブロック（`{}` と `[]`）は、スクリプトの要らないネイティブの `<details>` 要素で折りたためます。最初の行がボタン（Enter または Space）で、折りたたんだブロックは省略記号で終わります。

`JsonView::of(value, &ctx)` はどんな値もエンドポイントがブラウザーへ送るのと同じ方法でシリアライズするので、マスクされたフィールドはマスクされたままです。表示は最大 `MAX_CODE_LINES`（10,000）行、`MAX_CODE_BYTES`（1 MB）までで、それより長いテキストは行末で切られ、何行を表示しているかを示す注記が付きます。コピーボタン（`Share` も担う `share.js`）は表示中の行をコピーしてトーストで知らせます。スクリプトが動くまでは隠れています。`code.css`（2.7 KB）はこのコンポーネントと一緒の場合だけ読み込まれます。

| キー | 動作 |
|---|---|
| Tab | テキストへ移動し（一つのタブ位置で、矢印キーでスクロール）、続いて折りたたみボタンとコピーボタンを順に移動します |
| Enter, Space | 折りたたみボタンの上で: ブロックを折りたたむか開きます |

## Barcode と QrCode

`Barcode::code128(value)` と `QrCode::new(value)` はサーバーでシンボルを SVG として描きます。どちらのエンコーダーも公開された規格（Code 128 と QR コード）に沿って手で書かれているので、クレートもスクリプトも増えません。

```rust
Row::new()
    .col(Barcode::code128(order.number.clone()).height(56))
    .col(QrCode::new(order_url).level(QrLevel::Quartile).label(t!("orders.scan")))
```

`Barcode` は印字可能な ASCII 文字を 1 から 80 文字（`MAX_BARCODE_CHARS`）エンコードし、数字の連続は 1 シンボルに 2 桁ずつ詰めます（コードセット C）。チェックシンボルと両側 10 モジュールのクワイエットゾーンを付けます。`QrCode` はテキストを UTF-8 のバイトとして、選んだレベル（`QrLevel::Low`、既定の `Medium`、`Quartile`、`High`）で収まる最小のバージョン（1 から 40）にエンコードし、リード・ソロモン誤り訂正、ペナルティが最小のマスク、4 モジュールのクワイエットゾーンを使います。最大 `MAX_QR_BYTES`（2,953）バイトです。エンコードできない値は、壊れたシンボルの代わりに短い注記を表示します。

各シンボルは figure で、スクリーンリーダーは下のテキストを名前とする一つの画像として読み上げます。そのテキストは値そのもの、または指定した場合は `.label(text)` です。スキャナーは明るい地に暗いバーを必要とするため、どちらのテーマでもバーは白い台の上で黒のままです。`barcode.css`（1 KB）はこれらのコンポーネントと一緒の場合だけ読み込まれます。`.height(px)` と `.size(px)` で描画サイズを決め、狭いコンテナではシンボルが縮みます。

## Inbox と NotificationCenter

`Inbox` はユーザーの通知を新しい順に、ユーザーのタイムゾーンでの日ごと（今日、昨日、その後は日付）にまとめて一覧にし、未読を示します。行は DataGrid と同じ種類のアクション（`DataQuery` を受け取り `Page` を返す）から来ます。受信箱は時刻フィールドで並べ、最大 `.limit(n)` 行（既定 50、上限 `MAX_INBOX` の 200）を要求します。`NotificationCenter` はトップバーのベルで、未読数付きで受信箱の画面へのリンクになります。

```rust
#[action(permission = "notes.write")]
pub async fn mark_notes_read(s: InboxSelection, store: Notes, ctx: Ctx) -> Result<()> {
    store.mark_read(ctx.identity(), &s.ids)
}

#[action(permission = "notes.read")]
pub async fn count_unread(q: UnreadCount, store: Notes, ctx: Ctx) -> Result<u32> {
    store.count_unread(ctx.identity(), q.limit)
}

Inbox::of(my_notifications)
    .when(Note::created_at)
    .read(Note::read)
    .item(|n: &Note| ListItem::new(n.title.clone()).subtitle(n.body.clone()))
    .mark_read(mark_notes_read)
    .archive(archive_notes)

App::new("Admin").header(|h| h.end(NotificationCenter::of(count_unread, inbox, ())))
```

`.mark_read(action)` と `.archive(action)` を付けると、各行にチェックボックスが、受信箱に「すべて選択」付きのツールバーが付きます。ボタンは選んだキーを `InboxSelection` として `POST /_zeyn/data` に送ります。このエンドポイントは CSRF を確認し、書き込み制限に数えられ、画面のアクセスとアクションの権限を確認します。受け付けるのは 200 文字までのキーを最大 200 個です。キーはブラウザーから来るので、アクションは `ctx.identity()` の通知だけを変更します。ユーザーが実行できないボタンは、チェックボックスと一緒に表示されません。

一括操作の後、受信箱は自分のトピック（`"notifications"`、変更は `.topic(..)`）を告げます。受信箱とそのトピックを `.listens` するすべてのノードがその場で描き直され、ベルも件数を読み込み直します。ベルは `UnreadCount { limit }` を受け取って上限（100）で止まるアクション、つまり一回の上限付きクエリで数えます。99 を超えると `99+` と表示し、リンクの名前がスクリーンリーダーに件数を伝えます。件数はページの表示後に一回のリクエストで読み込み、受信箱、`ui.refresh("notifications")`、または `.live()` と `realtime` 機能を使う場合はどのアクションの `hub.notify(&ctx, "notifications")` でも、トピックが告げられると読み込み直します（トピックは `Realtime::topic` で宣言します）。`inbox.js`（6.1 KB、gzip で 2.3 KB）と `inbox.css`（3.1 KB）はこれらのコンポーネントと一緒の場合だけ読み込まれます。
