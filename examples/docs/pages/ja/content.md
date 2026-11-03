# テキストとコンテンツ

記事、プロフィール、一覧のような読むためのページは、いくつかの小さなコンポーネントで組み立てます。これらはサーバー上でレンダリングされる素の HTML でスクリプトを持たず、すべてのテキストはロケールファイルを通るか、エスケープされます。

@live content

## Heading

`Heading::h2(text)` から `Heading::h6(text)` までです。画面のタイトルがページで唯一の `h1` なので、ここには `h1` はありません。欲しい大きさではなく、文書の構成に合うレベルを選んでください。`.id("anchor")` を使うと、リンクから見出しを指せます。

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

エスケープされたテキストの段落です。`.muted()` は補助的なテキストの色を、`.small()` は小さいフォントサイズを使います。長い単語は折り返されます。

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

日付、読了時間、カテゴリーのような短い情報を 1 行に並べます。区切り文字（`.separator("/")` で別の指定をしない限り中黒）はスクリーンリーダーから隠され、空の部分は省かれます。

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

`rel="prev"` と `rel="next"` 付きの、前の項目と次の項目へのリンクです。ページを再読み込みせずに開きます。「Previous」と「Next」という語はフレームワークのテキストから来ており、`.prev_title` と `.next_title` はその下に項目のタイトルを追加します。右から左に書く言語では矢印の向きが反転し、スマートフォンでは 2 つのリンクが縦に積まれます。

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
```

## AutoGrid

使える幅に応じて列数が決まるグリッドに子要素を並べます。幅が少なくとも `min` ピクセル（指定しなければ 240）の列を収まるだけ並べ、スマートフォンでは 1 列になります。ブラウザーが `repeat(auto-fill, minmax(min(240px, 100%), 1fr))` でレイアウトするので、ブレークポイントもスクリプトもありません。カード、パネル、グリッドなど、どのノードでも入れられます。この名前は、データの行を表示する `DataGrid` と区別するためのものです。

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

`<ul>` の中で縦に並ぶ行です。各 `ListItem` にはタイトルがあり、先頭の小さな画像（カード画像と同じ安全なアドレス: パス、`https:` または `data:image/`）、サブタイトル、末尾の短いメタテキストを持てます。`.link(screen, params)` を使うと行全体が 1 つのリンクになるので、タブ順では見えるフォーカスリング付きの 1 つの停止位置になり、ページを再読み込みせずに開きます。長いテキストは折り返され、スマートフォンではメタテキストがタイトルの下に移り、右から左のページでは行が左右反転します。

`List::new()` は渡された項目を保持します。`List::of(action)` は DataGrid が使うのと同じ種類のアクションで行を読み込み、[カードグリッド](catalog.md#card-grid)と同じようにページ分けします。`paging` で別の指定をしない限り「Show more」ボタン付きで 20 行を表示し、並べ替えの選択肢、`.search(text)` によるサーバー側の検索があり、ページごとにアクションのアクセス権を検査します。

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

人やチームを表す 32、40、64 ピクセルの丸い画像です（`AvatarSize::Small`、`Medium`、`Large`）。名前は必須です。名前は代替テキストであり、画像がないとき、または画像がアプリの `img-src` で許可されたホストにないとき（`Image` と同じ規則）に表示されるイニシャルの元になります。イニシャルはその言語の大文字の規則に従うので、トルコ語の `i` は `İ` になります。名前がアバターのすぐ横に書かれている場合は、`.decorative()` でスクリーンリーダーから隠し、名前が 2 回読まれないようにします。

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

質問の答えのように、読者が開くまで隠れているコンテンツです。ネイティブの開閉要素（`<details>` と `<summary>`）なので、スクリプトなしでキーボードとスクリーンリーダーで動作し、開くまでコンテンツは読み上げ順に含まれません。ラベルはアプリが用意します。

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## AlphaIndex

文字の並んだバーで、各文字はその文字をパラメーターに持つ画面へのリンクです。たとえば名前がその文字で始まる人の一覧です。文字はユーザーの言語から、その言語自身の順序で並びます（`zeyn::ui::alphabet`）。トルコ語では `Ç Ğ I İ Ö Ş Ü` が本来の位置にあり、スペイン語には `Ñ` があり、ロシア語とアラビア語はそれぞれのアルファベットを使い、その他の言語はすべて `A` から `Z` になります。表示中の画面の文字は `aria-current="page"` で示され（トルコ語の `i` は `İ` を示します）、`.other()` はそれ以外の文字で始まる名前のために最後に `#` リンクを追加します。リンクはページを再読み込みせずに開き、スマートフォンでは複数行に折り返されます。

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

1 週間の授業、シフト、イベントのように、アクションから得た行を 7 つの曜日の列に並べます。アクションは DataGrid が使うのと同じ種類で、同じように検査されます。`.day` は行をその曜日に対応させます。`Date`、`Option<Date>`（日付のない行は省かれます）、または 1（月曜日）から 7（日曜日）までの曜日番号を使え、それ以外はコンパイルできません。各行は `ListItem` として表示されるので、画面へリンクできます。週はユーザーの週の最初の曜日（トルコ語では月曜日、アメリカ英語では日曜日）から始まり、今日には印が付き、行のない日はその旨を表示し、狭い画面では曜日が縦に積まれます。サーバーはアクションの順序で最大 `limit` 行（指定しなければ 100、最大 500）を読み込みます。たとえば日付にフィルターをかけて、その週の行をアクションに渡してください。

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
