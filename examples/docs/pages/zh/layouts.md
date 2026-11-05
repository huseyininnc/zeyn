# 布局

布局组件负责摆放其他组件。它们不持有数据，也不增加请求；由它们构成的界面就是一次 HTML 响应。

```rust
Stack::new()
    .add(Row::new().col(Card::new().title("Orders")).col(Card::new().title("Stock")).col(Card::new().title("People")))
    .add(
        Panel::new("Products").collapsible().add(
            Tabs::new("Views")
                .tab("Card", Card::new().title("A card inside a tab"))
                .tab("Split", Split::new(Card::new().title("Left"), Card::new().title("Right")).ratio(55)),
        ),
    )
```

@live layout

## Stack

节点上下排列。`.gap(units)` 以间距单位（常规密度下为 4 px）设置它们之间的间隔，取值 0 到 12。

## Row

列并排排列。每个 `.col(node)` 占用一份宽度；在列之后使用 `.weight(n)` 可使其更宽。在窄屏上，各列会堆叠。`.compact()` 用于紧凑排列徽章之类的小项目。

```rust
Row::new().col(customer_form).weight(2).col(order_totals)
```

## Card 与 Panel

`Card` 是带有可选标题的普通表面。`Panel` 是带标题的区块，可带有工具栏；`.collapsible()` 允许用户折叠它，`.collapsed()` 使其初始即为折叠状态。

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`Tabs::new(label)` 搭配 `.tab(title, node)`。方向键在标签页之间移动，与 ARIA tabs 模式的描述一致。已关闭标签页的内容在首次打开之前不会渲染，因此包含大量标签页的界面依然轻量。

`.remember(KEY)` 以选择（choice）键保存用户打开的标签页（参见[客户端偏好设置](security.md#client-preferences)），因此之后的页面会打开同一个标签页。URL 哈希中指定的标签页优先。

## Split

`Split::new(start, end)` 显示两个节点，中间有一个拖动手柄。`.ratio(percent)` 设置起始部分的大小。手柄可通过指针或方向键移动。

## Dock

`Dock` 围绕中心区域排列面板：左侧、右侧、底部和中心标签页，用户可以移动、调整大小、关闭并重新打开它们。每个用户的布局按界面保存。参见[外壳与命令](shell.md)。
