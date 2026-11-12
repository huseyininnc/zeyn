# 树与图表

## 树

`Tree::of(action)` 显示按需加载的层级结构。action 接受 `TreeQuery`，并返回 `q.parent` 的子节点（顶层时为 `None`）。分支在首次展开时才加载其子节点，因此大型层级结构上的树，每展开一个分支只产生一次小请求。

```rust
#[action(permission = "stock.product.read")]
pub async fn sample_tree(q: TreeQuery, store: SampleStore) -> Result<Vec<TreeItem>> {
    Ok(match q.parent.as_deref() {
        None => vec![TreeItem::branch("piece", "Piece"), TreeItem::branch("kg", "Kilogram")],
        Some(unit) => store.by_unit(unit)?.into_iter().map(|s| TreeItem::leaf(s.id.0.to_string(), s.name).hint(s.code)).collect(),
    })
}

Tree::of(sample_tree).label("Products by unit")
```

@live tree

树遵循 ARIA tree 模式：方向键在条目之间移动，并展开或折叠分支。服务器会限制每个分支的子节点数量（`MAX_TREE_CHILDREN`）。

`.draggable()` 让叶子可以拖到放置目标上，例如一个 island。拖动携带 MIME 类型 `application/x-zeyn-tree` 和 JSON `{ "id", "path", "label" }`（`path` 是从顶层到该叶子的 id 列表），并把标签作为 `text/plain`；放置目标在 `dragover` 处理函数中设置 `dropEffect = "copy"`。不拖动时，获得焦点的叶子上的“插入”按钮（用于指针）和 Control+Enter（Mac 上为 Command+Enter，用于键盘）会派发可取消的 DOM 事件 `zeyn:tree-insert`，其 `detail` 是同一个对象。在自己当前位置插入的目标调用 `preventDefault()`；只有这时树才会向屏幕阅读器播报这次插入。叶子获得焦点时 Control+Enter 归树所有，因此相同按键上的应用快捷键在那里不会运行。任何页面都可以伪造这份数据，所以目标把它当作不可信输入，并在服务器上检查访问权限。拖动代码（`treedrag.js`）只在含有可拖动树的页面上加载。

```rust
Tree::of(report_fields).label("Fields").draggable()
```

## 图表

`Chart::bar`、`stacked_bar`、`line`、`area`、`combo`、`scatter`、`donut`、`gauge` 和 `funnel` 在服务器上绘制 SVG。浏览器中没有图表库；图表是页面 HTML 的一部分。

```rust
Row::new()
    .col(Chart::bar("Sales").labels(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .series("2025", [120.0, 98.0, 143.0, 160.0, 151.0, 189.0])
        .series("2026", [131.0, 122.0, 150.0, 171.0, 166.0, 204.0]))
    .col(Chart::donut("Units").labels(["Piece", "Kg", "Liter"]).series("Products", [40.0, 40.0, 40.0]))

Chart::stacked_bar("Channels").labels(months).series("Shop", shop).series("Web", web)
Chart::combo("Sales and margin").labels(months).series("Sales", sales).line_series("Margin %", margin).right_axis()
Chart::area("Visits").labels(months).series("Visits", visits)
Chart::scatter("Price and units").points("Tea", [(1.5, 40.0), (2.5, 22.0)])
Chart::gauge("Capacity").value(72.0).range(0.0, 100.0).target(85.0)
Chart::funnel("Checkout").labels(["Visited", "Cart", "Paid"]).series("People", [1000.0, 420.0, 150.0])
```

@live chart

每个图表都有标题、供屏幕阅读器使用的文字摘要、在有两个或更多系列时显示的图例，以及位于 *Show data* 下方的数据表格。柱状图、折线图、面积图和组合图在 SVG 中只绘制几何图形，几何图形随卡片拉伸（`--z-chart-h`，240 px，手机上 200 px）；刻度标签是旁边的 HTML，因此在任何宽度下都保持 12 px，从一万起缩写（`600K`、`1.2M`），类别标签会被抽稀，最多显示十二个（宽度小于 480 px 的图表中为其一半）。柱子占其分带宽度的 60%；折线为 2 px 且不带圆点，只在最后一个值和悬停的类别上显示一个圆点；面积逐渐淡出。指针或键盘焦点落在某个类别上时，会绘制十字准线和一个提示框，列出该类别下的所有系列。系列颜色是 `--z-chart-1` 至 `--z-chart-8` 令牌，其顺序让前五种在色盲情况下仍可区分；系列 7 和 8 的折线还会画成虚线。启动检查和一项测试（`chart_colors_reach_three_to_one_in_both_themes`）要求每个系列相对 `--z-surface` 达到 3:1（参见 [主题](theming.md)）。

每个柱、点、扇区、阶段和仪表值都是一个标记（mark），其标签和值就是它的名称。SVG 是一个以标题和摘要命名的分组，整个图表只占一个 Tab 停靠点：

| 按键 | 作用 |
|---|---|
| Tab | 进入图表，定位到上次获得焦点的标记（初次为第一个） |
| Left、Right | 同一系列的上一个或下一个标记（在 `dir="rtl"` 下互换） |
| Up、Down | 上一个或下一个系列的同一数据点 |
| Home、End | 该系列的第一个或最后一个标记 |
| Escape | 隐藏提示框 |

悬停、聚焦和轻触会在提示框中显示相同的文字，提示框对屏幕阅读器隐藏，因此不会重复朗读。没有脚本时，标记仍然有名称，数据表格也仍然存在。较小的标记（8 px 的点）低于 24 px 的目标尺寸；*Show data* 下方的表格是等效的访问路径（WCAG 2.5.8）。

## Gantt

把任务显示为跨越若干天的条形，例如一个项目的计划。`Gantt::new(label, start, end)` 设置时间跨度，最长两年；`.task(title, from, to)` 添加一个任务，`.done(percent)` 和 `.tone(...)` 作用于最后添加的任务，`.today(day)` 在那一天画一条线。它是一个表格，以文字给出每个任务的第一天和最后一天，因此条形只是重复这些内容；条形在跨度的两端被截断。

```rust
Gantt::new(t!("project.plan"), project.starts, project.ends)
    .task(t!("project.design"), design.from, design.to)
    .done(design.percent)
    .today(ctx.today())
```

@live gantt

## OrgChart

以线条连接的方框表示的层级结构，从顶部的一个方框开始：公司的人员、产品的组成部分。`OrgNode::new(name)` 是一个方框，`.role(text)` 是名称下方较淡的一行，`.child(node)` 是它下面的一个方框。它是嵌套列表，因此屏幕阅读器可以得到层级；一张图最多在 8 层上绘制 200 个方框，过宽时在自己的框内滚动。

```rust
OrgChart::new(
    t!("company.chart"),
    OrgNode::new(director.name.clone())
        .role(t!("company.director"))
        .child(OrgNode::new(lead.name.clone())),
)
```

## Funnel

一个每一步都会流失一部分的流程的各个阶段，例如访问、购物车和订单。`Funnel::new(label)` 配合 `.stage(title, value)` 把每个阶段画成居中的条形，宽度等于它占第一阶段的比例，并以文字给出其数值和该比例。一个漏斗最多容纳 10 个阶段。

```rust
Funnel::new(t!("sales.funnel"))
    .stage(t!("sales.visits"), visits)
    .stage(t!("sales.orders"), orders)
```

## ActivityRings

把整体中最多四个部分显示为层层嵌套的圆环，例如一天的目标。`.ring(title, value, max, tone)` 从外向内添加一个圆环，`.size(px)` 设置直径。每个名称和百分比都写在圆环旁边，因此图形对屏幕阅读器是隐藏的。

```rust
ActivityRings::new(t!("day.goals"))
    .ring(t!("day.packed"), packed, 60, Tone::Brand)
    .ring(t!("day.calls"), calls, 20, Tone::Success)
```

@live visuals

## ComparisonTable

把套餐或产品并排显示，每个一列，每项功能一行。`.plan(name)` 添加一列，`.featured()` 突出显示最后添加的套餐；`.row(feature, cells)` 为每个套餐接收一个单元格：`true` 和 `false` 变成勾和叉，读作"包含"和"不包含"，文字则原样输出。它最多容纳 6 个套餐和 100 行。

```rust
ComparisonTable::new(t!("pricing.plans"))
    .plan(t!("pricing.free"))
    .plan(t!("pricing.team"))
    .featured()
    .row(t!("pricing.audit_log"), [false, true])
```

## Table

由屏幕已经持有的文字组成的小表格：价目表中的套餐、商店的营业时间、报表的合计。`Table::new(caption)` 接收一个为表格命名的标题，`.head([...])` 给出列标题，`.row([...])` 给出每一行；`.row_headers()` 把每一行的第一个单元格作为该行的标题，这样屏幕阅读器会读出每个单元格所在的列和行，`.numeric(index)` 让一列数字靠末端对齐，`.striped()` 为每隔一行加上底色。比表头短的行用空单元格补齐，比表头长的行会被截断；一个表格最多容纳 200 行和 12 列。它没有分页、排序或筛选，也不发出请求：来自 action 的行，或者用户要操作的行，应当放在 `DataGrid` 中。在窄屏上，它在自己的框内横向滚动，这个框可以获得焦点，以便用方向键滚动。

```rust
Table::new(t!("shop.opening_hours"))
    .head([t!("shop.day"), t!("shop.opens"), t!("shop.closes")])
    .row([t!("shop.weekdays"), "09:00".into(), "18:00".into()])
    .row([t!("shop.saturday"), "10:00".into(), "14:00".into()])
    .row_headers()
```

@live table

## Heatmap

最近几周中每一天的一个数字，例如接到的订单数或记录的工时，以方格组成的日历呈现：每列是一周，每行是星期几，数字越大颜色越深。`Heatmap::ending(label, end)` 以 `end` 所在的那一周结束，`end` 通常是今天，其后的日子保持为空。`.weeks(n)` 设置显示多少周，未设置时为 17 周，最多 53 周；`.value(day, n)` 或 `.values(days)` 提供数字，`.tone(...)` 提供颜色。所显示的最大数字使用四个色阶中最深的一个。一周从用户语言中一周开始的那一天开始。

```rust
Heatmap::ending(t!("sales.orders_a_day"), ctx.today())
    .weeks(12)
    .values(orders.iter().map(|o| (o.day, o.count)))
    .tone(Tone::Success)
```

@live heatmap

它是由服务器绘制的表格，不需要脚本。屏幕阅读器会读出每一行是星期几，并按用户的语言读出每一天的日期和数字，因此色阶只是重复文字所表达的内容；指向某个方格时会显示同样的文字。在窄屏上，表格在自己的框内横向滚动，这个框可以获得焦点，以便用方向键滚动。每一天是两个元素：默认的 17 周保持在单个组件 300 个元素的预算之内，而一整年则超出。

## 关键指标、迷你图与子弹图

`Kpi` 把一个值显示为卡片中最大的文字（30 px，等宽数字，货币符号或单位更小且弱化），把与上一周期相比的变化显示为一个小标签，例如 `+48.8%`，旁边写着“与上一周期相比”（变化为好时为绿色，否则为红色，低于百分之零点五时为灰色；成本和等待时间使用 `.lower_is_better()`；屏幕阅读器会听到完整的句子），还显示目标及已达成的比例，以及一条通栏的近期数值趋势线，颜色与该标签相同，下方是渐隐的面积。七位及以上的数值会被缩写（`$1.23M`，土耳其语中为 `1,23 Mn`），完整数值显示在提示框中并提供给屏幕阅读器。

```rust
Kpi::new("Revenue this month", 184_250.0).delta(12.4).target(200_000.0).trend([140.0, 152.0, 160.0, 184.0])
Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()
Kpi::money("Revenue", Money::tl(18_425_050)).delta(4.2).target(200_000.0)
```

`Kpi::money` 按用户的区域设置显示带货币的金额（英语中为 `₺184,250.50`，土耳其语中为 `184.250,50 ₺`，货币符号以较小字号显示），目标也使用同一货币。

同一个 `Row` 或 `AutoGrid` 中的 Kpi 与最高的一个等高，无论是否有趋势线，迷你折线图都位于每张卡片的底部。由 Kpi 组成的 `Row` 给每个 Kpi 四分之一的宽度：在桌面上四个正好填满一行，宽度小于约 830 px 时每行两个；小于 360 px 时卡片不再显示趋势线（基于该行的容器查询，因此窄列中的 Kpi 行会更早换行）。无论有没有趋势线，每张卡片至少保持 132 px。图表与其容器同宽。环形图、仪表盘、漏斗图和散点图保持比例；环形图和仪表盘最高 18rem，并居中显示。

```rust
Row::new()
    .col(Kpi::money("Revenue", revenue).delta(12.4).trend(weekly))
    .col(Kpi::new("Orders", 1_284.0).delta(3.1))
    .col(Kpi::new("New customers", 312.0).delta(-2.0))
    .col(Kpi::money("Average order", average).delta(0.0))
```

在 `DataGrid` 中，`Vec<f64>` 列会绘制为迷你图，数字或金额列会绘制为针对目标的子弹图（bullet）：

```rust
DataGrid::of(search_reps)
    .column(Rep::sales).cell(BulletCell::new(80_000.0, 120_000.0))
    .column(Rep::weeks).cell(SparklineCell::new())
```

两者每个单元格都是一个小型 SVG，名称形如 "Trend of 12 values from 40 to 61, lowest 12, highest 99"。预算：这两种单元格每行约 0.65 KB 的 HTML；在调试构建中渲染 10,000 行的两种单元格约需 0.4 秒（`sparkline_and_bullet_cells_render_ten_thousand_rows_quickly`），而且表格只渲染视野内的行。`charts.js` 增加 3.7 KB 原始大小（1.4 KB gzip），`charts.css` 增加 4.1 KB（1.3 KB gzip）。
