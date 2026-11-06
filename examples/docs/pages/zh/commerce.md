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
