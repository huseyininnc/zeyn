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
| `.badge(\|v\| tone)` | Shows the value as a colored badge |
| `.progress(max)` | A bar from 0 to `max` |
| `.check_icon()` | A check mark for `true` |
| `.link(\|row\| effect)` | Makes the cell a link that runs an effect |
| `.pin_start()`, `.hidden()` | Pinned to the start, or hidden until the user shows it |
| `.editable()` | Editable in place, saved with `.edit_with(action)` |

## Rows

- `.on_open(|row| effect)`: double click or Enter on a row.
- `.row_action(label, action, |row| input)`: a button on each row.
- `.bulk_action(label, action, M::id)`: runs on the selected rows.
- `.confirm(message)` and `.tone(Tone::Danger)` right after `.row_action` or `.bulk_action`: asks before that action runs, in the same dialog as `ActionButton::confirm` (Cancel has the focus first), and draws its button in red.
- `.row_menu(|row| ContextMenu)`: the right click menu of a row.
- `.row_style(|row| Option<RowStyle>)`: danger, warning or success rows.
- `.exportable()`: CSV export of the filtered rows, streamed by the server.

The [DataGrid reference](grid.md) covers saved views, master detail, grouping, tree data, inline editing, cell navigation, copy and paste, column virtualization and accessibility.
