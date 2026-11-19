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

`Card` 是带有可选标题的普通表面：20 px 内边距、一道边框、没有阴影。`.action(node)` 在标题行末尾放一个操作（链接按钮或菜单），`.footer(node)` 在分隔线下方添加页脚，例如“查看全部”链接。作为卡片内容的表格或列表会延伸到卡片边缘，因此只有一道边框而不是两道；卡片中的卡片会去掉自己的外框；`Row` 中的卡片会拉伸到最高那张的高度。`Panel` 是带标题的区块，可带有工具栏；`.collapsible()` 允许用户折叠它，`.collapsed()` 使其初始即为折叠状态。

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`Tabs::new(label)` 搭配 `.tab(title, node)`。方向键在标签页之间移动，与 ARIA tabs 模式的描述一致。已关闭标签页的内容在首次打开之前不会渲染，因此包含大量标签页的界面依然轻量。

`.remember(KEY)` 以选择（choice）键保存用户打开的标签页（参见[客户端偏好设置](security.md#client-preferences)），因此之后的页面会打开同一个标签页。URL 哈希中指定的标签页优先。

## Accordion

`Accordion::new()` 配合 `.section(title, node)` 把各个部分上下排列，每个部分通过其标题打开和关闭：帮助页面的答案、一条长记录的各个部分。各部分初始为关闭；在某个部分之后调用 `.open()` 会打开它。`.single()` 让同一时间只有一个部分打开，此时只有第一个标记为打开的部分初始为打开。一个 accordion 最多容纳 50 个部分（`MAX_ACCORDION_SECTIONS`）。

```rust
Accordion::new()
    .section(t!("help.password"), Paragraph::new(t!("help.password_answer")))
    .open()
    .section(t!("help.invoices"), Paragraph::new(t!("help.invoices_answer")))
    .single()
```

@live accordion

每个部分都是原生的展开元素（`<details>` 和 `<summary>`），而不是由标题和按钮组成的 ARIA accordion 模式，因为原生元素不需要脚本，让关闭的内容不进入阅读顺序，并且让浏览器的页内查找能打开包含匹配项的部分。在单一模式下，各部分共用一个 `name`，当另一个部分打开时浏览器会关闭已打开的那个。与 `Tabs` 不同，关闭部分的内容从一开始就在页面中。

| 按键 | 作用 |
|---|---|
| `Tab` | 移动到下一个部分的标题 |
| `Enter` 或 `Space` | 打开或关闭该部分；焦点留在其标题上 |

## Split

`Split::new(start, end)` 显示两个节点，中间有一个拖动手柄。`.ratio(percent)` 设置起始部分的大小。手柄可通过指针或方向键移动。

## Dock

`Dock` 围绕中心区域排列面板：左侧、右侧、底部和中心标签页，用户可以移动、调整大小、关闭并重新打开它们。每个用户的布局按界面保存。参见[外壳与命令](shell.md)。

## Surface

`Surface::new(label, width, height)` 是一个固定大小的区域，用户可以在其中自由放置项目，例如平面图上的桌子。`.item(Draggable::new(id, node).at(x, y).size(w, h))` 添加一个项目，`.axis(Axis::X)` 或 `Axis::Y` 让它保持在一条线上，`.snap(step)` 让每个位置对齐网格。`.on_move(action)` 保存每次移动：服务器先检查 id 属于该区域，并把位置限制在边界内、网格上和轴上，然后操作会收到一个包含项目 id 及新 `x` 和 `y` 的 `SurfaceMove`。

`.resizable()` 在项目右下角加一个手柄；`.min_size(w, h)`、`.max_size(w, h)` 和 `.aspect(w, h)` 限制尺寸，同时也让项目可调整大小。此时动作还会收到新的 `w` 和 `h`，它们保持在这些限制、区域和网格之内。设置宽高比时，高度跟随宽度。

```rust
Surface::new(t!("floor.plan"), 800, 480)
    .snap(20)
    .on_move(move_table)
    .item(
        Draggable::new("t1", Paragraph::new(t!("floor.table")))
            .at(40, 60)
            .size(120, 80),
    )
```

鼠标或触控笔可以拖动项目；触摸时短按后即可拖动。只用一个指针时，点击选中项目，再点击区域即可放到那里。点击角上的手柄选中尺寸，再点击区域把角放到那里。

| 按键 | 作用 |
|---|---|
| 方向键 | 将获得焦点的项目移动一个网格步长（无网格时为 10 像素） |
| `Shift` + 方向键 | 把可调整大小的项目改变一个网格步长 |
| `Escape` | 取消选中的项目，或把正在拖动的项目放回原处 |
