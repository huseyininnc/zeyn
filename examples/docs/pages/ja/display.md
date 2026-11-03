# 表示

状態を示すコンポーネントです。メッセージ、バッジ、進捗、キーと値、履歴、プレースホルダーがあります。サーバー上でレンダリングされ、独自のリクエストは必要ありません。

```rust
Stack::new()
    .add(Alert::info("Imports run in the background.").dismissible())
    .add(Alert::warning("Three products are low on stock.").title("Stock"))
    .add(Row::new().compact().col(Badge::new("Paid").tone(Tone::Success)).col(Badge::new("Overdue").tone(Tone::Danger)).col(Badge::new("Draft")))
    .add(Progress::new("Import", 64, 100))
    .add(Timeline::new()
        .event(ctx.now().add_ms(-26 * 3_600_000), "Created", None, Tone::Info)
        .event(ctx.now().add_ms(-3_600_000), "Approved", Some(Text::from("by the manager")), Tone::Success))
    .add(Skeleton::lines(2))
```

@live display

## Alert

`Alert::info`、`success`、`warning`、`danger`、または `Alert::new(tone, text)` です。`.title(text)` は見出しを、`.dismissible()` は閉じるボタンを追加します。トーンはメッセージの前に読み上げられるため、色だけで伝えられることはありません。

## Badge

`Badge::new(text).tone(Tone::...)`。トーンは `Neutral`、`Info`、`Success`、`Warning`、`Danger`、`Brand` です。グリッドは `.badge(|v| tone)` で値をバッジとして表示します。

## Progress

`Progress::new(label, value, max)` に、任意で `.tone(...)` を付けます。ラベルと値を持つ本物の `progressbar` です。

## DescriptionList

レコードのフィールドから、または手動で作るキーと値の組です。

```rust
DescriptionList::of(&invoice)
    .field(Invoice::customer)
    .field(Invoice::issued)
    .item(t!("ui.status"), "INV-2026-0042")
```

値はグリッドのセルと同じように整形されます。金額、日付、選択肢がユーザーの言語で表示されます。

## Timeline

`Timeline::new().event(moment, title, detail, tone)` はイベントを一覧にします。時刻は、ユーザーのタイムゾーンと言語で表示されます。

## EmptyState と Skeleton

`EmptyState::new(title).text(text).opens(label, screen, params)` は、空の領域を次のステップで埋めます。`Skeleton::lines(n)` は、何かを読み込んでいる間のプレースホルダーです。

## Breadcrumb

`Breadcrumb::new().link(label, screen, params).current(label)`: リンクは型付きの画面参照なので、画面の名前を変えてもリンク切れは起きません。

## ActionButton

`ActionButton::new(label, action, input)` はアクションを実行します。`.confirm(text)` は先に確認を求め、`.tone(Tone::Danger)` は破壊的な操作であることを示し、`.on_success(|result, ui| ...)` はエフェクトを実行します。入力はサーバー上で固定され、クライアントから来ることはありません。アクションの権限は、他のフォームと同様にサーバー上で検査されます。

```rust
ActionButton::new(t!("docs.live.count"), count_samples, ())
    .on_success(|n, ui| {
        if let Ok(n) = n {
            ui.toast(t!("docs.live.counted", n = n));
        }
    })
```

@live action

## Markdown

`Markdown::new(text)` は、コメント、商品の説明、ヘルプなどの整形されたテキストを表示します。テキストは文字列かロケールキーです。`markdown` feature（`zeyn = { features = ["markdown"] }`）が必要で、依存関係は増えません。

```rust
Markdown::new(product.description.clone())
Markdown::new(t!("help.import")).trusted()
```

- 見出し、段落、リスト、表、コードブロック、引用、罫線、インラインコード、リンク、画像、太字、斜体。
- 生の HTML は決してそのまま通されません。`<script>` はテキストとして表示され、コードブロックもエスケープされます。
- リンクは `http`、`https`、`mailto`、アンカー、相対パスだけを指せます。`javascript:` や `data:` などそれ以外は、ラベルだけが残ります。ユーザーのリンクには `rel="noopener noreferrer nofollow ugc"` が付きます。
- 画像はアプリ自身（相対パス）からのみ読み込まれ、他の画像は代替テキストが表示されます。
- `.trusted()` は、アプリが書いたテキスト用です。見出しにはリンク用の ID が付き、リンクからは `nofollow ugc` が外れます。エスケープとリンクのルールは同じままです。
- 64 KB（`MAX_MARKDOWN`）を超えるテキストは切り詰められ、入れ子にも上限があるため、悪意のある入力でもレンダリングの負荷は小さいままです。

## Embed

`Embed` は、動画プレーヤーや地図のような別のサイトのページを、ロックされたフレームの中に表示します。タイトルは必須です（スクリーンリーダーはそれでフレームを読み上げます）。`.title(...)` がなければ、埋め込みは配置できず、コンパイラがそう伝えます。

```rust
Embed::url("https://www.youtube-nocookie.com/embed/abc")
    .title(t!("tour.title"))
    .allow([EmbedPermission::Fullscreen, EmbedPermission::PictureInPicture])
    .aspect(16, 9)
    .click_to_load("/static/tour.jpg")
```

- ホストは、アプリの `frame-src` に含まれている必要があります: `App::csp(|c| c.frame_src(["https://www.youtube-nocookie.com"]))`（[セキュリティ](security.md#csrf-csp-and-headers) を参照）。アドレスが `https` でない、またはそのホストが一覧にない場合、フレームはまったくレンダリングされず、コンテンツを表示できないという警告が出ます。
- すべてのフレームに `sandbox="allow-scripts allow-same-origin allow-presentation"`、`referrerpolicy="no-referrer"`、`loading="lazy"` が付きます。ポップアップ、トップレベルのナビゲーション、フォーム、ダウンロードを許可することはできません。`.allow(...)` はブラウザの機能を追加します: `Fullscreen`、`PictureInPicture`、`Autoplay`、`EncryptedMedia` です。
- `.click_to_load(poster)` は、まずボタンを、ポスター画像（アプリ上のパス、または `img-src` のホスト。なしの場合は `""`）と相手のサイトの名前とともに表示します。ボタンが押されるまで、相手のサイトには接続されません。押されるとフレームがその場所に入り、フォーカスを受け取ります。このスクリプトは `media.js` の一部なので、追加のリクエストは発生しません。
- `.notice(t!(...))` は、コンテンツに関する警告のような、アプリのテキストをそのボタンに表示します。エスケープされ、他のすべてのテキストと同様に翻訳され、ボタンのアクセシブルな名前の一部になるため、ユーザーが判断する前にスクリーンリーダーがそれを読み上げます。`.click_to_load` がない場合は、ポスターなしでボタンが有効になります。
- `.remember("external-video", Duration::from_secs(24 * 60 * 60))` は、ページごとではなく、その時間につき一度だけ確認します。ブラウザは終了時刻（Unix ミリ秒）だけを `localStorage` の `zeyn.consent.{scope}` に保持します。Cookie は設定されず、サーバーがそれを見ることもないため、キャッシュされた公開ページは全員にとって同じままです。その後のページ、アプリ内ナビゲーション、他のタブは、そのスコープのフレームを自動的に、フォーカスを動かさずに読み込みます（WCAG 2.2 の 3.2.1 フォーカス時、2.4.3 フォーカス順序）。フォーカスがフレームに移るのは、実際に押された後だけです。時間が過ぎたとき、保存された値が整数でない、または埋め込みの時間より先の時刻になっているとき（手で書き換えられた場合）、ストレージがブロックされているとき、JavaScript がないときには、ボタンが再び表示されます。スコープは ASCII の英字、数字、`-`、`_` の 1 から 40 文字で、時間は 1 秒から 30 日（`MAX_EMBED_CONSENT`）です。それ以外はパニックとなり、メッセージでその旨が示されます。同じ notice を共有する埋め込みは、スコープを共有します。
- 埋め込むアドレスを保持するフィールドには `#[rule(embed)]` を付けます。フォームは URL または貼り付けられた埋め込みコードを受け付け、`src` だけを残し、`https` と一覧にあるホストを要求し、トラッキングパラメータ（`utm_*`、`fbclid`、`gclid` など）を取り除き、HTML ではなく正規のアドレスを保存します。API の入力は、正規のアドレスを送る必要があります。

```rust
#[entity(table = "videos")]
pub struct Video {
    #[key]
    pub id: VideoId,
    #[rule(required, embed)]
    pub url: String,
}
```

## Image

`Image` には代替テキストとサイズが必要です。`.alt(...)` と `.size(width, height)` がなければ配置できず、コンパイラがそう伝えます。サイズは、ファイルの読み込み中も場所を確保するため、ページが動くことはありません。

```rust
Image::new("https://images.example.com/logo.png")
    .alt(t!("brand.logo"))
    .size(240, 80)
    .lazy()
    .srcset([("https://images.example.com/logo@2x.png", "2x")])
```

- `src` は、アプリ上のパス（`/static/logo.png`）か、アプリの `img-src` にあるホストの `https` アドレス（`App::csp(|c| c.img_src([...]))`）です。それ以外の画像はレンダリングされず、代わりに代替テキストが表示されるため、ブラウザがブロックされたリクエストをログに残すことはありません。
- すべての画像に `decoding="async"` が付きます。`.lazy()` は、画面の下にある画像に `loading="lazy"` を追加します。`srcset` は密度（`2x`）または幅（`800w`）の記述子を受け取ります。他のホストのエントリや他の記述子のエントリは除外されます。幅の記述子は、画像の幅から `sizes` を追加します。
- 純粋に装飾的な画像には、空の代替テキストを指定します: `.alt(Text::from(""))`。

## ExternalLink

`ExternalLink::new(url, text)` は、`rel="noopener noreferrer"` 付きで別のサイトへリンクするため、相手のサイトにはページのアドレスもウィンドウへの参照も渡りません。スクリーンリーダーはテキストの後に「external site」と読み上げ、視覚的には矢印で示されます。

```rust
ExternalLink::new("https://example.com/help", t!("help.center")).new_tab()
ExternalLink::new(customer.website.clone(), t!("customer.website")).user_content()
```

`.user_content()` は、ユーザーが入力したアドレスに `nofollow ugc` を追加します。`.new_tab()` は新しいタブで開き、そのことをスクリーンリーダーに伝えます。指定しない限り、リンクは同じタブで開きます。リンクになるのは `https` と `http` のアドレスだけで、`javascript:` などそれ以外はプレーンテキストとして表示されます。

## ツールチップ、ポップオーバー、ビジーオーバーレイ

ツールチップは、1 つのコントロールの短い説明です。`ActionButton::tip(text)`、画面のツールバーの `ToolButton::tip(text)`、そしてフィールドのラベルの後に情報ボタンを追加する `Form::tip(field, text)` があります。テキストはコントロールの `aria-describedby` になるため、スクリーンリーダーはコントロールと一緒にそれを読み上げます。短いホバーの後に、フォーカス時には即座に表示され、ポインターがその上にある間は残り（WCAG 1.4.13）、Escape でフォーカスを動かさずに隠れ、タッチでは長押し（情報ボタンのタップ）で表示されます。ツールチップに、ユーザーが必ず知る必要のある情報を載せてはいけません。それはラベル、フィールドのルール、ページに書いてください。

`Popover::new(label).title(title).add(node)` は、小さな詳細のために、その隣に非モーダルのダイアログを開くボタンです。フォーカスはその中に移り、Escape、閉じるボタン、外側のクリックで閉じ、フォーカスはボタンに戻ります。狭い画面では、ボトムシートとして開きます。その子は通常のノードなので、中のフォームは、サーバーのすべての検査付きで、自身のノードパスへ送信されます。

`Busy::new().add(node)` は、その内側のリクエストが実行されている間、内容を覆います。フォームの保存、アクションボタン、グリッドやツリーの読み込みなどです。コンテナにはすぐに `aria-busy` が付き、スピナーとテキスト「Working」（ステータスメッセージ）は 400 ミリ秒後、または `.delay_ms(n)` の後にだけ表示されるため、速い応答でちらつくことはなく、いったん表示されると少なくとも 300 ミリ秒は残ります。オーバーレイはポインターをブロックしますが、キーボードはブロックせず、フォーカスは元の場所に残ります。

```rust
Stack::new()
    .add(ActionButton::new(t!("docs.live.count"), count_samples, ()).tip(t!("docs.live.count_tip")))
    .add(Popover::new(t!("docs.live.rules")).title(t!("docs.live.rules_title")).add(rules_list))
    .add(Busy::new().add(ActionButton::new(t!("docs.live.slow"), slow_count, ())))
```

@live overlay

| キー | 場所 | 動作 |
|---|---|---|
| Tab | ツールチップのあるコントロール | フォーカスするとツールチップが表示され、離れると隠れます |
| Escape | ツールチップの表示中ならどこでも | ツールチップを隠します。フォーカスは残ります |
| Enter、Space | ポップオーバーのボタン | ポップオーバーを開きます。フォーカスは最初のコントロールに移ります |
| Escape | ポップオーバー内 | 閉じます。フォーカスはボタンに戻ります |
| Tab | ポップオーバー内 | そのコントロールを順に移動します。離れると閉じます |

この 3 つには、約 2.2 KB（gzip）のスクリプトが必要で、独自のリクエストは不要です。ツールチップは 1 ノード、ポップオーバーは 6 ノードに内容、ビジーオーバーレイは 3 ノードです。

## ドロップダウンボタンと分割ボタン

`MenuButton::new(label)` はコマンドのメニューを開きます。`MenuButton::split(label, action, input)` は、主要部分から 1 つのアクションを実行し、矢印からメニューを開きます。コマンドには、`.action(label, action, input)`（成功後にエフェクトを実行する `.action_then(...)` もあります）、ウィンドウを開くなどのエフェクトのための `.item(label, effect)`、`.separator()` があります。`.confirm(text)` は、最後のアクションコマンドを実行する前に確認を求めます。

```rust
MenuButton::split(t!("orders.save"), save_draft, id)
    .action(t!("orders.save_and_send"), save_and_send, id)
    .separator()
    .action(t!("orders.delete"), delete_order, id)
    .confirm(t!("orders.delete_confirm"))
```

アクションコマンドは独自のパスを持つノードで、`ActionButton` と同様にフォームのエンドポイントへ送信されます。画面のアクセス権、Feature のガード、アクションの権限、インターセプター、CSRF、書き込みのレート制限は、呼び出しのたびに検査され、入力はサーバー上に残ります。ユーザーが実行できないコマンドはメニューから省かれます（分割ボタンの主要アクションは、代わりに無効になります）。それでも呼び出された場合は、サーバーが拒否します。

@live menus

| キー | 場所 | 動作 |
|---|---|---|
| Enter、Space、下矢印 | ボタン | 最初のコマンドでメニューを開きます |
| 上矢印 | ボタン | 最後のコマンドでメニューを開きます |
| 下矢印、上矢印 | メニュー | 次と前のコマンド。端で循環します |
| Home、End | メニュー | 最初と最後のコマンド |
| 文字キー | メニュー | その文字で始まる次のコマンド |
| Enter、Space | コマンド | 実行してメニューを閉じます |
| Escape | メニュー | 閉じます。フォーカスはボタンに戻ります |
| Tab | メニュー | 閉じて、次へ移動します |

メニューには約 0.8 KB（gzip）のスクリプトが必要で、コマンドが実行されるまでリクエストは発生しません。4 ノードに、コマンドごとに 2 ノードが加わります。
