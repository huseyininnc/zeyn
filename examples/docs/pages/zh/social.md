# 社交

用于社交页面的组件。每个组件都在服务端渲染，并且只在使用它的页面上加载自己的样式表和脚本。

## StoryRing

`StoryRing` 是带环的头像，用来表示某人的动态对查看者来说是否是新的。`StoryState::Unseen` 绘制品牌色的粗环，`StoryState::Seen` 绘制中性色的细环。状态来自服务端，取决于应用保存的关于查看者的数据；浏览器从不修改它。`StoryRow` 把多个环排成一行，可以像滑轨一样横向滚动。

```rust
StoryRow::new()
    .add(StoryRing::new(ada.name.clone(), StoryState::Unseen)
        .image(ada.photo.clone())
        .link(stories, StoryParams { person: ada.id }))
    .add(StoryRing::new(alan.name.clone(), StoryState::Seen).image(alan.photo.clone()))
```

设置 `.link(screen, params)` 后，环就是指向显示动态的屏幕的链接。它的无障碍名称是此人的姓名加上状态，例如“Ada Lovelace，未读动态”；环下方显示的名称以同样的方式开头，因此语音控制用户可以直接说出它。两种状态不仅颜色不同，环的宽度也不同；在强制颜色模式下，已读环以虚线绘制。图片遵循 `Avatar` 的规则（应用的路径或允许的 `https` 地址）；没有图片时显示首字母。

环只用 CSS 绘制：没有脚本，也没有图片。`stories.css`（2.2 KB）只在包含环的页面上加载。一行最多容纳 `MAX_STORY_RINGS`（100）个环。触摸、触控板和 Shift+滚轮都可以滚动它，环会吸附到位，获得键盘焦点的环会滚动到可见区域。当一行中没有任何环带链接时，由这一行本身获得焦点，这样键盘仍然可以滚动它。

| 按键 | 作用 |
|---|---|
| Tab | 移到下一个环并将其滚动到可见区域 |
| Enter | 打开当前焦点环的动态 |
| Left, Right | 焦点在行本身时：滚动该行 |


## Lightbox

任何 `Image` 都可以用 `.lightbox()` 全屏打开：这是一个模态对话框，图片会尽可能放大到屏幕允许的尺寸，并带有说明文字和缩放功能。`.full(src)` 指定 lightbox 显示的较大文件，页面本身仍使用小图。`.caption(text)` 在图片下方添加说明文字（两者都会启用 lightbox）。`.video(source, file)` 把图片设为某个媒体源中视频的封面，lightbox 会播放该视频。同一条帖子的图片，或同一个容器（例如 `Row`、`Stack` 或 `AutoGrid`）中的图片组成一组：方向键和滑动手势可在其中切换。

```rust
AutoGrid::new().min(140)
    .add(Image::new(p.thumb.clone()).alt(p.title.clone()).size(300, 300)
        .full(p.photo.clone())
        .caption(p.title.clone()))
    .add(Image::new(clip.poster.clone()).alt(clip.title.clone()).size(320, 180)
        .video("films", clip.file.clone()))
```

没有脚本时，图片是指向其文件的链接，因此仍然可以打开。有脚本时，`lightbox.js`（13 KB，gzip 后 4.4 KB）和 `lightbox.css`（4 KB）只在包含 lightbox 图片的页面加载，对话框在第一次打开时才构建。对话框中只有当前打开的一项；相邻项的较大文件会作为图片提前获取，视频则从不预取。缩略图立即以模糊状态显示，较大文件加载完成后将其覆盖。在桌面上打开大约需要 16 ms。

较大文件遵循图片的地址规则（应用内路径或允许的 `https` 地址）；违反规则时使用图片自身的文件。视频文件必须是该媒体源的有效媒体 id，媒体源会在每次请求时检查其权限并支持分段（range）传输。对话框遵循 W3C APG 对话框模式：焦点移到关闭按钮，Tab 和 Shift+Tab 保持在对话框内，关闭后焦点回到打开它的图片（如果信息流此时已卸载该图片，则回到它所在的帖子）。每个图片链接以其 alt 文本命名，alt 为空时命名为“打开图片”。位置信息（“第 2 个，共 4 个”）会被播报。动画遵循 `prefers-reduced-motion`，对话框打开时背后的页面不会滚动。

| Key | What it does |
|---|---|
| Enter | 在图片上：打开 lightbox |
| Right, Left | 下一项和上一项（从右到左的语言中相反）；平移已放大的图片 |
| Up, Down | 平移已放大的图片 |
| Home, End | 第一项和最后一项 |
| +, -, 0 | 放大、缩小、重置缩放 |
| Escape | 关闭 lightbox 并归还焦点 |
| Tab, Shift+Tab | 在对话框的按钮之间移动而不离开对话框 |

双击、双击触摸和缩放按钮同样可以缩放，已放大的图片可通过拖动平移。

## Feed

`Feed` 是一条无限的帖子流，最新的在最前。它通过与 `CardGrid` 同类的动作加载行（输入是带游标的 `DataQuery`，输出是 `Page`）；动作的顺序就是信息流的顺序，批大小由服务器设定（`.batch(n)`，未设置时为 10，最多 `MAX_FEED_BATCH`，即 50），`.scope(field, value)` 固定一个客户端无法更改的过滤条件。每一行成为一个 `Post`：带头像和用户名的作者、时间、正文、最多 `MAX_POST_MEDIA`（4）张可在 lightbox 中打开的图片、信息流的反应及该帖子的计数、评论数和分享按钮。

```rust
Feed::of(search_posts)
    .title(t!("social.feed"))
    .post(|p: &Story| Post::new(p.author.clone(), p.at)
        .avatar(p.avatar.clone())
        .text(p.text.clone())
        .media(Image::new(p.thumb.clone()).alt(p.alt.clone()).size(300, 300).full(p.photo.clone()))
        .reactions(p.reactions())
        .comments(p.comments)
        .link(story, StoryParams { id: p.id }))
    .reactions(react_to_post)
    .reaction("like", t!("social.like"), "\u{1f44d}")
    .topic("posts")
```

信息流遵循 W3C APG 的 feed 模式：一个 `role="feed"` 区域，内含 `<article>` 元素，每个都可获得焦点，以作者命名，以正文描述，并用 `aria-posinset` 编号（还能加载更多时 `aria-setsize` 为 -1）。加载一批时会设置 `aria-busy`。读者接近末尾时加载更早的帖子，以"显示更多"作为后备。所有文本都经过转义，帖子的正文保留换行。

新帖子绝不会移动读者正在看的内容。当 `.topic(..)` 的某个主题被刷新时（以 `ui.refresh("posts")` 结尾的动作，或带 `.live()` 的实时 hub），信息流会请求第一批，并把比其最新帖子更新的帖子暂存起来。信息流上方的按钮显示"3 条新帖子"，状态区域也会播报；按下后会把它们插入上方、保持滚动位置，并把焦点移到第一条新帖子。当新帖子多于一批所能容纳时，按钮会从最新处重新开始信息流。

内存保持有界。只有靠近视口的帖子在页面中；其余的以 HTML 保存，并保留它们原有的高度，所占空间用内边距表示，因此它们回来时不会有任何跳动。帖子在页面中时保持其元素，因此焦点和它的行键保持不变，滚动到很远处的有焦点的帖子会留在页面中，占据它自己的位置。超过 1,000 条帖子后，远在读者身后的最旧帖子会被丢弃，按钮提供返回最新的途径。

一次反应按下会发送该帖子的键。只有在信息流自己的动作以信息流的范围把该帖子返回给用户之后，服务器才会运行反应动作（带其权限、CSRF 检查和写入速率限制），因此没有人能对自己无法读取的帖子做出反应。动作以 `ReactionPress::target` 获得该键。`feed.js`（15.7 KB，gzip 后 5.3 KB）和 `feed.css`（3.6 KB）只在包含信息流的页面上加载。

| Key | What it does |
|---|---|
| Tab | 进入一条帖子，然后依次经过其链接和按钮 |
| Page Down, Page Up | 把焦点移到下一条或上一条帖子，在末尾加载更多 |
| Control+End | 把焦点移到信息流之后的第一个元素 |
| Control+Home | 把焦点移到信息流之前的最后一个元素 |

在组件目录的社交演示上测量（10,000 条由 id 派生的帖子，三分之一带照片），Chrome，1440 x 900，以 10 条一批加载的同时滚动经过 2,000 条帖子：满 CPU 下 49 fps，p95 帧时间 44 ms，文档中最多 17 条帖子和 883 个元素，4.6 MB JavaScript 堆。
