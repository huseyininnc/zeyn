# 商店组件

商品列表和商品页面需要几个专门的组件：带折扣的价格、带计数的筛选、范围滑块、规格选择器、图库、数量步进器、评价汇总和倒计时。它们和其他组件一样在服务器上渲染。需要浏览器代码的组件只在包含它们的页面上加载一个小模块，其样式（`commerce.css`）也只在那里链接。

## 价格

```rust
Price::new(p.price)
    .was(p.list_price)
    .unit(p.price_per_kg, PriceUnit::Kg)
    .large()
```

- **折扣。** `was(old)` 在当前价格旁显示划掉的原价，并显示四舍五入到整数的折扣百分比。不高于当前价格或币种不同的原价会被省略，因此过期的折扣永远不会显示负百分比。
- **屏幕阅读器**听到的是一句话：“原价 ₺1,599.00，现价 ₺1,299.00，优惠 19%”。划掉的价格和徽标对它们隐藏，因为大多数屏幕阅读器不会说出文字被划掉。
- **单位价格。** `unit(price, PriceUnit::Kg)` 添加一行较淡的文字，例如“每千克 ₺259.80”。单位有 `Item`、`Kg`、`Grams100`、`Liter`、`Ml100` 和 `Meter`。
- **格式。** 金额是采用页面数字格式的 `Money`：英语为 `₺1,299.00`，土耳其语为 `1.299,00 ₺`；在把百分号放在前面的语言中，百分号在前（`%19`）。`large()` 为商品页面放大当前价格。
- **无脚本。** 价格是纯 HTML。


### 卡片上的价格

`ImageCard::price(Price)` 把同样的价格放在 `CardGrid` 或 `Rail` 中卡片文字的下方、卡片链接之内，因此屏幕阅读器会一起读出标题和价格那句话。它不增加脚本；商店样式表随第一张带价格的卡片一起加载。

```rust
CardGrid::of(catalog_products).card(|p: &Product| {
    Card::image(p.photo.clone(), p.name.clone())
        .price(Price::new(p.price).was(p.list_price))
        .link(product_page, ProductPage { id: p.id })
})
```

## 范围滑块

```rust
RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
    .step(50)
    .money(Currency::Try)
    .value(p.min_price, p.max_price)
    .drives("products")
```

- **两个原生滑块。** 每个滑块都是一个带有两个名称之一的 `<input type="range">`，因此浏览器为它提供 W3C APG 滑块行为：各占一个 Tab 停靠点，方向键按步长移动，Home 和 End 移到两端，屏幕阅读器读出格式化后的值（`aria-valuetext`，例如“₺500.00”）和标签（“Price，起”“Price，止”）。Page Up 和 Page Down 按范围的十分之一移动。
- **作为筛选。** `.drives(topic)` 把滑块放进筛选表单：松开滑块时，两个值写入同名的屏幕参数，该主题的监听者就地刷新。在分面面板中由面板的表单完成这一步；在其他表单中，滑块像其他字段一样提交。
- **取值。** `value(low, high)` 接收参数（`None` 保留轨道的端点）。超出范围的值会被截断，低值大于高值时会互换，因此手动修改的地址无法破坏滑块。`money(currency)` 按页面格式显示货币的整数单位。
- **日期。** `RangeSlider::dates(label, names, first, last)` 配合 `.date_value(from, to)` 选择日期；表单通过隐藏字段提交 `2026-03-31` 这样的 ISO 日期。
- **脚本。** `range.js`（gzip 后约 1 KB）只在有滑块的页面加载。它让低值滑块不超过高值滑块，填充两个滑块之间的轨道，把值写在标签旁边，并在按下空白轨道时移动较近的滑块。它每次按下只读取一次轨道尺寸，滑块移动时从不读取。

## 数量步进器

```rust
#[action(permission = "cart.edit")]
pub async fn set_quantity(change: QuantityChange, cart: CartStore) -> Result<QuantityState> {
    let stock = cart.stock(&change.target)?;
    let kept = change.quantity.min(stock);
    cart.set(&change.target, kept)?;
    Ok(QuantityState::new(kept).max(stock))
}

QuantityStepper::of(set_quantity, line.sku.clone())
    .label(t!("cart.quantity"))
    .range(1, 20)
    .value(line.quantity)
```

- **在服务器上检查。** 一次更改（按下减号或加号，或输入数字）在用户停止 400 毫秒后通过 `POST /_zeyn/data` 发送，经过 CSRF 检查和写入速率限制，以用户身份运行，并检查 action 的权限。服务器在 action 运行前把数量限制在步进器的范围内，action 检查只有它知道的内容，例如库存。条目（`target`）由屏幕设定，从不从浏览器读取。
- **以服务器的回答为准。** action 返回它保留的数量和允许的最大值（`QuantityState::new(kept).max(stock)`）。如果少于用户要求的数量，步进器显示该数量，并在实时区域中说“仅剩 7 件。”。较新的更改会取消仍在途中的请求，因此只显示最后一次回答；请求失败时会恢复上次保留的数量并加以说明。
- **键盘与触控。** 按钮至少为 24 × 24 像素（WCAG 2.5.8），名为“减少”和“增加”，在范围两端会禁用；被按下的按钮禁用时，焦点移到输入框。输入框接受数字和方向键。
- **脚本。** `quantity.js`（gzip 后约 1 KB）只在有步进器的页面加载。

## 规格选择器

```rust
VariantPicker::new()
    .group(
        VariantGroup::swatches("color", t!("shop.color"))
            .swatch("sand", t!("shop.sand"), "#d8c3a5")
            .swatch("navy", t!("shop.navy"), "#1f3a68")
            .current(p.color.as_deref()),
    )
    .group(
        VariantGroup::chips("size", t!("shop.size"))
            .option("m", "M")
            .option("xl", "XL")
            .unavailable("xl", t!("shop.not_in_color"))
            .current(p.size.as_deref()),
    )
    .drives("variant")
```

- **单选组。** 每组都是由原生单选按钮组成的 fieldset，即不需要脚本的 W3C APG 单选组模式：Tab 到达已选选项，方向键移动并选择，图例说出组名和已选选项（“颜色 藏青”）。色块接受十六进制颜色或主题令牌；标签块显示其文字。
- **不可选的选项**会带着原因被禁用（`unavailable(value, reason)`），屏幕阅读器会连同选项一起读出原因，提示框也会显示；色块加一条斜线，标签块加删除线。
- **价格、图片和库存随之变化。** 选择器驱动它的主题，并且自己也监听这些主题。选择一个选项会把值写入与组名同名的屏幕参数，同步更新地址（因此可以分享所选规格的链接），并在一次请求中刷新监听该主题的所有节点：在价格、图库和库存行上加 `.listens("variant")`，屏幕会针对新的选择重新禁用选项。除主题模块外不需要其他脚本。
- **检查。** `App::check_page` 会报告不驱动任何内容的选择器，以及没有有效颜色的色块。

## 商品图库

```rust
p.photos.iter().fold(ProductGallery::new(p.name.clone()), |g, photo| {
    g.image(
        Image::new(photo.large.clone())
            .alt(photo.alt.clone())
            .size(800, 800)
            .low(photo.tiny.clone())
            .placeholder(Placeholder::color("--z-surface-2")),
    )
    .thumb(photo.thumb.clone())
})
```

- **一次一张图片。** 幻灯片位于滚动吸附的轨道中，手机无需脚本即可滑动切换。图片下方的缩略图是按钮（“显示第 2 张照片，共 5 张”，当前那张带有 `aria-current`）；在获得焦点的图片上，方向键按页面的阅读方向切换幻灯片。图库是一个有名称的轮播区域，幻灯片会说出“2 of 5”（W3C APG 轮播模式）。没有缩略图（`thumb`）时显示带编号的圆点。
- **只加载看得见的。** 第一张图片立即以 `fetchpriority="high"` 加载，因为它通常是商品页上最大的元素；其余图片在惰性模板中等待，`gallery.js` 通过一个 IntersectionObserver 放入屏幕上的图片及其两侧相邻的图片。12 张照片的图库一开始只加载三张。图片是 `Image`，因此保留占位、低分辨率文件和备用文件。
- **放大和全屏。** 鼠标会放大指针下的图片（指针进入时只读取一次幻灯片的尺寸，移动时从不读取）。按下图片或“全屏”按钮会打开一个模态对话框，包含图片、上一张和下一张按钮、放大切换和关闭按钮；焦点保持在其中，按 Escape 关闭，并把焦点还给打开它的元素。在减少动态效果时不会有滑动或放大动画。
- **脚本。** `gallery.js`（gzip 后约 1.5 KB）只在有图库的页面加载。

## 分面筛选面板

```rust
#[params]
#[derive(Default)]
pub struct Listing {
    #[serde(default)]
    pub color: Vec<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
}

#[screen(route = "/shop", public)]
pub async fn shop(p: Listing, store: ProductStore) -> Result<Screen> {
    let colors = store.count_colors(&p).await?;
    let panel = FacetPanel::of(shop, &p)
        .facet(colors.iter().fold(Facet::new("color", t!("shop.color")), |f, c| {
            f.option(c.value.clone(), c.name.clone(), c.count)
        })
        .chosen(p.color.clone()))
        .range(RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
            .money(Currency::Try)
            .value(p.min_price, p.max_price))
        .drives("products");
    let cards = CardGrid::of(search_products)
        .card(|p: &Product| Card::image(p.photo.clone(), p.name.clone()).price(Price::new(p.price)))
        .scope_any(Product::color, p.color.clone())
        .scope_range(Product::price, p.min_price.map(|m| Money::tl(m * 100)), p.max_price.map(|m| Money::tl(m * 100)))
        .listens("products");
    Ok(Screen::new(t!("shop.title")).body(Row::new().col(panel).col(cards).weight(3)))
}
```

- **计数来自服务器。** 每个分面是一组复选框，选项显示在应用其他分面后各有多少行（“红色 (124)”；屏幕阅读器听到“红色，124 个结果”）。屏幕为每个分面用一条有上限的查询计数，例如把其他筛选条件放进 `WHERE` 的 `SELECT color, count(*) ... GROUP BY color LIMIT 50`；一个分面最多 `MAX_FACET_OPTIONS`（50）个选项。没有行的选项在未被选中时会禁用。
- **每次更改一个请求。** 面板是一个驱动其主题并自己也监听这些主题的筛选表单。一次更改会把值写入屏幕参数（分面为列表，范围为两个数字），同步更新地址，并用一次 `POST /_zeyn/nodes` 刷新所有监听者：卡片网格、计数行以及带新计数的面板本身。较新的更改会取消仍在途中的请求。在 2,000 个商品上测得：从点击到新卡片和计数出现为 26 ms。
- **筛选留在服务器上。** 网格通过 `scope_any(field, values)` 和 `scope_range(field, from, to)`（在 `CardGrid` 和 `DataGrid` 上）接收相同的参数，因此浏览器从不发送自己的筛选条件；没有值或开放的端点会保留所有行。
- **已选筛选条件**以标签的形式显示在各组上方，并带有“全部清除”。每个标签都是指向去掉该筛选条件后的屏幕的链接，因此没有脚本也能使用，其名称为“移除筛选条件 红色”。各组是可折叠的 `<details>`，展开的组在刷新后保持展开。
- **检查。** `App::check_page` 会报告不驱动任何内容的面板，以及名称不是屏幕参数字段的分面。

## 评价与评分

```rust
ReviewSummary::new()
    .count(5, stats.five)
    .count(4, stats.four)
    .count(3, stats.three)
    .count(2, stats.two)
    .count(1, stats.one)
```

- **汇总。** `ReviewSummary` 以数字和星星显示平均分，显示评价数量，并为 5 到 1 星各显示一条带占比的横条。平均分和占比都由计数算出，因此始终一致。屏幕阅读器听到“4.6 星（满分 5 星）”，每个横条读作“5 星，68%”；横条本身对它们隐藏。它是不含脚本的 HTML。
- **评分输入。** 表单字段上的 `Control::Rating` 是一个绘制成星星的单选组（W3C APG，无脚本）。整数字段接受整星；带 `#[rule(min = 1, max = 5, scale = 1)]` 的 `Decimal` 字段接受半星，每半颗星是一个名为“4.5 of 5”的单选按钮。服务器按页面的数字格式读取值，并拒绝介于两个半星之间的值，例如 4.3。

```rust
#[model]
pub struct Review {
    #[rule(required, min = 1, max = 5, scale = 1)]
    pub stars: Option<Decimal>,
    pub text: String,
}

Form::of(Review::default())
    .field(Review::stars)
    .control(Review::stars, Control::Rating)
    .field(Review::text)
    .submit(t!("reviews.send"), save_review)
```
