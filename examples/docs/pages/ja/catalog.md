# カード、レール、検索

ショップのような公開ページでは、行を表ではなくカードとして表示します。`CardGrid`、`Rail`、`SearchBox` は、DataGrid が使うのと同じ種類のアクションで行を読み込みます。そのアクションは `DataQuery` を受け取り、`Page` を返します。ページング、並べ替え、検索テキストはサーバー上で実行され、DataGrid と同様に、読み取りにはアクションのアクセス権が必要です。

```rust
#[action(public)]
pub async fn catalog_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?.into_iter().filter(|p| p.active).collect()))
}
```

## カードグリッド

```rust
CardGrid::of(catalog_products)
    .title(t!("shop.products"))
    .card(|p: &Product| {
        Card::image(p.image.clone(), p.name.clone())
            .text(p.price.to_string())
            .badge(Badge::new(t!("shop.new")).tone(Tone::Success))
            .link(shop_product, ProductPage { id: p.id })
    })
    .sort_option(t!("shop.sort.name"), Product::name, false)
    .sort_option(t!("shop.sort.cheapest"), Product::price, false)
    .image_size(300, 300)
    .paging(Paging::infinite(24))
```

- **カード**。`Card::image(image, title)` は、行から 1 枚のカードを作り、`text`、`badge`、`alt`、`link`（画面とそのパラメータ。カード全体が 1 つのリンクになり、ページを再読み込みせずに開きます）を付けられます。画像は遅延読み込みされ、`image_size` の幅と高さが使われるため、ページが跳ねることはありません。画像の代替テキストは空です。隣にあるタイトルがカードの名前を示すからです。画像がそれ以上のことを伝える場合は `alt` を設定してください。パス、`https:`、`data:image/` 以外の画像アドレスは除外されます。
- **カードのその他の要素**。`subtitle` はタイトルの下に、より控えめな 2 行目を加えます（著者、カテゴリなど）。`corner(badge)` は画像の終端側の角にバッジを重ね、始端側には `badge` が置かれます。`badges([...])` はテキストの下にタグのようなバッジの列を加えます。`progress(fraction, label)` は、どこまで進んだかを示す細いバーを画像の下端に沿って描きます。割合は 0 から 1 で、スクリーンリーダーはラベルとパーセントを読み上げます（`role="progressbar"` と `aria-valuenow`）。`horizontal()` はカードを横一列に並べ、先頭に小さな画像、その横にテキストを置きます。このようなカードを含むカードグリッドやレールでは、列が広くなります。いずれも `CardGrid` と `Rail` で使えます。

```rust
Card::image(c.cover.clone(), c.title.clone())
    .subtitle(c.teacher.clone())
    .corner(Badge::new(t!("courses.new")).tone(Tone::Success))
    .badges(c.tags.iter().map(|t| Badge::new(t.clone())))
    .progress(c.done, t!("courses.done"))
    .horizontal()
```
- **最初のページはサーバーで。** 最初のページ（カードグリッドは 24 枚、rail は 12 枚、または `paging` で指定した数）はページの HTML に含まれるため、検索エンジンや JavaScript のない訪問者もカードとリンクを見られます。画像はサイズを持っているので、読み込み中に何も動きません。以降のページはブラウザーが読み込みます。公開ページはどの訪問者にも同じ HTML なので、キャッシュできます。ルートを持つ screen にリンクしたカードは、リダイレクトではなくその正規のアドレス（`/items/{slug}`）にリンクします。
- **ページング**。`Paging::infinite(n)`（既定、24）は「もっと見る」ボタンを表示し、このボタンはスクロールして表示範囲に入ったときにも作動します。ボタンを押すと、フォーカスは最初の新しいカードに移ります。`Paging::pages(n)` は、前へと次へのボタンとページ番号を表示します。action が合計を返すときは「Page 2 of 5」、返さないときは「Page 2」と表示され、その場合は action がカーソルを返す間、次へが動作します。ページサイズはサーバーが決め、クライアントは続きの位置だけを伝えます。
- **並べ替え**。各 `sort_option` は、ユーザーがセレクトから選べる順序です。最初のものがカードの初期の並び順になります。サーバーはこれらの順序だけを受け付け、クライアントが送ったフィルターと検索テキストは破棄します。
- **スコープ**。`.scope(field, value)` は、そのフィールドがその値である行だけを残し、すべてのリクエストで [`DataGrid::scope`](datagrid.md) と同じように働きます。たとえば `.scope(Article::author_id, author.id)` は 1 人の著者の記事だけを残します。カードグリッド、rail、データにバインドされたリストは、クライアントが送るフィルターをすべて破棄するため、クライアントはスコープを変更することも外すこともできません。テキストは大文字小文字を区別せずに比較されます。
- **検索**。`.search(text)` は、検索結果画面のテキストのように、自由テキスト（[`DataQuery::q`](#free-text)）に一致する行だけを表示します。
- **マスクされたフィールド**。カードは、生の行を見るクロージャによって構築されます。`#[mask]` フィールドを持つモデルの場合、それを表示（reveal）できるユーザーだけがカードを取得でき、それ以外のユーザーには `Forbidden` が返ります。公開カードには、マスクされたフィールドのないモデルを使ってください。

### ホバープレビュー

リンクを持つ `ImageCard` または `ListItem` の `.preview(screen, params)` は、その画面をカードの横のパネルに表示します。マウスで約 0.5 秒ホバーした後、またはカードがキーボードフォーカスを得たときは直ちに表示されます。画面はそのときに初めて、ウィンドウと同じアクセスチェックのもとで `GET /_zeyn/screen/...` から読み込まれ、各プレビューはページの存続中保持されます（最大 24 件、`nav.js` が別の画面を開くと消去）。フレーム、オブジェクト、メディアプレーヤー、スクリプトはパネルが表示される前に取り除かれるので、プレビューが iframe を読み込むことはありません。パネルは最上位レイヤーにあるため、内容を切り取る rail やグリッドに切られず、余白の広い側に開き、画面の端で反転し、内部でスクロールし、ページをスクロールすることはありません。WCAG 1.4.13 に従い、ポインターをパネルの上に移動でき、Escape で閉じ、ポインターまたはフォーカスが離れるまで表示されます。タッチ画面や粗いポインターではプレビューはなく、タップでリンクが開きます。スクリプト（`preview.js`、2.5 KB gzip）はそこでは読み込まれず、それ以外ではそのようなカードへの最初のホバーまたはフォーカスで読み込まれます。モーション低減の場合、パネルは短い拡大アニメーションなしで表示されます。

```rust
Rail::of(search_products).card(|p: &Product| {
    Card::image(p.image.clone(), p.name.clone())
        .link(product, ProductParams { id: p.id })
        .preview(product_summary, ProductParams { id: p.id })
})
```

@live preview


### ビルダー

既定は `.card` と `.item` のままです。リンク、画像、スクリーンリーダー向けのテキストが手間なく得られます。行に別の形が必要なときは、`.builder(|row| node)` が Flutter の `ListView.builder` のように各行を任意のノードとして組み立てます。`List::of`、`CardGrid`、`Rail` で使えます。`.separator(node)` は行と行の間にノードを置き（行を数えるスクリーンリーダーからは隠されます）、`Repeat::of(items, |item| node)` は画面がすでにメモリーに持っているリストについて、最大 500 項目まで同じことをします。

```rust
List::of(search_people)
    .builder(|p: &Person| Row::new().compact().col(Avatar::new(p.name.clone())).col(Paragraph::new(p.city.clone())))
    .separator(Paragraph::new("·"))

Repeat::of(order.steps.iter(), |s| Badge::new(s.name.clone())).separator(Paragraph::new("→"))
```

- **キー。** 組み立てた行と仮想リストの行はモデルのキーを `data-zeyn-key` に持つため、リストの更新（[トピック](effects.md#topics)）でも、レコードがどこへ移動しても、フォーカスと開いている details は同じレコードに残ります。
- **組み立てた行は表示用です。** そのノードはノードパスでアドレス指定されないため、グリッド、フォーム、lookup は別の場所に置いてください。`Repeat` のノードは子（`p.0`、`p.1`、...）なので、そのまま動作します。
- **仮想スクロール。** `.paging(Paging::virtual_scroll())` を持つリストは、全行分の高さを持つスクロール領域の中で、見えている行だけをページに置き、ユーザーのスクロールに合わせてブロックを offset で取得します。行の高さは 1 つの固定値で、行ごとの高さはありません。ビルダーでは `.row_height(px)` が必須で、ない場合は `App::check_page` が報告します（そこでは `separator` は使われません）。各行は `aria-posinset` と `aria-setsize` を持つため、スクリーンリーダーはリスト全体の中での位置を読み上げます。スクロール領域は、タイトルで名前が付いたフォーカス可能な region です。

## レール

レールは、横にスクロールする 1 行に並べたカードグリッドで、スクロールスナップと、1 画面分の幅ずつスクロールする前へ・次へのボタンがあります（ユーザーがモーションの軽減を求めていない限り滑らかに動きます）。ユーザーが末尾の近くまでスクロールすると、次のバッチを 12 件ずつ読み込みます。カードはタブ順に含まれるリンクで、フォーカスされると表示範囲へスクロールされます。

スクロールバーは非表示です。タッチ、トラックパッド、Shift+ホイールでは引き続きスクロールでき、`.scrollbar(true)` で細いバーを残せます。マウスで rail を押したまま横にドラッグできます。5 ピクセルを超えて動かすと、離したときのクリックは取り消されるため、ポインターの下のカードは開かず、rail は最も近いカードに揃います。それより小さい動きは通常のクリックです。ドラッグ中はテキストが選択されず、画像が外へドラッグされず、カードのプレビューも開きません。タッチではブラウザー自身のスクロールが使われます。前へ・次へのボタンは常に表示され、端では無効になります。非表示のスクロールバーではキーボードでスクロールできないためです。JavaScript がない場合、ボタンは非表示になり、rail はネイティブにスクロールします。

```rust
Rail::of(catalog_products)
    .title(t!("shop.new_arrivals"))
    .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
    .sort_option(t!("shop.sort.newest"), Product::id, true)
```

## 検索ボックス

候補付きの検索フィールドで、W3C APG の combobox パターンに従い、リストボックスのポップアップを持ちます。ユーザーが入力すると `instant` ミリ秒待ってから、サーバーに候補を問い合わせます。アクションは、そのテキストを `DataQuery::q` として実行され、各行の候補テキストが返されます（`suggestion` で指定しない限り、マスクされていない最初のテキストフィールド）。Enter を押すか候補を選ぶと、ページを再読み込みせずに、そのテキストで結果画面が開きます。

```rust
#[screen(route = "/shop/search", public)]
pub fn shop_search(p: SearchParams) -> Screen {
    Screen::new(t!("shop.search.all"))
        .body(SearchBox::of(catalog_products).value(p.q.clone()).results(shop_search))
        .body(CardGrid::of(catalog_products).card(product_card).search(p.q).paging(Paging::pages(12)))
}

SearchBox::of(catalog_products)
    .label(t!("shop.search.label"))
    .instant(150)
    .suggestion(|p: &Product| p.name.clone())
    .results(shop_search)
```

キーボード: 下矢印と上矢印で候補間を移動し（下矢印で候補が開きます）、Enter は強調表示された候補か入力したテキストで検索し、Escape は候補を閉じ、続けて押すとフィールドを消去します。ライブリージョンが、スクリーンリーダーに候補の数を伝えます。

結果画面は `SearchParams` を受け取ります（`q`。メニューから開いた場合は空）。サーバーはテキストを `MAX_SEARCH_QUERY`（100）文字で切り詰め、送る候補の数を決め（`limit` で指定しない限り 8、最大 20）、画面のアクセス権、Feature のガード、アクションの権限を検査し、すべての候補の呼び出しを書き込みのレート制限（`App::write_limit`）に数えます。

`.drives(topic)` は、結果画面を開く代わりにその場で検索します。Enter、候補の選択、フィールドのクリアのどれでも、テキストが画面の `q` param に入り、トピックを listen しているノードが更新されます（[トピック](effects.md#topics) を参照）。画面の params には、`SearchParams` と同じく `q: String` フィールドが必要です。

### リッチな候補

`.item(|row| ListItem::new(title).subtitle(text).image(src).link(screen, params))` は、各候補を、小さな画像（40 x 56 ピクセル、遅延読み込み、`img-src` のルールに従い、タイトルが行の名前になるため代替テキストは空で、画像がなければプレースホルダーは表示しません）、タイトル、サブタイトルを持つ行として表示します。サーバーは、レコードの正規アドレスを付けた `[{text, subtitle, image, href}]` を返します。クリックまたは Enter で行を選ぶと、`nav.js` を通じてそのリンクが開き、各行は本物のリンクなので、中クリックや「新しいタブで開く」も使えます。ハイライトされた行がない状態で Enter を押すと、従来どおり結果画面が開きます。combobox パターンは変わりません。各行はタイトルとサブタイトルで名前が付いた option で、行の高さは 44 ピクセル以上です。`.item` がなければ、候補は上記のようにプレーンテキストです。

```rust
SearchBox::of(catalog_products)
    .item(|p: &Product| {
        ListItem::new(p.name.clone())
            .subtitle(p.code.clone())
            .image(p.image.clone())
            .link(shop_product, ProductPage { id: p.id })
    })
    .results(shop_search)
```

## 自由テキスト

`DataQuery::q` は自由テキストです。テキストフィールドにそれを含む行が対象になります。テキストもフィールドも、`zeyn::text::fold` が返す形で比較されます。小文字、トルコ語の点付きと点なしの `i`（`İ`、`ı`）は `i`、アクセント付きのラテン文字はアクセントなし、`ß`、`æ`、`œ`、`þ` は展開した形です。そのため「istanbul」で「İSTANBUL」が、「nino」で「Niño」が、「strasse」で「Straße」が見つかります。`#[mask]` を持つフィールドは検索されません。空白のテキストはすべての行に一致し、100 文字より長いテキストは無視されます。`DataQuery::apply` と SQL のデータ層（列に対する `TRANSLATE`、`LOWER`、`REPLACE` と、バインドされたパラメータ付きの `LIKE`）も、同じように読み取ります。

```rust
use zeyn::text::fold;

assert_eq!(fold("İSTANBUL Işık"), "istanbul isik");
assert_eq!(fold("Año Pequeño"), "ano pequeno");
assert_eq!(fold("Straße MÜLLER"), "strasse muller");
```

アプリ自身が書き込む検索キーの列、たとえば[手書きのクエリ](database.md)のトライグラム列にも `fold` を使うと、キーがユーザーの入力と一致します。句読点とほかの文字体系はそのまま残ります。アドレスには `Slug::from_title` を使います。
