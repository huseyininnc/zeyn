# 文本与内容

文章、个人资料和列表这类阅读页面由几个小组件构成。它们是在服务器上渲染的纯 HTML，不含脚本，所有文本都来自 locale 文件或经过转义。

@live content

## Heading

`Heading::h2(text)` 到 `Heading::h6(text)`。screen 标题是页面唯一的 `h1`，所以这里没有 `h1`：应按大纲的层级选择级别，而不是按想要的字号。`.id("anchor")` 让链接可以指向它。

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

一段经过转义的文本。`.muted()` 使用次要文字颜色，`.small()` 使用小字号。长单词会换行。

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

在一行内显示简短的信息，例如日期、阅读时长和分类。分隔符（除非用 `.separator("/")` 指定，否则是间隔号）对屏幕阅读器隐藏，空的部分会被省略。

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

指向上一项和下一项的链接，带有 `rel="prev"` 和 `rel="next"`。它们无需重新加载页面即可打开。“上一页”和“下一页”这两个词来自框架自带的文本；`.prev_title` 和 `.next_title` 会在它们下方添加该项的标题。在从右到左的语言中箭头会反向，在手机上两个链接会上下堆叠。

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
```

## AutoGrid

子项排列在列数随可用宽度变化的网格中：每列至少 `min` 像素（未设置时为 240），能放下几列就放几列，手机上为一列。浏览器用 `repeat(auto-fill, minmax(min(240px, 100%), 1fr))` 完成布局，因此没有断点，也没有脚本。它可以容纳任何节点：卡片、面板、网格。这个名称是为了与显示数据行的 `DataGrid` 区分开。

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

一行接一行排列，位于一个 `<ul>` 中。每个 `ListItem` 有一个标题，还可以有一张开头的小图片（与卡片图片相同的安全地址：路径、`https:` 或 `data:image/`）、一个副标题和末尾的简短附注文本。使用 `.link(screen, params)` 时，整行就是一个链接，因此在 Tab 顺序中只占一个位置，有可见的焦点环，并且无需重新加载页面即可打开。长文本会换行；在手机上附注文本移到标题下方，从右到左的页面中该行会镜像。

`List::new()` 保存传给它的项。`List::of(action)` 通过与 DataGrid 相同类型的 action 加载行，并像[卡片网格](catalog.md#card-grid)一样分页：除非 `paging` 另有设置，否则每次 20 行并带有“Show more”按钮，还有排序选项、用 `.search(text)` 实现的服务器端搜索，并且每一页都会检查该 action 的访问权限。

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

一个人或一个团队的圆形头像，尺寸为 32、40 或 64 像素（`AvatarSize::Small`、`Medium`、`Large`）。名称是必填的：它是替代文本，也是没有图片、或图片不在应用的 `img-src` 允许的主机上时显示的首字母的来源（规则与 `Image` 相同）。首字母遵循该语言的大写规则，因此土耳其语的 `i` 会变成 `İ`。当名称就写在头像旁边时，`.decorative()` 会对屏幕阅读器隐藏头像，避免名称被读两遍。

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

在读者打开之前处于隐藏状态的内容，例如问题的答案。它是原生的展开控件（`<details>` 和 `<summary>`），因此无需任何脚本即可通过键盘和屏幕阅读器使用，并且内容在打开之前不在阅读顺序中。标签由应用提供。

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## AlphaIndex

一排字母，每个字母都是指向某个 screen 的链接，字母位于该 screen 的参数中，例如名字以该字母开头的人。字母来自用户的语言，并按该语言自己的顺序排列（`zeyn::ui::alphabet`）：土耳其语中 `Ç Ğ I İ Ö Ş Ü` 各在其位，西班牙语有 `Ñ`，俄语和阿拉伯语有各自的字母表，其他语言一律为 `A` 到 `Z`。当前显示的 screen 的字母用 `aria-current="page"` 标记（土耳其语的 `i` 标记 `İ`），`.other()` 会在末尾添加一个 `#` 链接，用于以其他字符开头的名称。这些链接无需重新加载页面即可打开，在手机上会折成多行。

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

七个按天划分的列，其中的行来自某个 action，例如一周的课程、班次或活动。这个 action 与 DataGrid 使用的类型相同，检查方式也相同。`.day` 把一行映射到它所在的日子：可以是 `Date`、`Option<Date>`（没有日期的行会被忽略）或从 1（星期一）到 7（星期日）的星期几数字；其他类型无法通过编译。每一行都以 `ListItem` 显示，因此可以链接到某个 screen。一周从用户的第一个工作日开始（土耳其语为星期一，美式英语为星期日），今天会被标记，没有行的日子会注明，在窄屏幕上各天上下堆叠。服务器按 action 的顺序最多加载 `limit` 行（未设置时为 100，最多 500）；请让 action 只返回这一周的行，例如对日期加过滤条件。

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
