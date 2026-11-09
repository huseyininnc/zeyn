# 卡片、横向滚动栏与搜索

商店之类的公开页面以卡片而不是表格来显示行。`CardGrid`、`Rail` 和 `SearchBox` 通过与 DataGrid 同类的 action 加载它们的行：该 action 接收 `DataQuery` 并返回 `Page`。分页、排序和搜索文本都在服务器上运行，读取需要该 action 的访问权限，与 DataGrid 相同。

```rust
#[action(public)]
pub async fn catalog_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?.into_iter().filter(|p| p.active).collect()))
}
```

## Card grid

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

- **卡片。** `Card::image(image, title)` 由一行生成一张卡片，可搭配 `text`、`badge`、`alt` 和 `link`（一个 screen 及其参数；整张卡片是一个链接，打开时不会重新加载页面）。图片是懒加载的，宽度和高度来自 `image_size`，因此页面不会跳动。图片的替代文本为空，因为旁边的标题已经说明了这张卡片；当图片传达的信息更多时，请设置 `alt`。路径、`https:` 或 `data:image/` 以外的图片地址会被省略。
- **卡片上的更多内容。** `subtitle` 在标题下方添加颜色较淡的第二行（作者、分类）。`corner(badge)` 把徽章放在图片的结束角，起始角则是 `badge`；`badges([...])` 在文本下方添加一排徽章，例如标签。`progress(fraction, label)` 沿图片底边绘制一条细进度条，表示完成了多少：比例取值 0 到 1，屏幕阅读器会读出标签和百分比（`role="progressbar"` 与 `aria-valuenow`）。`horizontal()` 让卡片按一行排列，起始处是小图片，文本在其旁边；包含此类卡片的卡片网格或 rail 会给它们更宽的列。以上都可用于 `CardGrid` 和 `Rail`。

```rust
Card::image(c.cover.clone(), c.title.clone())
    .subtitle(c.teacher.clone())
    .corner(Badge::new(t!("courses.new")).tone(Tone::Success))
    .badges(c.tags.iter().map(|t| Badge::new(t.clone())))
    .progress(c.done, t!("courses.done"))
    .horizontal()
```
- **快速的商品网格。** 网格上的 `.placeholder(...)` 在每张卡片图片加载期间填充它，卡片也可以设置自己的占位符。在卡片上，`.low(src)` 先显示一个小文件，`.fallback(src)` 会在卡片显示替代文本和图标之前尝试另一个文件一次。`.priority(n)` 使用 `fetchpriority="high"` 立即加载前 `n` 张卡片（最多 12 张）的图片；其他卡片图片保持懒加载，之后加载的页面永远不会优先。它们的作用与 [`Image`](display.md#image) 的选项相同，所有卡片共用一个监听器。

```rust
CardGrid::of(catalog_products)
    .card(|p: &Product| {
        Card::image(p.photo.clone(), p.name.clone())
            .low(p.photo_small.clone())
            .fallback("/static/no-photo.webp")
    })
    .placeholder(Placeholder::color("--z-surface-2"))
    .priority(4)
```
- **长网格。** 屏幕外的卡片和列表行会跳过布局和绘制（`content-visibility: auto`），并按图片尺寸预留大小，因此长网格在被看到之前几乎没有开销。包含 200 张或更多卡片的卡片网格只在页面中保留视口附近的行，与虚拟 [DataGrid](datagrid.md) 的做法相同：其他行是同样高度的空白，每一行的高度取前几张卡片中最高的那张，每张卡片都会向屏幕阅读器报告自己的位置（`aria-posinset`、`aria-setsize`）。Tab 和 Shift+Tab 可以到达每张卡片，获得焦点的卡片滚出视野时仍会保留，“显示更多”会添加到同一个窗口中。快速滑动时，只是一闪而过的卡片不会获取图片；滚动停下后才会加载。页面内查找只能找到视口附近的卡片；其他卡片请使用网格的搜索。
- **首页在服务器端渲染。** 第一页（卡片网格 24 张、rail 12 张，或 `paging` 指定的数量）是页面 HTML 的一部分，因此搜索引擎和未启用 JavaScript 的访客也能看到卡片及其链接；图片带有尺寸，加载时页面不会跳动。后续页面由浏览器加载。公开页面对每位访客都是相同的 HTML，因此可以缓存。链接到带路由的 screen 的卡片会直接使用其规范地址（`/items/{slug}`），而不是重定向。
- **分页。** `Paging::infinite(n)`（默认，24）显示一个“显示更多”按钮，该按钮滚动到视野中时也会触发；按下按钮后，焦点会移到第一张新卡片。`Paging::pages(n)` 显示上一页和下一页按钮以及页码：action 返回总数时为 "Page 2 of 5"，不返回时为 "Page 2"，此时只要 action 返回游标，Next 就可用。页面大小由服务器设置；客户端只说明从哪里继续。
- **排序。** 每个 `sort_option` 是用户可以从下拉框中选择的一种顺序；第一个是卡片起始的顺序。服务器只接受这些顺序，并丢弃客户端发送的过滤条件和搜索文本。
- **范围。** `.scope(field, value)` 在每个请求中只保留该字段等于此值的行，与 [`DataGrid::scope`](datagrid.md) 相同：`.scope(Article::author_id, author.id)` 表示某位作者的文章。卡片网格、rail 和数据绑定列表会丢弃客户端发送的所有过滤条件，因此客户端既不能更改也不能移除该范围。文本比较时忽略大小写。
- **搜索。** `.search(text)` 只显示与自由文本相匹配的行（[`DataQuery::q`](#free-text)），例如结果 screen 的文本。
- **掩码字段。** 卡片由一个能看到原始行的闭包构建。对于带有 `#[mask]` 字段的 model，只有有权揭示这些字段的用户才能得到卡片；其他人会得到 `Forbidden`。公开卡片请使用不含掩码字段的 model。

### 悬停预览

在带有链接的 `ImageCard` 或 `ListItem` 上使用 `.preview(screen, params)`，会在卡片旁的面板中显示该屏幕：鼠标悬停约半秒后，或卡片获得键盘焦点时立即显示。屏幕只在那时才通过 `GET /_zeyn/screen/...` 加载，访问检查与窗口相同，每个预览在页面存续期间保留（最多 24 个，`nav.js` 打开其他屏幕时清除）。框架、对象、媒体播放器和脚本在面板显示之前会被移除，因此预览绝不会加载 iframe。面板位于顶层，所以裁剪内容的 rail 和网格不会截断它；它在空间更大的一侧打开，在屏幕边缘翻转，在自身内部滚动，绝不滚动页面。它遵循 WCAG 1.4.13：指针可以移到面板上，Escape 关闭，直到指针或焦点离开才会消失。在触摸屏和粗略指针上没有预览，点按会打开链接；脚本（`preview.js`，2.5 KB gzip）在那里从不加载，在其他环境中于这类卡片首次悬停或获得焦点时加载。启用减少动态效果时，面板不带短暂的放大动画直接出现。

```rust
Rail::of(search_products).card(|p: &Product| {
    Card::image(p.image.clone(), p.name.clone())
        .link(product, ProductParams { id: p.id })
        .preview(product_summary, ProductParams { id: p.id })
})
```

@live preview


### 构建器

`.card` 和 `.item` 仍是默认方式：链接、图片和读屏文本都现成可用。当一行需要别的形状时，`.builder(|row| node)` 像 Flutter 的 `ListView.builder` 那样把每一行构建成任意节点，可用于 `List::of`、`CardGrid` 和 `Rail`。`.separator(node)` 在行与行之间放一个节点（对读屏软件隐藏，它们自己会数行），`Repeat::of(items, |item| node)` 对界面已经在内存中持有的列表做同样的事，最多 500 项。

```rust
List::of(search_people)
    .builder(|p: &Person| Row::new().compact().col(Avatar::new(p.name.clone())).col(Paragraph::new(p.city.clone())))
    .separator(Paragraph::new("·"))

Repeat::of(order.steps.iter(), |s| Badge::new(s.name.clone())).separator(Paragraph::new("→"))
```

- **键。** 构建的行和虚拟列表的行都在 `data-zeyn-key` 中带有模型的键，因此列表刷新（[主题](effects.md#topics)）时，无论记录移到哪里，焦点和展开的 details 都留在同一条记录上。
- **构建的行只用于展示。** 其中的节点不通过节点路径寻址，所以网格、表单或 lookup 要放在别处。`Repeat` 的节点是子节点（`p.0`、`p.1` ...），可以正常工作。
- **虚拟滚动。** 带 `.paging(Paging::virtual_scroll())` 的列表只把可见的行放在页面中，置于一个与全部行一样高的滚动区域内，并随用户滚动按 offset 获取数据块。行只有一个固定高度，没有逐行高度：使用构建器时必须写 `.row_height(px)`，缺少时 `App::check_page` 会报告（那里不使用 `separator`）。每一行都带有 `aria-posinset` 和 `aria-setsize`，读屏软件会读出它在整个列表中的位置；滚动区域是以标题命名、可获得焦点的 region。

## Rail

rail 是排成一行并可横向滚动的卡片网格，带有滚动吸附，以及按一个视图宽度滚动的上一个和下一个按钮（除非用户要求减少动画，否则为平滑滚动）。当用户滚动到接近末尾时，它每次加载下一批，每批 12 张。卡片是处于 Tab 顺序中的链接，获得焦点时会滚动到视野中。

滚动条默认隐藏；触摸、触控板和 Shift+滚轮仍可滚动，`.scrollbar(true)` 会保留一条细滚动条。可以用鼠标按住并左右拖动 rail。移动超过 5 像素后，松开时的点击会被取消，因此指针下的卡片不会打开，rail 会停在最近的卡片上；更小的移动则是普通点击。拖动期间不会选中文本，图片不会被拖出，也不会打开卡片预览。触摸设备保留浏览器自身的滚动。上一个和下一个按钮始终存在，到达两端时被禁用，因为隐藏的滚动条无法供键盘滚动。没有 JavaScript 时按钮被隐藏，rail 使用浏览器原生滚动。

```rust
Rail::of(catalog_products)
    .title(t!("shop.new_arrivals"))
    .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
    .sort_option(t!("shop.sort.newest"), Product::id, true)
```

## Search box

带建议的搜索字段，遵循 W3C APG 的 combobox 模式，弹出列表框。用户输入时，它等待 `instant` 毫秒，然后向服务器请求建议：action 以该文本作为 `DataQuery::q` 运行，并返回每一行的建议文本（默认是它第一个未掩码的文本字段，除非 `suggestion` 另有指定）。按 Enter，或选择某个建议，会带着该文本打开结果 screen，且不重新加载页面。

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

键盘：向下和向上键在建议之间移动（向下键会打开建议），Enter 以高亮的建议或输入的文本进行搜索，Escape 先关闭建议，再清空字段。实时区域会告诉屏幕阅读器有多少条建议。

结果 screen 接收 `SearchParams`（`q`，从菜单打开该 screen 时为空）。服务器会把文本截断到 `MAX_SEARCH_QUERY`（100）个字符，决定发送多少条建议（默认 8 条，除非 `limit` 另有指定，最多 20 条），检查 screen 的访问权限、Feature 的 guard 和 action 的权限，并把每次建议调用都计入写入频率限制（`App::write_limit`）。

`.drives(topic)` 会就地搜索，而不是打开结果界面：按 Enter、选择建议或清空字段，都会把文本放入界面的 `q` param，并刷新监听该主题的节点（见 [主题](effects.md#topics)）。界面的 params 需要一个 `q: String` 字段，就像 `SearchParams` 那样。

### 富建议

`.item(|row| ListItem::new(title).subtitle(text).image(src).link(screen, params))` 将每条建议显示为一行，包含小图片（40 x 56 像素，延迟加载，遵循 `img-src` 规则，替代文本为空，因为标题已经为该行命名；没有图片时不显示占位图）、标题和副标题。服务器返回 `[{text, subtitle, image, href}]`，其中包含记录的规范地址。通过点击或 Enter 选中某一行，会经由 `nav.js` 打开它的链接；由于每一行都是真正的链接，中键点击和“在新标签页中打开”同样可用。没有高亮行时按 Enter 仍会打开结果 screen。combobox 模式保持不变：每一行是以标题和副标题命名的选项，行高至少 44 像素。没有 `.item` 时，建议为上文所述的纯文本。

```rust
SearchBox::of(catalog_products)
    .item(|p: &Product| {
        ListItem::new(p.name.clone())
            .subtitle(p.code.clone())
            .image(p.image.clone())
            .link(shop_product, ProductPage { id: p.id })
    })
    .results(shop_search)
```

## 自由文本

`DataQuery::q` 是自由文本：其文本字段包含它的行。文本和字段都按 `zeyn::text::fold` 给出的形式比较：小写，土耳其语带点和不带点的 `i`（`İ`、`ı`）视为 `i`，带重音的拉丁字母去掉重音，`ß`、`æ`、`œ`、`þ` 展开书写。因此“istanbul”能找到“İSTANBUL”，“nino”能找到“Niño”，“strasse”能找到“Straße”。带有 `#[mask]` 的字段永远不会被搜索，空白文本匹配每一行，超过 100 个字符的文本会被忽略。`DataQuery::apply` 和 SQL 数据层（对列使用 `TRANSLATE`、`LOWER` 和 `REPLACE`，`LIKE` 使用绑定参数）以同样的方式读取它。

```rust
use zeyn::text::fold;

assert_eq!(fold("İSTANBUL Işık"), "istanbul isik");
assert_eq!(fold("Año Pequeño"), "ano pequeno");
assert_eq!(fold("Straße MÜLLER"), "strasse muller");
```

对于应用自己写入的搜索键列，例如[手写查询](database.md)的 trigram 列，也请使用 `fold`，这样键就与用户输入的内容一致。标点和其他文字保持不变；地址使用的形式是 `Slug::from_title`。
