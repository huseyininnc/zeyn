# Tree and charts

## Tree

`Tree::of(action)` shows a hierarchy that loads on demand. The action takes a `TreeQuery` and returns the children of `q.parent` (`None` for the top level). A branch loads its children the first time it opens, so a tree over a large hierarchy costs one small request per opened branch.

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

The tree follows the ARIA tree pattern: arrow keys move between items and open or close branches. The server caps the number of children per branch (`MAX_TREE_CHILDREN`).

## Charts

`Chart::bar`, `stacked_bar`, `line`, `area`, `combo`, `scatter`, `donut`, `gauge` and `funnel` draw SVG on the server. There is no chart library in the browser; a chart is part of the page HTML.

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

Each chart has a title, a text summary for screen readers, a legend, and its numbers as a table under *Show data*. Series colors are the `--z-chart-1` to `--z-chart-6` tokens; a test (`chart_colors_reach_three_to_one_in_both_themes`) checks 3:1 against `--z-surface` and `--z-surface-2` in the light and both dark themes (see [Theming](theming.md)).

Every bar, point, slice, stage and gauge value is a mark with its label and value as its name. The SVG is a group named by the title and the summary, and the chart is one tab stop:

| Key | What it does |
|---|---|
| Tab | Moves into the chart, to the mark last focused (the first one at the start) |
| Left, Right | Previous or next mark of the same series (swapped under `dir="rtl"`) |
| Up, Down | The same point of the previous or next series |
| Home, End | First or last mark of the series |
| Escape | Hides the tooltip |

Hover, focus and a tap show the same text in a tooltip, which is hidden from screen readers so nothing is read twice. Without the script the marks are still named and the table stays. Small marks (8 px points) are below the 24 px target size; the table under *Show data* is the equivalent path (WCAG 2.5.8).

## Key figures, sparklines and bullets

`Kpi` shows a value, its change against the previous period with an arrow and words for the direction (green when the change is good, red when not; `.lower_is_better()` for costs and waits), a target with the share reached, and a sparkline of recent values.

```rust
Kpi::new("Revenue this month", 184_250.0).delta(12.4).target(200_000.0).trend([140.0, 152.0, 160.0, 184.0])
Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()
```

In a `DataGrid`, a `Vec<f64>` column draws as a sparkline and a number or money column as a bullet against a target:

```rust
DataGrid::of(search_reps)
    .column(Rep::sales).cell(BulletCell::new(80_000.0, 120_000.0))
    .column(Rep::weeks).cell(SparklineCell::new())
```

Both are one small SVG per cell with a name such as "Trend of 12 values from 40 to 61, lowest 12, highest 99". Budget: about 0.65 KB of HTML per row for both cells; 10,000 rows of both render in about 0.4 s in a debug build (`sparkline_and_bullet_cells_render_ten_thousand_rows_quickly`), and grids only render the rows in view. `charts.js` adds 3.7 KB raw (1.4 KB gzip) and `charts.css` 3.3 KB (1.1 KB gzip).
