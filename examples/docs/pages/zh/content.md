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

### 可排序列表

`.sortable(action)` 让用户按自己的顺序排列 `List::of` 或 `CardGrid` 的行。每行有一个手柄：鼠标或笔直接拖动，触摸在短按之后拖动，一条线显示放下的位置。在获得焦点的手柄上，Space 提起该行，方向键移动它，Space 放下，Escape 放回原处；Alt 加上 Up 或 Down 立即移动。点击或 Enter 打开移动菜单（上移、下移、移到开头、移到末尾），因此单一指针就够用。每一步都会播报。

移动会立即显示，操作收到一个 `ReorderRequest`，其中有该行的键和新邻居的键（`after`、`before`），从不传位置。`r.rank(lookup)` 给出邻居之间的新 `Rank`，并拒绝查找不到的邻居（`NotFound`）或已不在这个顺序的邻居（`Conflict`，有人先移动了行）；这时该行会回到原处并显示原因。源操作按排名列出各行，因此不提供排序选项；在还有行未加载时，任何行都不能移到最后。 拖动的行停在边缘附近时，列表(或页面)会自动滚动。使用 `Paging::virtual_scroll()` 的列表保持虚拟：一条线标出行将落下的位置，服务器保存后各行按新顺序显示。

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

`.connect(group, id)` 把同一屏幕上属于同一组的可排序列表连接起来：行可以从一个列表拖到另一个列表，也可以用移动菜单中的"移动到"移动。操作会收到行进入的列表的 id（`r.list`）和行离开的列表的 id（`r.from`）；两个 id 都来自屏幕而不是浏览器，服务器会检查两个列表操作的权限。`.multi_select()` 让用户按住 Control 或 Command 点击手柄，或用菜单中的"选择以移动"来选择行，一次最多移动 100 行：其余的键在 `r.rest` 中，`r.ranks(lookup)` 为 `r.keys()` 中的每一行给出一个排名。请在行离开的列表中查找被移动的键。虚拟列表不参与这两项功能。

```rust
List::of(todo_tasks)
    .title("To do")
    .item(|t: &Task| ListItem::new(t.title.clone()))
    .sortable(Task::rank, move_tasks)
    .connect("tasks", "todo")
    .multi_select()
```

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

## AvatarGroup

与同一事物相关的人，例如一项任务的负责人，以一排相互重叠的头像显示。标签为屏幕阅读器命名这个组。组内显示前几个头像：除非 `.max(n)` 另有指定，否则为 5 个，且最多 12 个，其余的数量显示为 "+3"，并按用户的语言读作"另外 3 人"。`.size(...)` 设置组内每个头像的尺寸。

```rust
AvatarGroup::new(t!("task.assignees"))
    .add(Avatar::new(a.name.clone()).image(a.photo.clone()))
    .add(Avatar::new(b.name.clone()))
    .max(3)
    .size(AvatarSize::Small)
```

## Quote

引自某人或某部作品的话，以 `figure` 中的 `blockquote` 呈现。`.author(name)` 说明是谁说的，`.source(title)` 给出作品名称，作为标题书写（`cite`）。

```rust
Quote::new(review.text.clone()).author(review.name.clone()).source(t!("shop.verified_buyer"))
```

## Kbd

需要按下的按键，以键盘输入的形式书写（`kbd`）。`Kbd::new("Ctrl+K")` 读取按键之间带 `+` 的组合；`Kbd::keys(["Ctrl", "+"])` 逐个接收按键，用于组合中包含加号键本身的情况。按键名称按给定内容原样书写，最多 6 个。它只显示按键；真正执行操作的快捷键在命令所在的位置设置，就像 `PrevNext::shortcut` 那样。

```rust
Kbd::new("Alt+Shift+P")
```

## Divider

页面两个部分之间的一条线。`Divider::new()` 是一个主题分隔（`hr`）。`.label(text)` 在线的中间放置一段文字，例如两种登录方式之间的"或"；此时分隔线是一个以该文字命名的 `separator`。`.vertical()` 从上到下绘制这条线，用于紧凑 `Row` 的各列之间。

```rust
Divider::new().label(t!("signin.or"))
```

@live pieces

## Highlight

`Highlight::new(text, query)` 输出一段文字，并在其中标记出搜索词（`mark`），例如结果列表中的产品名称。查询按空白字符拆分，最多查找其中 8 个词，并且不区分大小写地标记出其中任何一个词出现的每一处；空查询不标记任何内容。文字和查询都是用户数据：文字会被转义，查询只用于比较，绝不会写入页面。字母按 Unicode 定义的小写形式进行比较，因此土耳其语的 `I ı` 和 `İ i` 这两对字母不会被视为相同。

```rust
Highlight::new(product.name.clone(), p.q.clone())
```

## Toc

目录：指向当前页面各标题的链接，用于文章或政策之类的长页面。`Toc::new(label)` 是一个以标签命名的 `nav`；`.item(title, id)` 链接到通过 `Heading::id` 设置了相同锚点的标题，`.sub(title, id)` 在前一个条目之下一级添加一个条目。锚点的清理方式与 `Heading::id` 相同，因此同一个字符串在两处都可用，清理后锚点为空的条目会被省略；一个目录最多容纳 100 个条目。这些链接是指向 `#anchor` 的普通链接，因此没有脚本也能工作，浏览器会滚动到该标题，地址也可以分享。不会标记正在阅读的部分，因为那需要脚本。

```rust
Toc::new(t!("page.contents"))
    .item(t!("policy.delivery"), "delivery")
    .sub(t!("policy.abroad"), "delivery-abroad")
    .item(t!("policy.returns"), "returns")
Heading::h2(t!("policy.delivery")).id("delivery")
```

@live marks

## Stepper

一个流程所处的位置：按顺序排列的步骤，已完成的、当前的和尚未进行的，例如订单从下单到送达的过程。`.step(title)` 添加一个步骤，`.description(text)` 在最后添加的步骤下方放置一行说明。`.current(index)` 从 0 开始计数，并将其之前的步骤标记为已完成；`.finished()` 将所有步骤标记为已完成。`.vertical()` 将步骤上下排列，在窄屏上也是这样显示。一个 stepper 最多容纳 12 个步骤。

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

这些步骤是一个由标签命名的有序列表。当前步骤带有 `aria-current="step"`，已完成的步骤会按用户的语言向屏幕阅读器读出"已完成"，因此状态不只靠颜色表达。stepper 只显示服务器给它的状态，其中没有任何可以按下的内容；分步填写的表单是带有自己步骤的 `Form`（参见[表单](forms.md)）。

## Pagination

指向列表各页的链接，这个列表由屏幕自己分页，例如博客的文章：上一页、页码和下一页。每个链接都指向同一个屏幕，页码在其参数中，因此服务器渲染被请求的那一页，地址可以分享，浏览器中不保存任何内容。链接通过 `nav.js` 打开而不重新加载页面，并且在没有脚本时也能工作。`DataGrid`、`List` 和 `CardGrid` 通过 `.paging(...)` 自行分页；只有在屏幕自己构建内容的地方才使用它。

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

页码从 1 开始计数。第一页和最后一页始终显示，当前页两侧各显示一页；`.around(n)` 可显示更多，最多 4 页。被省略的页面成为一个间隔，而间隔绝不会只隐藏一页。页码来自地址，因此屏幕在加载数据之前先把它限制在范围内；组件对它所标记的页码也这样做。只有一页或没有页面时，它不渲染任何内容。

这些链接是一个 `nav`，按用户的语言命名为"分页"。当前页带有 `aria-current="page"`，每个页码为屏幕阅读器命名为"第 3 页"，上一页和下一页带有 `rel="prev"` 和 `rel="next"`，在两端它们是标记了 `aria-disabled` 的纯文本，而不是无处可去的链接。

| 按键 | 作用 |
|---|---|
| `Tab` | 在上一页、页码和下一页之间移动 |
| `Enter` | 打开该页；焦点和滚动位置由 `nav.js` 处理 |

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


## CopyButton

把一段文字复制到剪贴板的按钮：运单号、API 地址、邀请链接。`.shown()` 用等宽字体把文字写在按钮旁边，这样也可以阅读和选择它；不使用它时，文字在页面中但被隐藏。除非 `.label` 另行设置，否则标签是用户语言中的"复制"，并由一个 toast 说明复制是否成功。它使用与 `Share` 相同的小脚本，这个脚本只在包含此类按钮的页面上加载；按钮以隐藏状态渲染并由该脚本显示，因此没有脚本的页面上不会有一个什么都不做的按钮。文字在 4096 字节处被截断，并且由于它是页面的一部分，它必须是用户可以看到的内容，绝不能是被掩码的值。

```rust
CopyButton::new(order.tracking.clone()).shown()
CopyButton::new(invite_url).label(t!("team.copy_invitation"))
```

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
