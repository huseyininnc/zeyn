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
```

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

`<ul>` の中に行を縦に並べます。各 `ListItem` にはタイトルがあり、先頭の小さな画像（カード画像と同じ安全なアドレス：パス、`https:`、`data:image/`）、サブタイトル、末尾の短いメタテキストを付けることもできます。`.link(screen, params)` を使うと行全体が 1 つのリンクになり、タブ順では 1 か所、フォーカスリングは見え、ページを再読み込みせずに開きます。長いテキストは折り返され、スマートフォンではメタテキストがタイトルの下に移り、右から左に書くページでは行が左右反転します。

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
            .meta(p.joined.format(&locale))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name, false)
    .paging(Paging::pages(20))
```

@live list

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
