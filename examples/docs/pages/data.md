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

`Chart::bar`, `Chart::line` and `Chart::donut` draw SVG on the server. There is no chart library in the browser; a chart is part of the page HTML.

```rust
Row::new()
    .col(Chart::bar("Sales").labels(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .series("2025", [120.0, 98.0, 143.0, 160.0, 151.0, 189.0])
        .series("2026", [131.0, 122.0, 150.0, 171.0, 166.0, 204.0]))
    .col(Chart::donut("Units").labels(["Piece", "Kg", "Liter"]).series("Products", [40.0, 40.0, 40.0]))
```

@live chart

Each chart has a title, a text summary for screen readers, a legend, and its numbers as a table under *Show data*. Series colors are the `--z-chart-1` to `--z-chart-6` tokens, which reach 3:1 against the surface in the light and dark themes (see [Theming](theming.md)).
