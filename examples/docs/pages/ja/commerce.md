# ショップ用コンポーネント

商品一覧や商品ページには専用のコンポーネントがいくつか必要です。割引価格、件数付きのフィルター、範囲スライダー、バリエーション選択、ギャラリー、数量ステッパー、レビューの集計、カウントダウンです。ほかのコンポーネントと同じくサーバーで描画されます。ブラウザーのコードが必要なものは、それを含むページでだけ小さなモジュールを読み込み、スタイル（`commerce.css`）もそのページでだけ読み込まれます。

## 価格

```rust
Price::new(p.price)
    .was(p.list_price)
    .unit(p.price_per_kg, PriceUnit::Kg)
    .large()
```

- **割引。** `was(old)` は現在の価格の横に元の価格を取り消し線付きで表示し、整数に丸めた割引率を示します。現在の価格より高くない元の価格や、通貨が異なる元の価格は表示されないので、古い割引がマイナスの割引率を示すことはありません。
- **スクリーンリーダー**には「通常価格 ₺1,599.00、現在 ₺1,299.00、19% オフ」という 1 文が読み上げられます。多くのスクリーンリーダーは取り消し線を読み上げないため、取り消し線付きの価格とバッジは隠されます。
- **単位価格。** `unit(price, PriceUnit::Kg)` は「1 kg あたり ₺259.80」のような控えめな行を追加します。単位は `Item`、`Kg`、`Grams100`、`Liter`、`Ml100`、`Meter` です。
- **書式。** 金額はページの数値書式に従う `Money` です。英語では `₺1,299.00`、トルコ語では `1.299,00 ₺` となり、パーセント記号を前に置く言語では前に付きます（`%19`）。`large()` は商品ページ向けに現在の価格を大きくします。
- **スクリプトなし。** 価格はただの HTML です。


### カード上の価格

`ImageCard::price(Price)` は同じ価格を `CardGrid` や `Rail` のカードのテキストの下、カードのリンクの中に置くので、スクリーンリーダーにはタイトルと価格の文が一緒に読み上げられます。スクリプトは追加されず、ショップのスタイルシートは価格のある最初のカードと一緒に読み込まれます。

```rust
CardGrid::of(catalog_products).card(|p: &Product| {
    Card::image(p.photo.clone(), p.name.clone())
        .price(Price::new(p.price).was(p.list_price))
        .link(product_page, ProductPage { id: p.id })
})
```

## 範囲スライダー

```rust
RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
    .step(50)
    .money(Currency::Try)
    .value(p.min_price, p.max_price)
    .drives("products")
```

- **2 つのネイティブなつまみ。** それぞれのつまみは 2 つの名前のどちらかを持つ `<input type="range">` なので、ブラウザーが W3C APG のスライダーの動作を与えます。つまみごとに Tab の停止位置が 1 つあり、矢印キーで 1 ステップ動き、Home と End で両端へ移り、スクリーンリーダーには書式付きの値（`aria-valuetext`、たとえば「₺500.00」）とラベル（「Price、下限」「Price、上限」）が読み上げられます。Page Up と Page Down は範囲の 10 分の 1 ずつ動かします。
- **フィルター。** `.drives(topic)` はスライダーをフィルターフォームに入れます。つまみを離すと 2 つの値が同じ名前の画面パラメーターに入り、そのトピックのリスナーがその場で更新されます。ファセットパネルの中ではパネルのフォームがこれを行い、それ以外のフォームではつまみがほかのフィールドと同じように送信されます。
- **値。** `value(low, high)` はパラメーターを受け取ります（`None` ならトラックの端のまま）。範囲外の値は切り詰められ、下限が上限より大きければ入れ替わるので、手で書き換えたアドレスでスライダーが壊れることはありません。`money(currency)` は通貨の整数単位をページの書式で表示します。
- **日付。** `RangeSlider::dates(label, names, first, last)` と `.date_value(from, to)` で日を選びます。フォームは `2026-03-31` のような ISO 日付を隠しフィールドで送ります。
- **スクリプト。** `range.js`（gzip で約 1 KB）はスライダーのあるページでだけ読み込まれます。下のつまみが上のつまみを超えないようにし、つまみの間のトラックを塗り、値をラベルの横に書き、何もないトラックが押されたら近いほうのつまみを動かします。トラックの大きさは押すたびに 1 回だけ読み、つまみが動いている間は読みません。

## 数量ステッパー

```rust
#[action(permission = "cart.edit")]
pub async fn set_quantity(change: QuantityChange, cart: CartStore) -> Result<QuantityState> {
    let stock = cart.stock(&change.target)?;
    let kept = change.quantity.min(stock);
    cart.set(&change.target, kept)?;
    Ok(QuantityState::new(kept).max(stock))
}

QuantityStepper::of(set_quantity, line.sku.clone())
    .label(t!("cart.quantity"))
    .range(1, 20)
    .value(line.quantity)
```

- **サーバーで確認。** 変更（マイナスやプラスを押す、数字を入力する）は、ユーザーが手を止めてから 400 ms 後に `POST /_zeyn/data` で送られます。CSRF の確認と書き込みのレート制限を通り、ユーザーとして、アクションの権限を確認して実行されます。サーバーはアクションの実行前に数量をステッパーの範囲に収め、アクションは在庫のように自分だけが知ることを確認します。対象の品目（`target`）は画面が決め、ブラウザーからは読みません。
- **答えが優先。** アクションは保持した数量と上限（`QuantityState::new(kept).max(stock)`）を返します。それがユーザーの求めた数より少なければ、ステッパーはその数を表示し、ライブリージョンで「在庫は 7 個だけです。」と伝えます。新しい変更はまだ途中のリクエストを取り消すので、表示されるのは最後の答えだけです。リクエストが失敗すると最後に保持された数量に戻し、そのことを伝えます。
- **キーボードとタッチ。** ボタンは 24 × 24 ピクセル以上（WCAG 2.5.8）で、「減らす」「増やす」という名前を持ち、範囲の端では無効になります。押したボタンが無効になるとフォーカスは入力欄に移ります。入力欄は数字と矢印キーを受け付けます。
- **スクリプト。** `quantity.js`（gzip で約 1 KB）はステッパーのあるページでだけ読み込まれます。

## バリエーション選択

```rust
VariantPicker::new()
    .group(
        VariantGroup::swatches("color", t!("shop.color"))
            .swatch("sand", t!("shop.sand"), "#d8c3a5")
            .swatch("navy", t!("shop.navy"), "#1f3a68")
            .current(p.color.as_deref()),
    )
    .group(
        VariantGroup::chips("size", t!("shop.size"))
            .option("m", "M")
            .option("xl", "XL")
            .unavailable("xl", t!("shop.not_in_color"))
            .current(p.size.as_deref()),
    )
    .drives("variant")
```

- **ラジオグループ。** 各グループはネイティブのラジオボタンでできた fieldset で、スクリプトなしの W3C APG ラジオグループのパターンです。Tab で選択中の選択肢に移り、矢印キーで移動して選び、legend がグループ名と選択中の選択肢（「色 ネイビー」）を伝えます。色見本は 16 進の色かテーマトークンを受け取り、チップはテキストを表示します。
- **選べない選択肢**は理由付きで無効になります（`unavailable(value, reason)`）。スクリーンリーダーは選択肢と一緒に理由を読み上げ、ツールチップにも表示されます。色見本には斜線が、チップには取り消し線が付きます。
- **価格、写真、在庫が追従します。** 選択はそのトピックを動かし、自分でもそれを聞きます。選択肢を選ぶと、その値がグループ名の画面パラメーターに入り、アドレスも合わせて更新されるので選んだバリエーションのリンクを共有でき、トピックを聞くすべてのノードが 1 回のリクエストで更新されます。価格、ギャラリー、在庫の行に `.listens("variant")` を付ければ、画面は新しい選択に合わせて選択肢をまた無効にします。トピックのモジュール以外にスクリプトは要りません。
- **チェック。** `App::check_page` は何も動かさない選択と、有効な色のない色見本を報告します。

## 商品ギャラリー

```rust
p.photos.iter().fold(ProductGallery::new(p.name.clone()), |g, photo| {
    g.image(
        Image::new(photo.large.clone())
            .alt(photo.alt.clone())
            .size(800, 800)
            .low(photo.tiny.clone())
            .placeholder(Placeholder::color("--z-surface-2")),
    )
    .thumb(photo.thumb.clone())
})
```

- **一度に 1 枚。** スライドはスクロールスナップのトラックに並ぶので、スマートフォンではスクリプトなしでスワイプして切り替えられます。画像の下のサムネイルはボタンで（「5 枚中 2 枚目の写真を表示」、現在の 1 枚には `aria-current`）、フォーカスのある画像では矢印キーでページの読む方向にスライドを切り替えます。ギャラリーは名前付きのカルーセル領域で、スライドは「2 of 5」と伝えます（W3C APG のカルーセルパターン）。サムネイル（`thumb`）がなければ番号付きの点を表示します。
- **見えるものだけを読み込みます。** 最初の画像はたいてい商品ページでいちばん大きな要素なので、`fetchpriority="high"` ですぐに読み込みます。残りは不活性なテンプレートで待ち、`gallery.js` が 1 つの IntersectionObserver で見守りながら、画面上の画像とその両隣を入れます。12 枚のギャラリーなら最初に読み込むのは 3 枚です。画像は `Image` なので、プレースホルダー、低解像度のファイル、代わりのファイルはそのまま使えます。
- **拡大と全画面。** マウスではポインターの下の画像を拡大します（スライドの大きさはポインターが入ったときに 1 回だけ読み、動いている間は読みません）。画像を押すか「全画面」ボタンを押すと、画像、前と次のボタン、拡大の切り替え、閉じるボタンのあるモーダルダイアログが開きます。フォーカスは中にとどまり、Escape で閉じ、開いた要素にフォーカスを戻します。動きを減らす設定では、スライドも拡大のアニメーションもしません。
- **スクリプト。** `gallery.js`（gzip で約 1.5 KB）はギャラリーのあるページでだけ読み込まれます。

## ファセットパネル

```rust
#[params]
#[derive(Default)]
pub struct Listing {
    #[serde(default)]
    pub color: Vec<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
}

#[screen(route = "/shop", public)]
pub async fn shop(p: Listing, store: ProductStore) -> Result<Screen> {
    let colors = store.count_colors(&p).await?;
    let panel = FacetPanel::of(shop, &p)
        .facet(colors.iter().fold(Facet::new("color", t!("shop.color")), |f, c| {
            f.option(c.value.clone(), c.name.clone(), c.count)
        })
        .chosen(p.color.clone()))
        .range(RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
            .money(Currency::Try)
            .value(p.min_price, p.max_price))
        .drives("products");
    let cards = CardGrid::of(search_products)
        .card(|p: &Product| Card::image(p.photo.clone(), p.name.clone()).price(Price::new(p.price)))
        .scope_any(Product::color, p.color.clone())
        .scope_range(Product::price, p.min_price.map(|m| Money::tl(m * 100)), p.max_price.map(|m| Money::tl(m * 100)))
        .listens("products");
    Ok(Screen::new(t!("shop.title")).body(Row::new().col(panel).col(cards).weight(3)))
}
```

- **件数はサーバーが数えます。** 各ファセットはチェックボックスのグループで、選択肢にはほかのファセットを適用したときの件数が表示されます（「赤 (124)」。スクリーンリーダーには「赤、124 件」と読み上げられます）。画面はファセットごとに上限付きのクエリ 1 本で数えます。たとえばほかの条件を `WHERE` に入れた `SELECT color, count(*) ... GROUP BY color LIMIT 50` です。1 つのファセットの選択肢は最大 `MAX_FACET_OPTIONS`（50）個です。件数のない選択肢は、選ばれていなければ無効になります。
- **変更ごとにリクエスト 1 回。** パネルは自分のトピックを動かし、自分でもそれを聞くフィルターフォームです。変更すると値が画面パラメーターに入り（ファセットはリスト、範囲は 2 つの数値）、アドレスが合わせて更新され、1 回の `POST /_zeyn/nodes` ですべてのリスナーが更新されます。カードグリッド、件数の行、新しい件数を持つパネル自身です。新しい変更はまだ途中のリクエストを取り消します。2,000 商品で計測：クリックから新しいカードと件数まで 26 ms。
- **条件はサーバーに残ります。** グリッドは同じパラメーターを `scope_any(field, values)` と `scope_range(field, from, to)`（`CardGrid` と `DataGrid` にあります）で受け取るので、ブラウザーが独自の条件を送ることはありません。値がないか端が開いていれば、すべての行が残ります。
- **選んだ条件**はグループの上にチップとして「すべて解除」と一緒に表示されます。各チップはその条件を外した画面へのリンクなので、スクリプトなしでも動き、名前は「条件 赤 を外す」です。グループは折りたためる `<details>` で、開いたグループは更新後も開いたままです。
- **チェック。** `App::check_page` は何も動かさないパネルと、名前が画面パラメーターのフィールドにないファセットを報告します。

## レビューと評価

```rust
ReviewSummary::new()
    .count(5, stats.five)
    .count(4, stats.four)
    .count(3, stats.three)
    .count(2, stats.two)
    .count(1, stats.one)
```

- **集計。** `ReviewSummary` は平均を数値と星で、レビューの件数、そして星 5 から 1 までそれぞれの割合を示すバーを表示します。平均と割合は件数から計算するので、いつも食い違いません。スクリーンリーダーには「5 つ星のうち 4.6」と、バーごとに「星 5 つ、68%」が読み上げられ、バー自体は隠されます。スクリプトのない HTML です。
- **評価の入力。** フォームのフィールドに付けた `Control::Rating` は星の形で描いたラジオグループです（W3C APG、スクリプトなし）。整数のフィールドは星 1 つ単位、`#[rule(min = 1, max = 5, scale = 1)]` の付いた `Decimal` フィールドは半分単位で、半分ごとに「4.5 of 5」という名前のラジオボタンになります。サーバーはページの数値書式で値を読み、4.3 のように半分の間にある値は受け付けません。

```rust
#[model]
pub struct Review {
    #[rule(required, min = 1, max = 5, scale = 1)]
    pub stars: Option<Decimal>,
    pub text: String,
}

Form::of(Review::default())
    .field(Review::stars)
    .control(Review::stars, Control::Rating)
    .field(Review::text)
    .submit(t!("reviews.send"), save_review)
```
