# 文本与内容

文章、个人资料和列表等阅读页面由几个小组件构成。它们是在服务器端渲染的纯 HTML，不含脚本，每段文本都来自 locale 文件或经过转义。

@live content

## Heading

`Heading::h2(text)` 到 `Heading::h6(text)`。screen 标题是页面唯一的 `h1`，因此这里没有 `h1`：请选择符合大纲层级的级别，而不是你想要的字号。`.id("anchor")` 让链接可以指向它。

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

一段经过转义的文本。`.muted()` 使用次要文本颜色，`.small()` 使用小号字体。长单词会自动换行。

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

一行中的简短信息，例如日期、阅读时长和分类。分隔符（默认是中间点，除非 `.separator("/")` 另行指定）对屏幕阅读器隐藏，空的部分会被省略。

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

指向上一项和下一项的链接，带有 `rel="prev"` 和 `rel="next"`。它们打开时不会重新加载页面。“Previous”和“Next”这两个词来自框架自身的文本；`.prev_title` 和 `.next_title` 在它们下方添加该项的标题。在从右到左的语言中箭头会反转方向，在手机上两个链接会上下堆叠。

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
```

## AutoGrid

把子元素放进一个网格，列数随可用宽度变化：在放得下的范围内尽可能多地排列宽度至少为 `min` 像素（未设置时为 240）的列，在手机上为一列。浏览器用 `repeat(auto-fill, minmax(min(240px, 100%), 1fr))` 进行布局，因此没有断点，也没有脚本。它可以容纳任何节点：卡片、面板、表格。这个名称把它与显示数据行的 `DataGrid` 区分开来。

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

在 `<ul>` 中上下排列的行。每个 `ListItem` 有一个标题，还可以有一个小的前置图片（与卡片图片相同的安全地址：路径、`https:` 或 `data:image/`）、一个副标题以及末尾的简短元信息。使用 `.link(screen, params)` 后整行就是一个链接，因此在 Tab 顺序中只占一个停靠点并带有可见的焦点环，打开时不会重新加载页面。长文本会换行；在手机上元信息移到标题下方，从右到左的页面会镜像整行。

`List::new()` 容纳传给它的项。`List::of(action)` 通过与 DataGrid 相同类型的 action 加载行，并像[卡片网格](catalog.md#card-grid)一样分页：除非 `paging` 另行指定，否则显示 20 行和一个“Show more”按钮，提供排序选项、通过 `.search(text)` 进行的服务器端搜索，并且每一页都检查 action 的访问权限。

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

一个人或一个团队的圆形图片，尺寸为 32、40 或 64 像素（`AvatarSize::Small`、`Medium`、`Large`）。名称是必需的：它是替代文本，也是在没有图片，或图片不在应用的 `img-src` 允许的主机上时（与 `Image` 规则相同）所显示首字母的来源。首字母遵循该语言的大写规则，因此土耳其语的 `i` 会变成 `İ`。当名称就写在头像旁边时，`.decorative()` 会对屏幕阅读器隐藏它，避免名称被读两遍。

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

在读者打开之前一直隐藏的内容，例如问题的答案。它是原生的展开控件（`<details>` 和 `<summary>`），因此无需任何脚本即可配合键盘和屏幕阅读器使用，并且在打开之前内容不在阅读顺序中。标签由应用提供。

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## AlphaIndex

一排字母，每个字母都是指向某个 screen 的链接，参数中带有该字母，例如名字以该字母开头的人。字母按用户语言自身的顺序给出（`zeyn::ui::alphabet`）：土耳其语中 `Ç Ğ I İ Ö Ş Ü` 位于各自的位置，西班牙语有 `Ñ`，俄语和阿拉伯语使用各自的字母表，其他所有语言使用 `A` 到 `Z`。当前所显示 screen 的字母用 `aria-current="page"` 标记（土耳其语的 `i` 会标记 `İ`），`.other()` 会在最后添加一个 `#` 链接，用于以其他字符开头的名称。这些链接打开时不会重新加载页面，在手机上会换成多行。

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

由某个 action 返回的行组成的七个日期列，例如一周的课程、班次或活动。该 action 与 DataGrid 使用的类型相同，检查方式也相同。`.day` 将一行映射到它所属的日期：`Date`、`Option<Date>`（没有日期的行会被省略）或从 1（星期一）到 7（星期日）的星期编号；其他类型无法编译。每一行都显示为一个 `ListItem`，因此可以链接到某个 screen。一周从用户的第一个工作日开始（土耳其语为星期一，美式英语为星期日），今天会被标记，没有行的日期会注明这一点，在窄屏上各天会上下堆叠。服务器按 action 的顺序最多加载 `limit` 行（未设置时为 100，最多 500）；请让 action 只返回这一周的行，例如对日期加一个筛选条件。

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
