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

每个图表都有标题、供屏幕阅读器使用的文字摘要、图例，以及位于 *Show data* 下方的数据表格。系列颜色是 `--z-chart-1` 至 `--z-chart-6` 令牌；一项测试（`chart_colors_reach_three_to_one_in_both_themes`）会检查它们在浅色主题和两种深色主题中，相对 `--z-surface` 和 `--z-surface-2` 的对比度是否达到 3:1（参见 [主题](theming.md)）。

每个柱、点、扇区、阶段和仪表值都是一个标记（mark），其标签和值就是它的名称。SVG 是一个以标题和摘要命名的分组，整个图表只占一个 Tab 停靠点：

| 按键 | 作用 |
|---|---|
| Tab | 进入图表，定位到上次获得焦点的标记（初次为第一个） |
| Left、Right | 同一系列的上一个或下一个标记（在 `dir="rtl"` 下互换） |
| Up、Down | 上一个或下一个系列的同一数据点 |
| Home、End | 该系列的第一个或最后一个标记 |
| Escape | 隐藏提示框 |

悬停、聚焦和轻触会在提示框中显示相同的文字，提示框对屏幕阅读器隐藏，因此不会重复朗读。没有脚本时，标记仍然有名称，数据表格也仍然存在。较小的标记（8 px 的点）低于 24 px 的目标尺寸；*Show data* 下方的表格是等效的访问路径（WCAG 2.5.8）。

## 关键指标、迷你图与子弹图

`Kpi` 显示一个值、与上一周期相比的变化（带有箭头和表示方向的文字，变化为好时显示绿色，否则显示红色；成本和等待时间使用 `.lower_is_better()`）、目标及已达成的比例，以及近期数值的迷你图（sparkline）。

```rust
Kpi::new("Revenue this month", 184_250.0).delta(12.4).target(200_000.0).trend([140.0, 152.0, 160.0, 184.0])
Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()
```

在 `DataGrid` 中，`Vec<f64>` 列会绘制为迷你图，数字或金额列会绘制为针对目标的子弹图（bullet）：

```rust
DataGrid::of(search_reps)
    .column(Rep::sales).cell(BulletCell::new(80_000.0, 120_000.0))
    .column(Rep::weeks).cell(SparklineCell::new())
```

两者每个单元格都是一个小型 SVG，名称形如 "Trend of 12 values from 40 to 61, lowest 12, highest 99"。预算：这两种单元格每行约 0.65 KB 的 HTML；在调试构建中渲染 10,000 行的两种单元格约需 0.4 秒（`sparkline_and_bullet_cells_render_ten_thousand_rows_quickly`），而且表格只渲染视野内的行。`charts.js` 增加 3.7 KB 原始大小（1.4 KB gzip），`charts.css` 增加 3.3 KB（1.1 KB gzip）。
