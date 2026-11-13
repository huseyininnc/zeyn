# テキストとコンテンツ

記事、プロフィール、一覧のような読むためのページは、いくつかの小さなコンポーネントで作ります。これらはサーバーで描画されるただの HTML で、スクリプトはなく、すべてのテキストは locale ファイルから取得されるか、エスケープされます。

@live content

## Heading

`Heading::h2(text)` から `Heading::h6(text)` まであります。screen のタイトルがページ唯一の `h1` なので、ここには `h1` はありません。望むサイズではなく、アウトラインで次に来るレベルを選んでください。`.id("anchor")` を付けると、リンクからこの見出しを指せます。

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

エスケープされたテキストの段落です。`.muted()` は副次的なテキスト色を、`.small()` は小さいフォントサイズを使います。長い単語は折り返されます。

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

日付、読了時間、カテゴリのような短い情報を 1 行に並べます。区切り文字（`.separator("/")` で変えない限り中黒）はスクリーンリーダーから隠され、空の部分は省かれます。

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

前の項目と次の項目へのリンクで、`rel="prev"` と `rel="next"` が付きます。ページを再読み込みせずに開きます。「前へ」「次へ」という語はフレームワークのテキストから取得され、`.prev_title` と `.next_title` はその下に項目のタイトルを加えます。右から左に書く言語では矢印の向きが反転し、スマートフォンでは 2 つのリンクが縦に積まれます。

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
    .shortcut("Alt+Shift+N")
```

### リンクのショートカット

`.shortcut(keys)` は、最後に設定した側にキーボードショートカットを与え、`Chips::links(...).link(...).shortcut(keys)` は、最後に追加したリンク chip に与えます。そのキーを押すと、ページを再読み込みせずに `nav.js` を通じてリンクが辿られます。アプリシェルのページでもサイトシェルのページでも同じです。フォーカスが input、text area、select、編集可能な領域にあるときは何も起こらないため、入力が奪われることはありません。リンクには、スクリーンリーダー向けの `aria-keyshortcuts` が付き、キーは `<kbd>` のヒントとして表示されます（タッチ画面では非表示）。ショートカットは `Command::shortcut` と同じように読み取られ、ブラウザーが保持するもの（`Ctrl+T`、`Ctrl+W`）や使うもの（`Ctrl+P`、`F5`）は割り当てられません。`App::check` は、サイトヘッダーのスロット内のリンクのショートカットのうち、読み取れないもの、ブラウザーのもの、1 文字だけのキー（WCAG 2.2、2.1.4: `Shift+N` は該当し、`Alt+Shift+N` は該当しません）、コマンド、パレット、または別のリンクと衝突するものを報告します。`App::check` は自分で画面を構築できないため、`App::check_page(&screen)` は、テストでパラメーター付きで構築した画面について同じ検査を行います。

フォーカスが別オリジンのフレーム（埋め込み動画プレーヤーなど）の中にあるときに押したキーは、そのフレームのドキュメントに届き、ページには届きません。スクリプトでこれを変えることはできません。ショートカットを使う前に、フレームの外をクリックするか、Tab でフレームから出るよう、ユーザーに伝えてください。

## AutoGrid

列数が利用できる幅に応じて変わるグリッドに子要素を並べます。幅が `min` ピクセル以上（指定しなければ 240）の列が入るだけ並び、スマートフォンでは 1 列です。ブラウザが `repeat(auto-fill, minmax(min(240px, 100%), 1fr))` で配置するため、ブレークポイントもスクリプトもありません。カード、パネル、グリッドなど、どんなノードでも入れられます。この名前は、データの行を表示する `DataGrid` と区別するためのものです。

```rust
AutoGrid::new()
    .min(220)
    .gap(4)
    .add(Card::new().title(t!("home.orders")))
    .add(Card::new().title(t!("home.stock")))
    .add(Card::new().title(t!("home.people")))
```

@live autogrid

## List

`<ul>` の中に行を縦に並べます。各 `ListItem` にはタイトルがあり、先頭の小さな画像（カード画像と同じ安全なアドレス：パス、`https:`、`data:image/`）、サブタイトル、末尾の短いメタを付けることもできます。テキストか、`RelativeTime` のようなインラインコンポーネントです。`.link(screen, params)` を使うと行全体が 1 つのリンクになり、タブ順では 1 か所、フォーカスリングは見え、ページを再読み込みせずに開きます。長いテキストは折り返され、スマートフォンではメタテキストがタイトルの下に移り、右から左に書くページでは行が左右反転します。

`List::new()` は渡された項目をそのまま保持します。`List::of(action)` は DataGrid と同じ種類の action で行を読み込み、[カードグリッド](catalog.md#card-grid)と同じようにページングします。`paging` で指定しない限り 20 行と「Show more」ボタン、並べ替えオプション、`.search(text)` によるサーバー側の検索があり、ページごとに action のアクセス権が確認されます。

```rust
List::new()
    .title(t!("team.title"))
    .item(ListItem::new("Ada Lovelace").subtitle(t!("team.math")))
    .item(ListItem::new("Alan Turing").subtitle(t!("team.computing")).meta(t!("team.lead")))

List::of(search_people)
    .title(t!("people.title"))
    .item(|p: &Person| {
        ListItem::new(p.name.clone())
            .image(p.photo.clone())
            .subtitle(p.city.clone())
            .meta(RelativeTime::new(p.joined))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name)
    .paging(Paging::pages(20))
```

@live list

### 並べ替えられるリスト

`.sortable(action)` を使うと、`List::of` や `CardGrid` の行を利用者が自分の順番に並べられます。各行にハンドルが付きます。マウスやペンはそのままドラッグし、タッチは少し長押ししてからドラッグします。落ちる位置は線で示されます。フォーカスのあるハンドルでは Space で行を持ち上げ、矢印キーで動かし、Space で置き、Escape で元に戻します。Alt と Up または Down ですぐに動かせます。クリックか Enter で移動メニュー(上へ、下へ、先頭へ、末尾へ)が開くので、単一のポインターで足ります。各操作は読み上げられます。

移動はすぐに表示され、アクションは行のキーと新しい隣の行のキー(`after`、`before`)を持つ `ReorderRequest` を受け取ります。位置は送りません。`r.rank(lookup)` は隣の行の間の新しい `Rank` を返し、検索で見つからない隣の行(`NotFound`)や、もうその順番にない隣の行(`Conflict`、先に誰かが行を動かした)を拒否します。そのときは行が元に戻り、理由が表示されます。元のアクションは行をランク順に返すので並べ替えの選択肢は出ません。また、まだ読み込む行がある間はどの行も最後には移せません。 ドラッグ中の行を端の近くに保つと、リスト(またはページ)がスクロールします。`Paging::virtual_scroll()` のリストは仮想のままです。線が行の落ちる位置を示し、サーバーが保存したあとで行が新しい順に表示されます。

```rust
#[action(permission = "tasks.write")]
pub async fn reorder_tasks(r: ReorderRequest<TaskId>, db: Db) -> Result<()> {
    let rank = r.rank(|k| db.rank_of(*k))?;
    db.set_rank(r.key, rank).await
}

List::of(search_tasks)
    .item(|t: &Task| ListItem::new(t.title.clone()))
    .sortable(Task::rank, reorder_tasks)
```

## Avatar

人やチームの丸い画像で、サイズは 32、40、64 ピクセルです（`AvatarSize::Small`、`Medium`、`Large`）。名前は必須です。名前は代替テキストであり、画像がないとき、または画像がアプリの `img-src` で許可されたホストにないときに表示されるイニシャルの元になります（`Image` と同じ規則）。イニシャルはその言語の大文字に従うため、トルコ語の `i` は `İ` になります。名前がアバターのすぐ隣に書かれている場合は、`.decorative()` でアバターをスクリーンリーダーから隠し、名前が二度読まれないようにします。

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

読者が開くまで隠れているコンテンツで、たとえば質問の答えです。標準の開閉要素（`<details>` と `<summary>`）なので、スクリプトなしでキーボードやスクリーンリーダーで使え、開くまで内容は読み上げ順に入りません。ラベルはアプリが指定します。

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## AvatarGroup

タスクの担当者のように、ひとつの物事に関わる人々を、一列に重なり合うアバターとして表示します。ラベルはスクリーンリーダー向けにグループの名前になります。グループは最初のアバターを表示します。`.max(n)` で指定しない限り 5 個で、12 個を超えることはありません。残りの人数は "+3" と表示され、ユーザーの言語で「他 3 人」と読み上げられます。`.size(...)` はグループ内のすべてのアバターのサイズを設定します。

```rust
AvatarGroup::new(t!("task.assignees"))
    .add(Avatar::new(a.name.clone()).image(a.photo.clone()))
    .add(Avatar::new(b.name.clone()))
    .max(3)
    .size(AvatarSize::Small)
```

## Quote

人や作品から引用した言葉を、`figure` の中の `blockquote` として表示します。`.author(name)` は誰の言葉かを示し、`.source(title)` は作品名を示します。作品名はタイトルとして書かれます（`cite`）。

```rust
Quote::new(review.text.clone()).author(review.name.clone()).source(t!("shop.verified_buyer"))
```

## Kbd

押すキーを、キーボード入力として表示します（`kbd`）。`Kbd::new("Ctrl+K")` はキーの間に `+` がある組み合わせを読み取ります。`Kbd::keys(["Ctrl", "+"])` はキーをひとつずつ受け取り、プラスキーそのものを含む組み合わせに使います。キー名は渡されたとおりに書かれ、最大 6 個です。キーを表示するだけです。実際に何かを行うショートカットは、`PrevNext::shortcut` のように、コマンドのある場所で設定します。

```rust
Kbd::new("Alt+Shift+P")
```

## Divider

ページの 2 つの部分の間の線です。`Divider::new()` は主題の区切り（`hr`）です。`.label(text)` は線の中央にテキストを置きます。たとえば 2 つのサインイン方法の間の「または」です。このとき区切り線は、そのテキストを名前に持つ `separator` になります。`.vertical()` は線を上から下に引きます。コンパクトな `Row` の列の間に使います。

```rust
Divider::new().label(t!("signin.or"))
```

@live pieces

## Highlight

`Highlight::new(text, query)` は、検索語を印付き（`mark`）にしてテキストを出力します。検索結果の一覧に表示する商品名などに使います。クエリは空白で分割され、そのうち最大 8 語を探し、いずれかの語が見つかったすべての箇所に、大文字と小文字を区別せずに印を付けます。空のクエリでは何も印を付けません。テキストもクエリもユーザーデータです。テキストはエスケープされ、クエリは比較に使われるだけで、ページに書き出されることはありません。文字は Unicode の定義する小文字で比較されるため、トルコ語の `I ı` と `İ i` の組は同じ文字として扱われません。

```rust
Highlight::new(product.name.clone(), p.q.clone())
```

## Toc

目次です。表示中のページの見出しへのリンクで、記事やポリシーのような長いページに使います。`Toc::new(label)` はラベルを名前に持つ `nav` です。`.item(title, id)` は、`Heading::id` で同じアンカーを指定した見出しにリンクし、`.sub(title, id)` は直前の項目の 1 段下に項目を追加します。アンカーは `Heading::id` と同じ方法で整えられるので、同じ文字列を両方で使えます。整えた結果アンカーが空になった項目は省かれます。目次が持てる項目は最大 100 個です。リンクは `#anchor` へのただのリンクなので、スクリプトなしで動作し、ブラウザーが見出しまでスクロールし、アドレスを共有できます。読んでいる箇所に印を付けることはしません。それにはスクリプトが必要になるからです。

```rust
Toc::new(t!("page.contents"))
    .item(t!("policy.delivery"), "delivery")
    .sub(t!("policy.abroad"), "delivery-abroad")
    .item(t!("policy.returns"), "returns")
Heading::h2(t!("policy.delivery")).id("delivery")
```

@live marks

## Stepper

プロセスがどこまで進んだかを示します。順に並んだステップのうち、完了したもの、現在のもの、これからのものです。たとえば注文が確定してから配達されるまでの流れです。`.step(title)` はステップを追加し、`.description(text)` は最後に追加したステップの下に 1 行を置きます。`.current(index)` は 0 から数え、それより前のステップを完了として示します。`.finished()` はすべてのステップを完了として示します。`.vertical()` はステップを縦に並べます。狭い画面でも同じ並びになります。stepper が持てるステップは最大 12 個です。

```rust
Stepper::new(t!("order.progress"))
    .step(t!("order.placed"))
    .step(t!("order.packed"))
    .step(t!("order.shipped"))
    .description(order.shipped_note.clone())
    .step(t!("order.delivered"))
    .current(order.stage as usize)
```

@live stepper

ステップは、ラベルを名前に持つ順序付きリストです。現在のステップは `aria-current="step"` を持ち、完了したステップはスクリーンリーダーにユーザーの言語で「完了」と伝えるので、状態が色だけで示されることはありません。stepper はサーバーが渡した状態を表示するだけで、押せるものは何もありません。ステップごとに入力するフォームは、独自のステップを持つ `Form` です（[フォーム](forms.md)を参照）。

## Pagination

画面が自分でページ分けするリスト、たとえばブログの記事のページへのリンクです。前へ、ページ番号、次へ、が並びます。それぞれは、ページ番号をパラメーターに持つ同じ画面へのリンクなので、サーバーは求められたページを描画し、アドレスは共有でき、ブラウザーには何も保持されません。リンクは `nav.js` を通じてページを再読み込みせずに開き、スクリプトがなくても動作します。`DataGrid`、`List`、`CardGrid` は `.paging(...)` で自分でページ分けします。これは画面が自分でコンテンツを組み立てる場合にだけ使ってください。

```rust
#[screen(route = "/blog", public)]
pub fn blog(p: BlogParams) -> Screen {
    Screen::new(t!("blog.title")).body(
        Pagination::new(blog, |page| BlogParams { page })
            .pages(page_count)
            .current(p.page),
    )
}
```

ページは 1 から数えます。最初のページと最後のページは常に表示され、現在のページの両側に 1 ページずつ表示されます。`.around(n)` でさらに多く、最大 4 ページまで表示できます。省略されたページは省略記号になり、省略記号が 1 ページだけを隠すことはありません。ページ番号はアドレスから来るので、画面はデータを読み込む前にそれを範囲内に収めます。コンポーネントも、現在のページとして示すものについて同じことを行います。ページが 1 つ以下の場合は何も描画しません。

リンクは、ユーザーの言語で「ページ送り」という名前の `nav` です。現在のページは `aria-current="page"` を持ち、各番号はスクリーンリーダー向けに「3 ページ」という名前になり、前へと次へは `rel="prev"` と `rel="next"` を持ちます。両端では、どこにも行かないリンクの代わりに、`aria-disabled` の付いたただのテキストになります。

| キー | 動作 |
|---|---|
| `Tab` | 前へ、ページ番号、次へ、の順に移動します |
| `Enter` | ページを開きます。フォーカスとスクロール位置は `nav.js` に従います |

## Chips

複数行に折り返す、小さな丸いリンクの列です。`Chips::links(label)` は、記事のタグなど画面への通常のリンクを、label で名前が付いた `nav` の中に持ちます。`Chips::filter(label, screen, params)` は画面パラメーターに対するトグルボタンを作ります。各 chip は同じ画面へのリンクで、その値がパラメーターに追加または削除されるため、サーバーは受け取ったパラメーターから `aria-pressed` を描画し、ブラウザーには何も保持されません。chip を押すと `nav.js` を通じてページを再読み込みせずに新しいアドレスが開き、フォーカスとスクロール位置は chip に残ります。`.multiple()` を呼ばない限り、一度に押せる値は一つです。パラメーター内の選択肢でない値は無視されます。キーボード: Tab で chip 間を移動し、Enter または Space で押し、リンク chip には独自のショートカットを付けられます（[リンクのショートカット](#link-shortcuts) を参照）。押された chip にはチェックマークも表示されるので、状態が色だけに頼りません。スクリプト（`chips.js`、1 KB 未満）はフィルター chip があるページでのみ読み込まれます。

```rust
#[screen(route = "/articles", public)]
pub fn articles(p: ArticleParams) -> Screen {
    Screen::new(t!("blog.articles")).body(
        Chips::filter(t!("blog.tags"), articles, |tags| ArticleParams { tags: tags.join(",") })
            .current(p.tags.split(','))
            .option("guide", t!("blog.guides"))
            .option("news", t!("blog.news"))
            .multiple(),
    )
}
```

@live chips


## Hero

ページ上部の大きなセクションで、背景画像にタイトル、一行のテキスト、ボタンなどのアクションが付きます。`.link(label, screen, params)` は別のページへの `LinkButton` を追加し、スライドの最初のものはプライマリの外観、以降はセカンダリの外観になります。デフォルトでは静的です。`.slide(title)` はスライドをもう一つ追加し、`.text`、`.image`、`.action` は常に最後のスライドに設定します。複数のスライドがある場合は W3C APG のカルーセルパターンに従います。スライド選択ボタンがあり、各スライドは「2 / 3」という名前のグループで、選ばれたスライドは控えめに読み上げられます。アプリが `.rotate(seconds)`（4 から 30）を呼ばない限り、スライドはユーザーが選んだときだけ切り替わります。呼んだ場合は一時停止と再生のボタンが先頭に来て（WCAG 2.2.2）、ポインターが hero の上にあるとき、フォーカスがその中にあるとき、タブが非表示のときは回転が止まり、ユーザーがモーション低減を望む場合は開始しません。スライドが動いている間は読み上げられません。スクリプトがなければ最初のスライドが表示されます。

画像は `Image` のルールに従います（アプリのパス、またはアプリの `img-src` にあるホストの `https` アドレス）。テキストの背後の装飾なので代替テキストは空です。重要な内容はタイトルとテキストに書いてください。これらは、どんな画像の上でも白いテキストのコントラストが 4.5:1 以上に保たれる暗いスクリムの上に置かれます。リンクボタンは塗りつぶしの色を保ち、画像の上では白いフォーカスリングが付きます。最初のスライドの画像は素早い初回描画のために高い優先度で読み込まれ、他は遅延読み込みされます。スクリプト（`hero.js`、1.6 KB gzip）はスライドがあるページでのみ読み込まれます。

```rust
Hero::new(t!("home.fresh"))
    .text(t!("home.fresh_text"))
    .image("/img/hero/kitchen.jpg")
    .link(t!("home.menu"), menu, ())
    .action(ActionButton::new(t!("home.order"), start_order, ()))
    .slide(t!("home.new_teas"))
    .image("/img/hero/teas.jpg")
    .rotate(8)
```

@live hero


## Reactions

アプリが選んだリアクションボタンの列で、それぞれにキー、名前、絵文字などの記号、そして件数があります。`Reactions::of(action, target)` はリアクションの対象となる項目を指定します。target は画面が設定し、ブラウザーから読み取ることはありません。ボタンを押すとリアクションが `POST /_zeyn/data` に送られ、このエンドポイントは CSRF を確認し、書き込みレート制限に数えられ、画面を開き、アクションの権限を確認し、アクションが実行される前に集合外のリアクションを拒否します。アクションは `ReactionPress`（target、reaction、pressed）を受け取って保存し、新しい `ReactionState`（件数とユーザー自身のリアクション）を返し、件数と `aria-pressed` の状態はその場で更新されます。各ボタンはリアクションの名前と件数で名前が付いたトグルボタンで、記号はスクリーンリーダーから隠されます。`.state(...)` は最初の描画の件数を与えるので、スクリプトなしでも表示されます。スクリプト（`reactions.js`、1.3 KB gzip）はリアクションがあるページでのみ読み込まれます。

```rust
#[action(permission = "answers.react")]
pub async fn react_to_answer(press: ReactionPress, db: Db, ctx: Ctx) -> Result<ReactionState> {
    db.toggle_reaction(&ctx, &press.target, &press.reaction, press.pressed).await?;
    db.reaction_state(&ctx, &press.target).await
}

Reactions::of(react_to_answer, format!("answer-{}", answer.id))
    .reaction("helpful", t!("answers.helpful"), "\u{1f44d}")
    .reaction("thanks", t!("answers.thanks"), "\u{1f64f}")
    .state(answer.reactions.clone())
```

@live reactions


## RelativeTime

ページの言語で「3 分前」や「2 日後」のような相対時間として表示される時刻です。サーバーはユーザーのタイムゾーンでの正確な日付と時刻を `<time datetime="...">` の中に書くので、HTML は分ごとに変わらず、キャッシュされたページも正しいままです。ランタイムは、ページの読み込み時、`nav.js` が画面を差し替えたとき、および 1 分ごとに `Intl.RelativeTimeFormat` でテキストを書き換えます。正確な時刻はツールチップに残ります。スクリプトがなければ、正確な時刻がテキストになります。

```rust
RelativeTime::new(post.published)
```


## Share

リンクを共有するボタンです。ブラウザーに Web Share API がある場合（ほとんどのスマートフォン）はシステムの共有シートを、ない場合はその旨を伝えるトーストとともにクリップボードへコピーします。表示中のページを共有するか、コンパイラーがパラメーターを検査する `Share::screen(screen, params)` で画面を共有します。`.title` は共有シートに表示されるタイトルを設定し、ラベルは `.label` で指定しない限りユーザーの言語で「共有」になります。サーバーはボタンを非表示で描画し、スクリプトがそれを表示するので、スクリプトのないページには何もしないボタンがありません。スクリプト（`share.js`、1 KB 未満）は共有ボタンのあるページでのみ読み込まれます。

```rust
Share::page().title(post.title.clone())
Share::screen(article, ArticleParams { id: post.id }).label(t!("blog.share_article"))
```

@live engage


## CopyButton

テキストをクリップボードにコピーするボタンです。追跡番号、API のアドレス、招待リンクなどに使います。`.shown()` はテキストをボタンの横に等幅フォントで表示するので、読んだり選択したりもできます。指定しない場合、テキストはページ内にありますが非表示です。ラベルは `.label` で設定しない限りユーザーの言語の「コピー」で、コピーが成功したかどうかは toast で伝えます。`Share` と同じ小さなスクリプトを使います。このスクリプトは、こうしたボタンのあるページでだけ読み込まれます。ボタンは非表示で描画され、そのスクリプトによって表示されるので、スクリプトのないページに何もしないボタンが出ることはありません。テキストは 4096 バイトで切られます。またページの一部になるので、ユーザーが見てよいテキストでなければならず、マスクされた値は決して渡してはいけません。

```rust
CopyButton::new(order.tracking.clone()).shown()
CopyButton::new(invite_url).label(t!("team.copy_invitation"))
```

## AlphaIndex

文字を並べたバーで、各文字はその文字をパラメーターに持つ screen へのリンクです。たとえば、名前がその文字で始まる人を表示します。文字はユーザーの言語から、その言語自身の順序で取得されます（`zeyn::ui::alphabet`）。トルコ語では `Ç Ğ I İ Ö Ş Ü` が正しい位置に入り、スペイン語には `Ñ` があり、ロシア語とアラビア語にはそれぞれの文字体系があり、それ以外の言語は `A` から `Z` です。表示中の screen の文字には `aria-current="page"` が付き（トルコ語の `i` は `İ` に付く）、`.other()` は、それ以外の文字で始まる名前のために最後に `#` リンクを加えます。リンクはページを再読み込みせずに開き、スマートフォンでは複数行に折り返されます。

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

action から取得した行を、曜日ごとの 7 つの列に並べます。1 週間の授業、シフト、イベントなどに使います。action は DataGrid と同じ種類で、同じように検査されます。`.day` は行を曜日に対応づけます。`Date`、`Option<Date>`（日付のない行は除外）、または 1（月曜）から 7（日曜）までの曜日番号が使え、それ以外はコンパイルできません。各行は `ListItem` として表示されるため、screen へリンクできます。週はユーザーの週の最初の曜日から始まり（トルコ語では月曜、アメリカ英語では日曜）、今日には印が付き、行のない日はその旨が表示され、狭い画面では曜日が縦に積まれます。サーバーは action の順序で最大 `limit` 行（指定しなければ 100、最大 500）を読み込みます。action にはその週の行を渡してください。たとえば日付にフィルターを使います。

```rust
WeekSchedule::of(this_week_classes)
    .title(t!("classes.this_week"))
    .day(|c: &Class| c.date)
    .item(|c: &Class| {
        ListItem::new(c.name.clone())
            .meta(c.starts.to_string())
            .link(class, ClassParams { id: c.id })
    })
```

@live week
