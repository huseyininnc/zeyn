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
- **分页。** `Paging::infinite(n)`（默认，24）显示一个“显示更多”按钮，该按钮滚动到视野中时也会触发；按下按钮后，焦点会移到第一张新卡片。`Paging::pages(n)` 显示上一页和下一页按钮以及页码。页面大小由服务器设置；客户端只说明从哪里继续。
- **排序。** 每个 `sort_option` 是用户可以从下拉框中选择的一种顺序；第一个是卡片起始的顺序。服务器只接受这些顺序，并丢弃客户端发送的过滤条件和搜索文本。
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

## 自由文本

`DataQuery::q` 是自由文本：其文本字段包含它的行，忽略大小写。土耳其语带点和不带点的 `i`（`İ`、`ı`）都与 `i` 匹配，因此“istanbul”能找到“İSTANBUL”。带有 `#[mask]` 的字段永远不会被搜索，空白文本匹配每一行，超过 100 个字符的文本会被忽略。`DataQuery::apply` 和 SQL 数据层（`LOWER(TRANSLATE(column, 'İı', 'ii')) LIKE ...`，使用绑定参数）以同样的方式读取它。
