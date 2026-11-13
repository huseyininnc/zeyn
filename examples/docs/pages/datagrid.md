# DataGrid

`DataGrid` shows rows from an action that takes a `DataQuery` and returns `Page<E>`. Paging, sorting, filters and totals run on the server, so a grid over a million rows costs the browser the same as a grid over a hundred.

## A grid

```rust
DataGrid::of(search_samples)
    .paging(Paging::pages(8))
    .column(Sample::code).width(110).filter()
    .column(Sample::name).flex(1).filter()
    .column(Sample::unit).width(120).filter().badge(|u| match u {
        Unit::Piece => Tone::Neutral,
        Unit::Kg => Tone::Info,
        Unit::Liter => Tone::Brand,
    })
    .column(Sample::price).width(150).avg()
    .column(Sample::stock).width(150).sum().progress(120)
    .column(Sample::active).width(100).check_icon()
    .row_style(|s| match s.stock {
        0 => Some(RowStyle::Danger),
        1..=9 => Some(RowStyle::Warning),
        _ => None,
    })
```

@live datagrid

Sort by clicking a header, type in a filter box, drag a header to move a column or its edge to resize it. Totals cover every filtered row, not only the page.

## The action

```rust
#[action(permission = "stock.product.read")]
pub async fn search_samples(q: DataQuery, store: SampleStore) -> Result<Page<Sample>> {
    Ok(q.apply(store.all()?))
}
```

`DataQuery::apply` filters, sorts, totals and pages a `Vec` in memory, which suits small lists and tests. A store backed by a database reads the same `DataQuery` (page, sort, filters, totals) and answers with SQL; see [Data and databases](database.md).

## Paging

| Paging | Use |
|---|---|
| `Paging::pages(n)` | Numbered pages, 50 rows unless set |
| `Paging::infinite(n)` | Loads more rows while scrolling |
| `Paging::virtual_scroll()` | Very large lists; only the visible rows are in the page |
| `Paging::none()` | Short lists that fit on one page |

The server caps the page size whatever the client asks for.

## Columns

| Call | Result |
|---|---|
| `.width(px)`, `.flex(n)` | Fixed width or a share of the free width |
| `.filter()` | A filter box that fits the type: text, number range, date range, choice list |
| `.no_sort()` | No sort button or sort menu entries; the server drops sort keys on the column. For actions that cannot order by it (generated rows, no database index) |
| `.sum()`, `.avg()`, `.min()`, `.max()` | Totals in the footer. `sum` on a text column does not compile |
| `.badge(\|v\| tone)` | Shows the value as a label in the tone, on its subtle tint |
| `.status(\|v\| tone)` | Shows the value as status: a dot in the tone before plain text, quieter than a badge |
| `.progress(max)` | A bar from 0 to `max` |
| `.check_icon()` | A check mark for `true` |
| `.link(\|row\| effect)` | Makes the cell a link that runs an effect |
| `.pin_start()`, `.hidden()` | Pinned to the start, or hidden until the user shows it |
| `.editable()` | Editable in place, saved with `.edit_with(action)` |
| `.plain()` | Whole numbers without group separators, for codes and years |
| `.wrap()` | Long values wrap onto more lines. Cells stay on one line and end in an ellipsis unless set, so every row has the same height |

Rows are 40 px (32 px in compact density) and divided by lines only; `DataGrid::zebra()` shades every other row for very wide grids. Links in cells read as text at weight 500 and underline on hover.

Numbers and money are right aligned with tabular digits. Whole numbers are grouped by the user's locale (`1,234,567` in English, `1.234.567` in Turkish), in grid cells and in display values such as `DescriptionList`; `#[key]` ids are never grouped, and `.plain()` turns grouping off for one column.

## Rows

- `.on_open(|row| effect)`: double click or Enter on a row.
- `.row_action(label, action, |row| input)`: a button on each row.
- `.bulk_action(label, action, M::id)`: runs on the selected rows.
- `.confirm(message)` and `.tone(Tone::Danger)` right after `.row_action` or `.bulk_action`: asks before that action runs, in the same dialog as `ActionButton::confirm` (Cancel has the focus first), and draws its button in red.
- `.row_menu(|row| ContextMenu)`: the right click menu of a row.
- `.row_style(|row| Option<RowStyle>)`: danger, warning or success rows.
- `.add_with(create)`, `.delete_with(action, M::id)`, `.batch_edit()`: an "Add row" button, a
  Delete button and key, and edits saved together. See the [DataGrid reference](grid.md).
- `.groupable()`, `.summary(label, |t| ...)` on a column: users group by it from its menu or the group panel, and a value computed from the totals shows under it.
- `.filter_builder()`: a "Filter" button that opens a dialog to build the filter conditions; the same list the filter row, saved views and AI agents use.
- `.exportable()`: CSV export of the filtered rows, streamed by the server.

## Columns known at run time

`DataGrid::dynamic(action)` is for results whose columns come with the rows, such as a SQL
preview. The action returns a `DynamicPage` with the columns (name, label and a `DynamicKind`
such as `Integer`, `Decimal` or `Date`) and one page of rows as JSON:

```rust
#[action(permission = "reports.preview")]
pub async fn preview(q: DataQuery, db: Reports) -> Result<DynamicPage> {
    let span = q.span();
    db.run_preview(span.offset, span.limit).await
}

DataGrid::dynamic(preview).paging(Paging::virtual_scroll())
```

Paging, virtual scrolling, keyboard use and number and date formatting in the user's language work
as in a typed grid. Names, labels and cells are escaped, and the server caps columns, rows and text
length. The grid does not sort or filter, and its rows are read only. See the
[DataGrid reference](grid.md).

## Search and bands

`.search()` puts a search box above the grid. The action gets the text as `DataQuery::q`, which
`DataQuery::apply` matches against the text fields, never the masked ones.
`.band(label, |b| b.column(..).column(..))` groups columns under one header cell. See the
[DataGrid reference](grid.md).

## Refreshing in place

`.listens(topic)`, called last, makes the grid reload in place when the topic is named (see [Topics](effects.md#topics)): after `ui.refresh(topic)` it reloads its current page with its sort and filters, after a filter control that `.drives(topic)` changed it starts on the first page with the new screen params. The grid reloads through `POST /_zeyn/grid` with every permission check, and a screen param the filter changed reaches the grid through the screen function, for example as a scope:

```rust
let mut grid = DataGrid::of(search_orders);
if let Some(region) = p.region {
    grid = grid.scope(Order::region, region);
}
grid.column(Order::number).column(Order::total).listens("orders")
```

The [DataGrid reference](grid.md) covers saved views, master detail, grouping, tree data, inline editing, cell navigation, copy and paste, column virtualization and accessibility.
