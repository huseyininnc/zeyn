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
    .shortcut("Alt+Shift+N")
```

### 链接快捷键

`.shortcut(keys)` 为最后设置的一侧链接指定键盘快捷键，`Chips::links(...).link(...).shortcut(keys)` 则为最后添加的链接 chip 指定快捷键。按下这些键会通过 `nav.js` 跟随该链接，无需重新加载页面，应用 shell 和站点 shell 页面均适用。焦点位于输入框、文本区域、下拉选择框或可编辑区域时，这些键不起作用，因此输入永远不会被抢占。链接带有供屏幕阅读器使用的 `aria-keyshortcuts`，并以 `<kbd>` 提示显示按键，触屏设备会隐藏该提示。快捷键的解析方式与 `Command::shortcut` 相同：浏览器保留（`Ctrl+T`、`Ctrl+W`）或使用（`Ctrl+P`、`F5`）的快捷键不会被绑定。`App::check` 会报告站点头部插槽中的链接快捷键存在以下问题：无法解析、属于浏览器、是单字符按键（WCAG 2.2，2.1.4：`Shift+N` 算，`Alt+Shift+N` 不算），或与命令、命令面板或其他链接冲突；`App::check_page(&screen)` 对在测试中带参数构建的 screen 做同样的检查，因为 `App::check` 无法自己构建 screen。

当焦点位于来自其他源的框架内（例如嵌入的视频播放器）时，按键会交给该框架的文档，永远不会到达页面；任何脚本都无法改变这一点。请告诉用户在使用快捷键之前，先点击框架之外，或按 Tab 离开该框架。

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

一行接一行排列，位于一个 `<ul>` 中。每个 `ListItem` 有一个标题，还可以有一张开头的小图片（与卡片图片相同的安全地址：路径、`https:` 或 `data:image/`）、一个副标题和末尾的简短附注：文本，或 `RelativeTime` 这样的行内组件。使用 `.link(screen, params)` 时，整行就是一个链接，因此在 Tab 顺序中只占一个位置，有可见的焦点环，并且无需重新加载页面即可打开。长文本会换行；在手机上附注文本移到标题下方，从右到左的页面中该行会镜像。

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
            .meta(RelativeTime::new(p.joined))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name)
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

## Chips

一排可换行的圆角小链接。`Chips::links(label)` 在以 label 命名的 `nav` 中放置指向屏幕的普通链接，例如文章的标签。`Chips::filter(label, screen, params)` 基于屏幕参数生成切换按钮：每个 chip 都是指向同一屏幕的链接，其值会被加入或移出参数，因此服务器根据收到的参数渲染 `aria-pressed`，浏览器中不保存任何状态。按下 chip 会通过 `nav.js` 在不刷新页面的情况下打开新地址，焦点和滚动位置保持在该 chip 上。除非调用 `.multiple()`，否则同一时间只能按下一个值；参数中不属于选项的值会被忽略。键盘：Tab 在 chip 之间移动，Enter 或 Space 按下其中一个，链接 chip 还可以有自己的快捷键（参见[链接快捷键](#link-shortcuts)）。被按下的 chip 还会显示对勾，因此状态不只依赖颜色。脚本（`chips.js`，小于 1 KB）只在带有筛选 chip 的页面上加载。

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

页面顶部的大型区块：背景图，加上标题、一行文字和按钮等操作。`.link(label, screen, params)` 添加指向其他页面的 `LinkButton`：每张幻灯片的第一个采用主要样式，之后的采用次要样式。默认是静态的。`.slide(title)` 会添加另一张幻灯片，而 `.text`、`.image` 和 `.action` 始终作用于最后一张。有多张幻灯片时，它遵循 W3C APG 轮播模式：幻灯片选择按钮，每张幻灯片是名为“2 / 3”的分组，被选中的幻灯片会以礼貌方式播报。除非应用调用 `.rotate(seconds)`（4 到 30），幻灯片只在用户选择时切换。调用后，暂停与播放按钮排在最前（WCAG 2.2.2），指针位于 hero 上、焦点位于其中以及标签页隐藏时轮播会停止，用户偏好减少动态效果时则完全不会启动；幻灯片在切换过程中不会被播报。没有脚本时显示第一张幻灯片。

图片遵循 `Image` 的规则（应用的路径，或位于应用 `img-src` 允许的主机上的 `https` 地址）。它是文字后面的装饰，因此替代文本为空：重要内容请写入标题和文字，它们位于深色遮罩之上，该遮罩可保证白色文字在任何图片上的对比度至少为 4.5:1。链接按钮保持其实心颜色，并在图片上带有白色焦点环。第一张幻灯片的图片以高优先级加载以加快首次绘制，其他的则延迟加载。脚本（`hero.js`，1.6 KB gzip）只在带有幻灯片的页面上加载。

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

由应用选定的一排反应按钮，每个按钮有一个 key、一个名称和一个符号（如 emoji），以及计数。`Reactions::of(action, target)` 指明这些反应针对哪个条目；target 由屏幕设定，绝不从浏览器读取。按下按钮会把反应提交到 `POST /_zeyn/data`，该端点会检查 CSRF，计入写入速率限制，打开屏幕，检查操作的权限，并在操作运行之前拒绝集合之外的反应。操作会收到 `ReactionPress`（target、reaction、pressed），将其保存并返回新的 `ReactionState`（各计数和用户自己的反应），计数和 `aria-pressed` 状态就地更新。每个按钮都是以反应名称和计数命名的 toggle button；符号对屏幕阅读器隐藏。`.state(...)` 提供首次渲染时的计数，因此无需脚本也能显示。脚本（`reactions.js`，1.3 KB gzip）只在带有反应的页面上加载。

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

以页面语言显示的相对时间，例如“3 分钟前”或“2 天后”。服务器把用户时区下的准确日期和时间写入 `<time datetime="...">`，因此 HTML 每分钟都保持不变，缓存的页面依然正确。运行时会在页面加载时、`nav.js` 换入屏幕时以及每分钟用 `Intl.RelativeTimeFormat` 重写文本；准确时间保留在提示中。没有脚本时，文本就是准确时间。

```rust
RelativeTime::new(post.published)
```


## Share

分享链接的按钮：浏览器支持 Web Share API 时（大多数手机）使用系统分享面板，否则复制到剪贴板并用 toast 提示。它分享当前显示的页面，或通过 `Share::screen(screen, params)` 分享某个屏幕，其参数由编译器检查。`.title` 设置分享面板显示的标题；除非用 `.label` 指定，标签为用户语言的“分享”。服务器把按钮渲染为隐藏状态，由其脚本显示，因此没有脚本的页面不会出现什么都不做的按钮。脚本（`share.js`，小于 1 KB）只在包含分享按钮的页面上加载。

```rust
Share::page().title(post.title.clone())
Share::screen(article, ArticleParams { id: post.id }).label(t!("blog.share_article"))
```

@live engage


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
