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
- **ページング**。`Paging::infinite(n)`（既定、24）は「もっと見る」ボタンを表示し、このボタンはスクロールして表示範囲に入ったときにも作動します。ボタンを押すと、フォーカスは最初の新しいカードに移ります。`Paging::pages(n)` は、前へと次へのボタンとページ番号を表示します。ページサイズはサーバーが決め、クライアントは続きの位置だけを伝えます。
- **並べ替え**。各 `sort_option` は、ユーザーがセレクトから選べる順序です。最初のものがカードの初期の並び順になります。サーバーはこれらの順序だけを受け付け、クライアントが送ったフィルターと検索テキストは破棄します。
- **検索**。`.search(text)` は、検索結果画面のテキストのように、自由テキスト（[`DataQuery::q`](#free-text)）に一致する行だけを表示します。
- **マスクされたフィールド**。カードは、生の行を見るクロージャによって構築されます。`#[mask]` フィールドを持つモデルの場合、それを表示（reveal）できるユーザーだけがカードを取得でき、それ以外のユーザーには `Forbidden` が返ります。公開カードには、マスクされたフィールドのないモデルを使ってください。

## レール

レールは、横にスクロールする 1 行に並べたカードグリッドで、スクロールスナップと、1 画面分の幅ずつスクロールする前へ・次へのボタンがあります（ユーザーがモーションの軽減を求めていない限り滑らかに動きます）。ユーザーが末尾の近くまでスクロールすると、次のバッチを 12 件ずつ読み込みます。カードはタブ順に含まれるリンクで、フォーカスされると表示範囲へスクロールされます。

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

## 自由テキスト

`DataQuery::q` は自由テキストです。大文字小文字を区別せず、テキストフィールドにそれを含む行が対象になります。トルコ語の点付きと点なしの `i`（`İ`、`ı`）は `i` に一致するため、「istanbul」で「İSTANBUL」が見つかります。`#[mask]` を持つフィールドは検索されません。空白のテキストはすべての行に一致し、100 文字より長いテキストは無視されます。`DataQuery::apply` と SQL のデータ層（バインドされたパラメータ付きの `LOWER(TRANSLATE(column, 'İı', 'ii')) LIKE ...`）も、同じように読み取ります。
