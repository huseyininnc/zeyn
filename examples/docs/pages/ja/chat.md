# チャット

会話のためのコンポーネントです。会話のメッセージ、ユーザーが書き込むボックス、誰かが入力中であることを知らせる表示があります。どれもサーバーで描画され、スタイルシートとスクリプトはそれを使うページでだけ読み込まれます。

## MessageThread

`MessageThread` は 1 つの会話のメッセージをチャットの吹き出しとして表示します。ユーザー自身のメッセージは終端側に、ほかの人のメッセージは開始側に置かれます（右から左に書く言語では左右が反転します）。同じ送信者が 5 分以内に続けて送ったメッセージは 1 つの名前の下にまとめられ、ユーザーのタイムゾーンで日ごとに見出しが付き、各メッセージには時刻が、自分のメッセージには配信状態が表示されます。

```rust
#[action(permission = "chat.read")]
pub async fn conversation_messages(q: ThreadQuery, store: Chats, ctx: Ctx) -> Result<Page<Msg>> {
    store.check_member(&q.conversation, &ctx)?;
    Ok(q.query.apply(store.messages(&q.conversation)))
}

MessageThread::of(conversation_messages)
    .conversation(params.id.to_string())
    .author(Msg::author)
    .name(Msg::author_name)
    .text(Msg::body)
    .when(Msg::sent_at)
    .status(Msg::status)
    .attachments(Msg::files, CHAT_FILES)
    .live()
```

履歴の action は `ThreadQuery` を受け取ります。これは、サーバーがコンポーネントから取り出す会話（ブラウザーが送ることはありません）と、新しい順に並べた `DataQuery` です。ユーザーがその会話の参加者であることを action の中で確認してください。`.author` は送信者のユーザー ID で、サインインしているユーザーと比べて自分のメッセージとほかの人のメッセージを見分けます。`.name` は表示される名前です。モデルには `#[key]` が必要で、これにより 2 回読み込まれたメッセージも 1 回だけ表示されます。

最初の 50 件のメッセージ（`THREAD_PAGE`）は画面と一緒に描画されます。上にスクロールすると、次の古いページが `POST /_zeyn/grid` で読み込まれます。そのたびに画面へのアクセスと action の権限が確認され、読者が見ているメッセージはその位置に保たれます。ページに残るメッセージは最大 200 件です。さらに進むと遠い側の端が捨てられ、読者が戻ってきたときに再び読み込まれます。そのため、数千件のメッセージがある会話でも短い会話と同じメモリしか使いません。5,000 件のメッセージのスクロールは、ページ上の要素がおよそ 2,000 個の状態でディスプレイのフレームレートで動きます。

`Delivery` には `Sending`、`Sent`、`Delivered`、`Read`（既読通知）、`Failed` の状態があり、それぞれアイコンとスクリーンリーダー向けのテキストで示されます。`.attachments(field, rules)` はメッセージのファイルを表示します（`Vec<File>` フィールドと、`Feature::files` で登録したストアのルール）。画像はプレースホルダーの上に遅延読み込みされる固定サイズのボックスに表示され、それ以外のファイルはストアの読み取り権限を通るダウンロードリンクとして表示されます。

新しいメッセージは、スレッドの topic のどれかが指定されたときに取得されます。`.topic(..)` で別の指定をしない限り `"chat"` です。`Composer` が送信した後、action が `ui.refresh("chat")` で終わった後、または `.live()` と `realtime` feature がある場合はどこかで `hub.notify(&ctx, "chat")` が実行されたときです。ほかの人のメッセージは polite なライブリージョンで読み上げられます。

### AI の返信

```rust
#[action(permission = "chat.ai")]
pub async fn ask_model(r: ReplyRequest, store: Chats) -> Result<Reply> {
    let id = store.reserve_id();
    let (tx, reply) = Reply::channel(id.to_string());
    tokio::spawn(async move {
        let mut text = String::new();
        for piece in my_model_stream(&r.text).await {
            text.push_str(&piece);
            if !tx.send(piece).await {
                break;
            }
        }
        store.save_ai_answer(id, &r.conversation, text);
    });
    Ok(reply)
}

MessageThread::of(conversation_messages)
    .conversation(id)
    .ai(Msg::from_ai)
    .reply(ask_model)
```

`.reply(action)` を付けると、同じ会話の `Composer` で送られたメッセージが AI の返信を開始します。ブラウザーはテキストを `POST /_zeyn/stream` に送り（CSRF、書き込み制限、画面へのアクセス、action の権限、インターセプター）、返信は組み込みアシスタントと同じ形式の JSON 行としてストリームで返ってきます。断片は上限付きのチャネル（`REPLY_BUFFER`）を通ります。サーバーは `MAX_REPLY_CHARS`（32,000）文字または `REPLY_SECONDS`（120）秒で止まり、停止ボタンで返信を早めに終えることもできます。`ReplySender::send` は読者がいなくなると `false` を返し、これが生成側への停止の合図になります。

返信はテキストノードだけで組み立てた小さな Markdown のサブセットとして表示されるため、モデルが書いたものが HTML として解析されることはありません。それまでのメッセージが再描画されることはなく、描画されるのは書き込み中の段落だけです。2,000 個の断片からなる返信でも、最後のフレームあたりのコストは最初と同じで、上にあるものは動きません。ライブリージョンは返信が完了したときに一度だけ読み上げ、断片ごとには読み上げません。`.ai(field)` で印を付けたメッセージも、履歴から安全な Markdown として読み込まれます（`markdown` feature がある場合。ない場合はプレーンテキスト）。

返信のキー（`Reply::channel(key)`）は保存されるメッセージが持つキーと同じなので、履歴は 2 つ目のコピーではなく保存された回答をその位置に表示します。

## Composer

`Composer` はユーザーがメッセージを書くボックスです。テキストに合わせて最大の高さまで伸びるテキストエリア、送信ボタン、そして必要に応じて添付、メンション、保持される下書きを備えます。

```rust
#[action(permission = "chat.write")]
pub async fn send_message(m: ChatSend, store: Chats, hub: Hub, ctx: Ctx) -> Result<()> {
    store.check_member(&m.conversation, &ctx)?;
    store.add(&m.conversation, &ctx, m.text, m.attachments, m.mentions)?;
    hub.notify(&ctx, "chat");
    Ok(())
}

pub const CHAT_DRAFTS: ClientPref<Draft> =
    ClientPref::draft("chat-draft", Duration::from_secs(7 * 86_400), Purpose::Preference);

Composer::of(send_message)
    .conversation(params.id.to_string())
    .attachments(CHAT_FILES)
    .mentions(search_people)
    .remember(CHAT_DRAFTS)
```

送信の action は `ChatSend` を受け取ります。サーバーがコンポーネントから取り出す会話、前後の空白を除いたテキスト、添付ファイル、メンションされた人の ID です。送信は CSRF トークン付きで `POST /_zeyn/data` を通り、書き込み制限に数えられ、action の権限が必要です。ユーザーが送信できない composer はページに含まれません。サーバーは、空のメッセージ、`.max_chars(n)`（指定しなければ 4,000）より長いメッセージ、ストアのルールが許すより多いファイル、形式の正しくないファイル ID、20 件を超えるメンションを拒否します。それ以外も信頼できないものとして扱ってください。ユーザーがその会話の参加者であり、メンションされた人を指定してよいことを確認してください。

既定の `SendKey::Enter` は Enter で送信し、Shift+Enter で改行します。`.send_key(SendKey::CtrlEnter)` は Ctrl+Enter（Mac では Cmd+Enter）で送信し、Enter は改行に使います。メッセージが空か長すぎる間は送信ボタンが無効になり、上限に近づくとカウンターが表示されます。送信後、composer は自分の topic を指定します（`.topic(..)` で別の指定をしない限り `"chat"`）。そのため同じ会話の `MessageThread` はメッセージをすぐに表示し、`.reply(..)` があれば AI の返信をストリームで表示します。

`.attachments(rules)` は `Feature::files` で登録したストアのアップロード欄を追加します。ファイルは進捗バー付きで 1 つずつ `POST /_zeyn/files/{store}` にアップロードされ、ストアの書き込み権限と種類の確認を通り、その ID がメッセージと一緒に送られます。

`.mentions(action)` は `@` の後に lookup action で人を検索します（入力は `LookupQuery`、出力は `LookupItem` のページ）。検索は最後のキー入力から 200 ms 後に始まり、新しい検索は古い検索を取り消します。サーバーが求めるのは最大 `MAX_MENTIONS`（8）人で、返すのもその数までです。リストはテキストエリアから操作する listbox で、矢印キーで移動し、Enter か Tab で選び、Escape で閉じます。

`.remember(key)` は、各会話の未送信のテキストを、送信されるまでクライアント設定ストアの `App::client_prefs` で登録した `Draft` キーの下に保持します。下書きは最大 `MAX_DRAFT_CHARS`（4,000）文字、保持される会話は最大 `MAX_DRAFTS`（20）件で、古いものから消えます。会話 ID はハッシュとしてのみ保存されます。`Purpose::Preference` の場合、ユーザーがそれ以上に同意するまで、下書きはセッションの間だけ残ります。

## TypingIndicator

`TypingIndicator` は、会話でほかに誰が入力中かを polite なライブリージョンで示します。「Ada が入力中」「Ada と Grace が入力中」「3 人が入力中」のように表示されます。

```rust
let realtime = Realtime::new().topic("chat", "chat.read");

TypingIndicator::new("chat")
    .conversation(params.id.to_string())
    .person("ada", "Ada Lovelace")
    .person("grace", "Grace Hopper")
```

これはリアルタイムチャネル（`realtime` feature、`realtime.md` を参照）の上で、`Realtime::topic` で宣言した topic を使って動きます。ユーザーが同じ会話の `Composer` で入力している間、ブラウザーはページのライブソケットで小さな信号を最大 3 秒に 1 回送ります。サーバーはユーザーごとにそれより速い信号を捨て、信号をテナント内の topic のほかの購読者に渡し、同じユーザーに送り返すことはありません。信号に含まれるのは会話 ID と、このサーバーだけが作れるユーザーのキーで、ユーザー ID やテキストは決して含まれません。名前は最後の信号から 6 秒間表示されます。`.person(user, name)` で指定していない人は「誰か」と表示されます。

`realtime` feature がない場合、またはアプリにリアルタイムチャネルがない場合、インジケーターは何も描画せず、スクリプトも読み込まれず、何も送られません。信号はテナント内の topic のすべての購読者に届くので、会話 ID は彼らに見えます。不透明な ID を使うか、対象者ごとに topic を分けてください。
