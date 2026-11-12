# 展示组件

用于展示状态的组件：消息、徽标、进度、键值对、历史记录和占位内容。它们在服务器端渲染，不需要自己的请求。

```rust
Stack::new()
    .add(Alert::info("Imports run in the background.").dismissible())
    .add(Alert::warning("Three products are low on stock.").title("Stock"))
    .add(Row::new().compact().col(Badge::new("Paid").tone(Tone::Success)).col(Badge::new("Overdue").tone(Tone::Danger)).col(Badge::new("Draft")))
    .add(Progress::new("Import", 64, 100))
    .add(Timeline::new()
        .event(ctx.now().add_ms(-26 * 3_600_000), "Created", None, Tone::Info)
        .event(ctx.now().add_ms(-3_600_000), "Approved", Some(Text::from("by the manager")), Tone::Success))
    .add(Skeleton::lines(2))
```

@live display

## Alert

`Alert::info`、`success`、`warning` 和 `danger`，或者 `Alert::new(tone, text)`。`.title(text)` 添加标题，`.dismissible()` 添加关闭按钮。语气（tone）也会在消息之前被朗读出来，因此信息绝不只靠颜色传达。

## Badge

`Badge::new(text).tone(Tone::...)`。可用的 tone：`Neutral`、`Info`、`Success`、`Warning`、`Danger` 和 `Brand`。带色调的徽标以该色调的文字显示在该色调的淡色背景上；`Neutral` 是带一道轮廓的弱化标签，用于套餐之类的普通值。`.dot()` 在普通文字前绘制一个 8 px 的该色调圆点，是列表中表示状态的更低调形式；`.count()` 为数字绘制一个圆形胶囊。表格通过 `.badge(|v| tone)` 将值显示为徽标，通过 `.status(|v| tone)` 显示为圆点。

## Progress

`Progress::new(label, value, max)`，可选 `.tone(...)`。它是一个真正的 `progressbar`，带有标签和数值。

## ProgressRing

`ProgressRing::new(label, value, max)` 把同样的进度显示为一个圆环，中间是百分比，下方是标签，用于在很小的空间里表示整体中的一部分：已用的配额、已填写的资料。`.tone(...)` 为圆环着色，`.size(px)` 设置其直径，未设置时为 64，并保持在 32 到 160 之间。圆环是由服务器绘制的内联 SVG，因此不需要脚本，在任何像素密度下都保持清晰。它是一个由其标签命名的真正的 `progressbar`；超过最大值的值按最大值计算。

```rust
ProgressRing::new(t!("account.storage_used"), used, quota).tone(Tone::Warning).size(96)
```

@live rings

## Meter

`Meter::new(label, value, min, max)` 显示已知范围内的一个测量值，例如已用的磁盘空间或库存水平。与 `Progress` 不同，它不是一个走向终点的任务：数值可以上升也可以下降，`.low(n)` 和 `.high(n)` 标出数值从哪里开始算偏低或偏高。进度条的颜色取自这个状态：没有设置方向时，两个阈值之间的范围是良好的，两端都是警告；`.better(Better::Higher)` 让高数值为好、低数值为差，`Better::Lower` 则相反。状态绝不只靠颜色表达：数值旁边会用用户的语言写出"偏低"或"偏高"，这也是屏幕阅读器从 ARIA `meter` 读到的内容的一部分。数值是整数，按用户语言的方式分组，`.unit(text)` 添加一个单位。

```rust
Meter::new(t!("server.disk_used"), used_gb, 0, total_gb)
    .high(total_gb * 8 / 10)
    .better(Better::Lower)
    .unit("GB")
```

## Indicator

`Indicator::on(node)` 在另一个组件的角上放置一个计数或一个圆点：头像上的未读消息数、链接上的新内容提示。`.count(n)` 显示数字，超过 99 时显示 "99+"，为零时什么都不显示；`.dot()` 显示一个圆点；`.tone(...)` 设置颜色，未设置时为 `Danger`。标记本身对屏幕阅读器是隐藏的，因为名字后面一个单独的 "3" 说明不了什么：`.label(text)` 给出它们代之读出的文字，例如"3 条未读消息"。只要标记带有其下方组件尚未表达的含义，就应当提供标签；没有标签时，计数按数字读出，圆点则不读出。

```rust
Indicator::on(Avatar::new(user.name.clone()))
    .count(unread)
    .label(t!("inbox.unread_count").arg("n", unread.to_string()))
```

@live meters

## DescriptionList

键值对，可以来自记录的字段，也可以手动添加：

```rust
DescriptionList::of(&invoice)
    .field(Invoice::customer)
    .field(Invoice::issued)
    .item(t!("ui.status"), "INV-2026-0042")
```

值的格式与表格单元格相同：金额、日期和选项都按用户的语言显示。

## Timeline

`Timeline::new().event(moment, title, detail, tone)` 列出事件。时间点按用户的时区和语言显示。

## EmptyState 和 Skeleton

`EmptyState::new(title).text(text)` 用下一步操作填充空白区域：`.link(label, screen, params)` 通过 `LinkButton` 前往另一个页面，`.opens(label, screen, params)` 则以窗口形式打开应用的一个 screen。`Skeleton::lines(n)` 是内容加载期间的占位。

## LinkButton

去往某处用链接，执行某事用按钮。`LinkButton::new(label, screen, params)` 是指向该 screen 规范路径（例如 `/products/42`）的真正的 `<a href>`，外观像按钮，因此中键点击、“在新标签页中打开”和搜索引擎都把它当作链接处理。它通过 `nav.js` 打开，无需重新加载页面，按 Enter 即可跟随。除非用 `.primary()` 要求主要操作的实心样式，否则外观为次要样式。该 screen 必须有路由，否则无法编译。`EmptyState::link` 和 `Hero::link` 使用它。

```rust
LinkButton::new(t!("cart.browse"), products, ()).primary()
```

## ClearPrefs

`ClearPrefs::new(label)` 是一个按钮，用于移除此浏览器为该应用保存的所有客户端偏好，例如放在隐私页面中，紧挨着 `App::client_prefs_table()` 生成的列表。会有一条 toast 提示确认。服务器设置的 cookie（主题、语言、会话）不受影响。脚本可以用 `zeyn.prefs.clear()` 做同样的事。

## Breadcrumb

`Breadcrumb::new().link(label, screen, params).current(label)`：链接是带类型的 screen 引用，因此重命名 screen 后不会留下失效链接。

## ActionButton

`ActionButton::new(label, action, input)` 运行一个 action。`.confirm(text)` 先进行确认，`.tone(Tone::Danger)` 标记破坏性操作，`.on_success(|result, ui| ...)` 运行 effect。input 在服务器端固定，绝不来自客户端；action 的权限与任何表单一样在服务器端检查。 `.tone(Tone::Brand)` 将其绘制为所在区域（例如卡片）的主要操作。

```rust
ActionButton::new(t!("docs.live.count"), count_samples, ())
    .on_success(|n, ui| {
        if let Ok(n) = n {
            ui.toast(t!("docs.live.counted", n = n));
        }
    })
```

@live action

## Markdown

`Markdown::new(text)` 显示格式化文本，例如评论、产品描述或帮助内容。文本可以是字符串或 locale 键。它需要 `markdown` feature（`zeyn = { features = ["markdown"] }`），不会引入任何依赖。

```rust
Markdown::new(product.description.clone())
Markdown::new(t!("help.import")).trusted()
```

- 支持标题、段落、列表、表格、代码块、引用、分隔线、行内代码、链接、图片、粗体和斜体。
- 原始 HTML 绝不会被直接输出：`<script>` 显示为文本，代码块同样会被转义。
- 链接只能指向 `http`、`https`、`mailto`、锚点或相对路径。其他链接（例如 `javascript:` 或 `data:`）只保留其标签文字。用户提供的链接会带上 `rel="noopener noreferrer nofollow ugc"`。
- 图片只从应用自身加载（相对路径）；其他图片显示其替代文本。
- `.trusted()` 用于应用自己编写的文本：标题会获得 id 以便链接跳转，链接不再带有 `nofollow ugc`。转义和链接规则保持不变。
- 超过 64 KB（`MAX_MARKDOWN`）的文本会被截断，嵌套深度也有上限，因此恶意输入的渲染成本依然很低。

## Embed

`Embed` 在锁定的框架中显示来自其他站点的页面，例如视频播放器或地图。标题是必需的（屏幕阅读器通过它来播报框架）；没有 `.title(...)` 的 embed 无法放置，编译器会给出提示。

```rust
Embed::url("https://www.youtube-nocookie.com/embed/abc")
    .title(t!("tour.title"))
    .allow([EmbedPermission::Fullscreen, EmbedPermission::PictureInPicture])
    .aspect(16, 9)
    .click_to_load("/static/tour.jpg")
```

- 主机必须在应用的 `frame-src` 中：`App::csp(|c| c.frame_src(["https://www.youtube-nocookie.com"]))`（参见[安全](security.md#csrf-csp-and-headers)）。当地址不是 `https` 或其主机未列出时，根本不会渲染框架；会有一条警告说明内容无法显示。
- 每个框架都会带上 `sandbox="allow-scripts allow-same-origin allow-presentation"`、`referrerpolicy="no-referrer"` 和 `loading="lazy"`。弹出窗口、顶层导航、表单和下载永远不能被允许。`.allow(...)` 添加浏览器功能：`Fullscreen`、`PictureInPicture`、`Autoplay` 和 `EncryptedMedia`。
- `.click_to_load(poster)` 先显示一个按钮，带有海报图片（应用上的路径或 `img-src` 中的主机；传 `""` 表示没有）以及对方站点的名称。在按下按钮之前不会联系对方站点；按下后框架取代按钮并获得焦点。该脚本是 `media.js` 的一部分，因此不会产生额外请求。
- `.notice(t!(...))` 在该按钮上放置应用自己的文本，例如内容警告。它会被转义，与所有文本一样可翻译，并且是按钮可访问名称的一部分，因此屏幕阅读器会在用户做出决定之前读出它。没有 `.click_to_load` 时，它会启用无海报的按钮。
- `.remember(EXTERNAL_VIDEO)` 在该键的有效期内只询问一次，而不是每个页面都问。该键是用 `ClientPref::consent` 声明并通过 `App::client_prefs` 注册的同意键（参见[客户端偏好设置](security.md#client-preferences)）；绑定到同一个键的 embed 共用该答案。之后的页面、应用内导航和其他标签页会自行加载这些框架，且不移动焦点（WCAG 2.2 3.2.1 获得焦点时、2.4.3 焦点顺序）；只有在真正按下之后焦点才会移到框架。以下情况按钮会重新出现：记录已结束或未通过检查；应用没有注册该键；存储被阻止且页面是新的；以及没有 JavaScript 时。普通字符串或选择（choice）键无法编译。
- 保存待嵌入地址的字段使用 `#[rule(embed)]`。表单接受 URL 或粘贴的嵌入代码，只保留 `src`，要求 `https` 和已列出的主机，丢弃跟踪参数（`utm_*`、`fbclid`、`gclid` 等），并保存规范地址，绝不保存 HTML。API 输入必须发送规范地址。

```rust
#[entity(table = "videos")]
pub struct Video {
    #[key]
    pub id: VideoId,
    #[rule(required, embed)]
    pub url: String,
}
```

## Image

`Image` 需要替代文本和尺寸；没有 `.alt(...)` 和 `.size(width, height)` 就无法放置，编译器会给出提示。尺寸会在文件加载期间保留空间，因此页面不会跳动。

```rust
Image::new("https://images.example.com/logo.png")
    .alt(t!("brand.logo"))
    .size(240, 80)
    .lazy()
    .srcset([("https://images.example.com/logo@2x.png", "2x")])
```

- `src` 是应用上的路径（`/static/logo.png`），或位于应用 `img-src` 主机上的 `https` 地址（`App::csp(|c| c.img_src([...]))`）。其他任何图片都不会被渲染，而是显示其替代文本，因此浏览器绝不会记录被拦截的请求。
- 每张图片都带有 `decoding="async"`；`.lazy()` 为首屏以下的图片添加 `loading="lazy"`。`srcset` 接受密度（`2x`）或宽度（`800w`）描述符；位于其他主机或使用其他描述符的条目会被省略。宽度描述符会根据图片宽度添加 `sizes`。
- 纯装饰性图片使用空的替代文本：`.alt(Text::from(""))`。

### 长列表中的照片

对于拥有数千张商品照片的目录，图片可以在加载期间先显示内容，并在加载失败时恢复。这些都使用浏览器的原生功能；媒体模块的一个监听器服务页面上的所有图片。

```rust
Image::new(p.photo_800.clone())
    .alt(p.name.clone())
    .size(400, 500)
    .lazy()
    .srcset([(p.photo_400.as_str(), "400w"), (p.photo_800.as_str(), "800w")])
    .sizes("(min-width: 800px) 25vw, 50vw")
    .placeholder(Placeholder::color("#d9dde3"))
    .low(p.photo_40.clone())
    .fallback("/static/no-photo.webp")
    .on_error(t!("shop.no_photo"))
    .fit(Fit::Cover)
```

- `.placeholder(...)` 在文件加载期间填充图片框，无需脚本：`Placeholder::color("#d9dde3")` 或主题 token（`"--z-surface-2"`）；`Placeholder::blur(data_uri)` 用于应用生成的极小图片（最多 600 字节的 base64 `data:image/webp`、`avif`、`jpeg` 或 `png` URI，绝不能是 SVG）；`Placeholder::image(src)` 用于所有图片共用的一个文件。违反规则的占位符会被省略。
- `.low(src)` 立即在完整文件下方显示一个小的低分辨率文件。完整文件同时加载（图片为懒加载时也懒加载），准备好后淡入；在减少动态效果的设置下，它直接出现，不带淡入。
- `.fallback(src)` 在图片加载失败时尝试另一个文件一次。之后显示 `.on_error(text or component)`；没有设置时，在占位符上显示替代文本和一个图标。不会再次重试。
- `.priority()` 标记访客看到的第一张图片（最大内容绘制）：`loading="eager"` 和 `fetchpriority="high"`，绝不懒加载。每个页面只使用一次。
- `.sizes(...)` 说明图片在布局中的宽度，使浏览器选择合适的 `srcset` 宽度。没有它时，懒加载图片会得到 `sizes="auto"`，由浏览器测量所在位置。
- `.fit(Fit::Cover)` 填满图片框并裁掉溢出部分；`Fit::Contain` 显示整个文件。
- `low`、`fallback` 和 `Placeholder::image` 遵循与 `src` 相同的地址规则；其他地址会被省略。图片框始终保持图片的尺寸，因此文件加载或失败时页面不会跳动。

## ImageCompare

两张尺寸相同的图片叠放在一起，用一个滑块让其中一张覆盖在另一张上露出，例如前后对比。`.before(src, alt)` 和 `.after(src, alt)` 给出图片，它们遵循 `Image` 的规则，`.size(width, height)` 决定框的形状。滑块是一个以标签命名的范围输入框，因此可以用方向键移动；一个只在包含此组件的页面上加载的小脚本会显示它，没有脚本时两张图片各显示一半。

```rust
ImageCompare::new(t!("repair.before_after"))
    .before(job.before.clone(), t!("repair.before"))
    .after(job.after.clone(), t!("repair.after"))
    .size(1200, 800)
```

## Tour

页面的分步导览。`Tour::new(label)` 是开始导览的按钮，`TourStop::on(node).title(text).body(text)` 标记导览停留的组件；站点原样绘制它的组件。导览按页面顺序访问可见的站点，最多 30 个，使用一个带有“上一步”“下一步”和“关闭”的小对话框；按 Escape 关闭它，焦点回到按钮。只有包含导览的页面才加载的一小段脚本负责显示按钮，因此没有脚本时页面保持原样。

```rust
Tour::new(t!("orders.tour"))
TourStop::on(orders_grid)
    .title(t!("orders.tour.grid"))
    .body(t!("orders.tour.grid_text"))
```

@live tour

## ExternalLink

`ExternalLink::new(url, text)` 以 `rel="noopener noreferrer"` 链接到其他站点，因此对方站点既得不到页面地址，也无法获得窗口的句柄。屏幕阅读器会在文字之后读出“外部站点”；视觉上用箭头标记。

```rust
ExternalLink::new("https://example.com/help", t!("help.center")).new_tab()
ExternalLink::new(customer.website.clone(), t!("customer.website")).user_content()
```

`.user_content()` 为用户输入的地址添加 `nofollow ugc`。`.new_tab()` 在新标签页中打开并告知屏幕阅读器；未设置时链接在同一标签页中打开。只有 `https` 和 `http` 地址会成为链接；其他任何内容（例如 `javascript:`）都显示为纯文本。

## 工具提示、弹出层和忙碌遮罩

工具提示是对单个控件的简短说明：`ActionButton::tip(text)`、screen 工具栏中的 `ToolButton::tip(text)`，以及 `Form::tip(field, text)`，后者会在字段标签之后添加一个信息按钮。该文本是控件的 `aria-describedby`，因此屏幕阅读器会随控件一起读出。它在悬停片刻后显示，获得焦点时立即显示，指针停留其上时保持显示（WCAG 1.4.13），按 Escape 隐藏且不移动焦点，在触屏上长按（点按信息按钮）后显示。工具提示绝不承载用户必须获知的信息：这类信息应放在标签、字段规则或页面中。

`Popover::new(label).title(title).add(node)` 是一个按钮，会在其旁边打开一个非模态对话框，用于显示少量细节。焦点会移入其中；按 Escape、关闭按钮或点击外部都会将其关闭，焦点回到按钮。在窄屏幕上它以底部面板的形式打开。它的子元素是普通节点，因此其中的表单会以每项服务器检查提交到自己的节点路径。

`Busy::new().add(node)` 在其内部的请求运行期间覆盖其内容：表单保存、操作按钮、表格或树加载。容器会立即获得 `aria-busy`；加载指示器和文字“Working”（状态消息）仅在 400 毫秒后（或 `.delay_ms(n)`）才显示，因此快速响应不会闪烁，显示后至少保持 300 毫秒。遮罩阻挡指针，不阻挡键盘，焦点保持原位。

```rust
Stack::new()
    .add(ActionButton::new(t!("docs.live.count"), count_samples, ()).tip(t!("docs.live.count_tip")))
    .add(Popover::new(t!("docs.live.rules")).title(t!("docs.live.rules_title")).add(rules_list))
    .add(Busy::new().add(ActionButton::new(t!("docs.live.slow"), slow_count, ())))
```

@live overlay

| 按键 | 位置 | 作用 |
|---|---|---|
| Tab | 带工具提示的控件 | 获得焦点时显示工具提示；离开时隐藏 |
| Escape | 工具提示显示期间的任何位置 | 隐藏工具提示；焦点保持不动 |
| Enter、Space | 弹出层按钮 | 打开弹出层；焦点移到其第一个控件 |
| Escape | 弹出层内部 | 关闭弹出层；焦点回到按钮 |
| Tab | 弹出层内部 | 在其控件之间移动；离开时将其关闭 |

这三者共需约 2.2 KB gzip 的脚本，没有自己的请求；工具提示是 1 个节点，弹出层是 6 个加上其内容，忙碌遮罩是 3 个。

## 下拉按钮和拆分按钮

`MenuButton::new(label)` 打开一个命令菜单；`MenuButton::split(label, action, input)` 通过主体部分运行一个 action，并通过箭头打开菜单。命令包括 `.action(label, action, input)`（或成功后带 effect 的 `.action_then(...)`）、用于打开窗口等 effect 的 `.item(label, effect)`，以及 `.separator()`。`.confirm(text)` 会在最后一个 action 命令运行之前进行询问。

```rust
MenuButton::split(t!("orders.save"), save_draft, id)
    .action(t!("orders.save_and_send"), save_and_send, id)
    .separator()
    .action(t!("orders.delete"), delete_order, id)
    .confirm(t!("orders.delete_confirm"))
```

action 命令是拥有自己路径的节点，与 `ActionButton` 一样提交到表单端点：每次调用都会检查 screen 的访问权限、feature 的 guard、action 的权限、拦截器、CSRF 和写入速率限制，input 保留在服务器端。用户无权运行的命令会从菜单中省略（拆分按钮的主 action 则改为禁用），即使被调用，服务器仍会拒绝。

@live menus

| 按键 | 位置 | 作用 |
|---|---|---|
| Enter、Space、Arrow Down | 按钮 | 打开菜单并定位到第一个命令 |
| Arrow Up | 按钮 | 打开菜单并定位到最后一个命令 |
| Arrow Down、Arrow Up | 菜单 | 下一个和上一个命令，循环 |
| Home、End | 菜单 | 第一个和最后一个命令 |
| 字母 | 菜单 | 下一个以这些字母开头的命令 |
| Enter、Space | 命令 | 运行该命令并关闭菜单 |
| Escape | 菜单 | 关闭菜单；焦点回到按钮 |
| Tab | 菜单 | 关闭菜单并继续移动 |

菜单需要约 0.8 KB gzip 的脚本，在命令运行之前没有请求；它是 4 个节点加上每个命令 2 个。

## DocumentViewer

`DocumentViewer::of(page_action, page_count)` 显示一个分页文档（例如报表预览），其页面在服务器上绘制。页面 action 接收一个 `DocPage`（`document` 来自 `.document(id)`，从不来自浏览器，以及 `page`），并以 SVG 或 HTML 片段返回一页。每一页都具有 `.page_size(..)` 的尺寸（`PageSize::A4`、`PageSize::LETTER`、`PageSize::pt(w, h)` 或 `PageSize::px(w, h)`），因此滚动区域从一开始就与整个文档一样高。

```rust
DocumentViewer::of(report_page, report.pages)
    .document(report.id.to_string())
    .page_size(PageSize::A4)
    .outline(vec![OutlineItem::new("Summary", 1).child(OutlineItem::new("Totals", 2))])
    .search(search_report)
    .download("PDF", format!("/reports/{}/pdf", report.id))
```

- 页面中只有可见的页以及两侧各一页。它们在滚动停顿后通过 `POST /_zeyn/data` 获取（快速滚过的页从不获取），最近的优先，同时最多 3 个；页面滚出视图的请求会被中止。浏览器中最多保留 12 个已渲染的页。
- 缩放会缩放页面而不重新获取：适合宽度、适合页面以及 50% 到 400%。页码框可跳转到某一页，滚动停止后会朗读当前页（“第 3 页，共 500 页”）。
- `.outline(items)` 添加目录面板，`.search(action)` 添加搜索框，其 action 接收 `DocSearch` 并返回 `PageHit`（最多显示 50 个），`.download(label, href)` 添加下载链接。
- 服务器在每一页上检查屏幕的访问权限、页面 action 的权限、CSRF 和写入限制，拒绝 1 到页数范围之外的页码以及超过 2 MB（`MAX_PAGE_BYTES`）的页面，并用允许列表（`sanitize_markup`）清理每一页：移除脚本、事件处理器、`foreignObject`、框架、外部链接和图片以及样式中的 `url()`，并为每页的 id 加上前缀。`.trusted()` 为应用用自己掌控的数据绘制的页面跳过清理；未经转义进入此类页面的用户文本会在每个打开它的人那里作为脚本运行。
- `document.js`（13 KB，gzip 后 4.7 KB）和 `document.css`（3.6 KB）只在含有查看器的页面上加载。

| 按键 | 位置 | 作用 |
|---|---|---|
| Page Down、Page Up | 页面 | 下一页和上一页 |
| Home、End | 页面 | 第一页和最后一页 |
| `+`、`-` | 页面 | 放大和缩小 |
| Enter | 页码框 | 跳转到输入的页 |
