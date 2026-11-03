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
- **卡片上的更多内容。** `subtitle` 在标题下方添加一行较淡的第二行（作者、分类）。`corner(badge)` 在图片的结束角放一个徽标，与位于起始角的 `badge` 并列；`badges([...])` 在文本下方添加一行徽标，例如标签。`progress(fraction, label)` 沿图片底部画一条细条，表示完成了多少：比例从 0 到 1，屏幕阅读器会读出标签和百分比（带 `aria-valuenow` 的 `role="progressbar"`）。`horizontal()` 将卡片排成一行，开头是一张小图片，文本在旁边；包含这类卡片的卡片网格或 rail 会给它们更宽的列。以上全部都可以在 `CardGrid` 和 `Rail` 中使用。

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

## Rail

rail 是排成一行并可横向滚动的卡片网格，带有滚动吸附，以及按一个视图宽度滚动的上一个和下一个按钮（除非用户要求减少动画，否则为平滑滚动）。当用户滚动到接近末尾时，它每次加载下一批，每批 12 张。卡片是处于 Tab 顺序中的链接，获得焦点时会滚动到视野中。

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
