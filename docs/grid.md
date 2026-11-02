# Data Grid

Status: implemented by `zeyn::Grid`, the grid endpoints in `zeyn::server` and `assets/zeyn.js`.

The grid is rendered on the server. Its rows come from an action that takes a `GridQuery` and
returns `Page<E>`. The browser runtime asks for pages, sends what the user does (sort, filter,
layout, edits, actions) and puts the HTML it gets back in place. Every call checks permissions on
the server, and nothing the client sends is trusted.

```rust
Grid::of(search_products)
    .mode(GridMode::Server)
    .paging(Paging::pages(25))
    .column(Product::code).width(120).pin_start().filter()
        .link(|p| Window::open(product_editor, ProductParams { id: Some(p.id) }))
    .column(Product::name).flex(1).filter().editable()
    .column(Product::unit).filter().editable().badge(|u| match u {
        Unit::Kg => Tone::Info,
        _ => Tone::Neutral,
    })
    .column(Product::price).filter().editable().avg().max()
    .column(Product::stock).filter().sum().progress(120)
    .column(Product::active).filter().check_icon().editable()
    .edit_with(save_product)
    .row_action(t!("stock.restock"), restock_product, |p| p.id)
    .bulk_action(t!("stock.deactivate"), deactivate_products, Product::id)
    .row_menu(|p| ContextMenu::new()
        .item(t!("stock.edit"), Window::open(product_editor, ProductParams { id: Some(p.id) }))
        .action(t!("stock.restock"), restock_product, p.id))
    .exportable()
```

## Column totals

`sum()` and `avg()` work on whole numbers and money, `min()` and `max()` also on dates, times and
moments, `count()` on every column. Using them on another type does not compile. The totals cover
every row that matches the filters, not only the page.

`GridQuery::apply` computes them for in memory data. A data layer that pages in SQL reads
`query.aggregates` and adds `Page::with_total(field, op, value)` from one aggregate query. The grid
keeps only the totals its columns declare, so a client cannot ask for others. Totals are formatted
on the server in the user's language. Averages of whole numbers have two decimals
(`Value::Decimal`), and money averages are rounded half away from zero to the smallest unit.

## Filters and sorting

`filter()` adds a filter to the filter row, with the input chosen by the column type:

| Column | Filter | Query |
|---|---|---|
| Text | contains, starts with, equals | `contains`, `starts_with`, `equals` |
| Whole number, money | from, to | `from`, `to` as `1234.56` |
| Date | from, to | `from`, `to` as `2026-03-31` |
| Moment (`DateTime`) | from, to (dates) | a whole day in the user's time zone |
| Time | from, to | `14:30` |
| Choice | pick any of the options | `any_of` |
| Yes/no | all, yes, no | `is` |

Values travel in a fixed machine form, whatever the user's language. The old
`{ "field", "contains" }` shape still works. The server drops filters on columns without
`filter()` and sorts on columns the grid does not have, and it caps filters (32), filter text (200
characters), choices (100) and sort keys (4). A value that cannot be read is ignored. The time zone
for date filters comes from the user's settings on the server.

Shift+click on a header adds that column to the sort keys. A number in the header shows the order.

## Columns: pin, resize, hide, move

`pin_start()` and `pin_end()` keep a column at one edge while the grid scrolls sideways (at most
three per side). `hidden()` leaves a column out until the user shows it. Every header has a
column menu (the button, a right click, Shift+F10 or the context menu key) with sort, pin, move
left and right, hide, filter, a column chooser and reset. The edge of a header resizes the column,
by dragging or with the arrow keys (Shift for bigger steps). All of it works right to left.

## Saved views

The user's layout (shown columns in order, widths, pins), sort and filters form a `DataGridView`.
With `App::datagrid_views(store)` it is saved per user on the server, for each tenant, screen and
grid, and the page comes back in it with the first page already sorted and filtered. Implement
`DataGridViewStore` for a database; `MemoryViewStore` is for development and tests. Without a store
the browser keeps the view in `localStorage`. The server cleans every view it receives with the
grid's own columns and limits.

## Cells

| Method | Columns | Shows |
|---|---|---|
| `badge(\|v\| Tone)` | `#[choice]` enums | a colored badge per option |
| `progress(max)` | numbers, money | a bar and the number |
| `link(\|row\| Effect)` | any | the value as a link that runs an effect |
| `check_icon()` | `bool` | a check mark or a dash, with words for screen readers |
| `relative_time()` | `DateTime` | "3 minutes ago", exact time as tooltip |
| `cell(renderer)` | any | your own HTML (`CellRenderer<T>`) |

A `CellRenderer` must escape user data. Exports and copies use the plain value.

## Row actions, menus and bulk actions

`row_action(label, action, |row| input)` adds a button to each row. `row_menu(|row| ContextMenu)`
adds a context menu. `Screen::context_menu(...)` adds one to the empty space of a screen. Items
that run an action show only for users with that action's permission.

When a row item runs, the client sends only the row key and the item. The server loads the row
again through the grid's action, builds the item from that row, checks the action's permission,
runs the interceptors and then the action. An item that no longer exists for the row (for
example, one that became disabled) is refused. The input never comes from the client.

`bulk_action(label, action, Model::key)` adds a button to the selection bar. The action takes
`Vec<K>`, the type of the key field; anything else does not compile. Picking "all matching rows"
lets the server collect the keys itself with the user's filters, up to 10,000. Keys picked one by
one come from the client, so the action must check each record as an API action would.

The browser keeps its own context menu everywhere else. `App::browser_context_menu(false)` turns
it off except in text inputs and editable text. That protects nothing: users can still read the
page source, open the developer tools or turn scripts off. It also takes away spell check, copy
link, translation and assistive tools. Keep the default.

## Inline editing

`edit_with(save)` with an action that takes `Valid<E>` (the same rule as forms) and `editable()` on
columns. Keys, read only fields and row versions never become editable. F2 or a double click on
an editable cell starts editing the row; Enter saves, Escape cancels, Ctrl+Z or the toast button
undoes the last save.

The server loads the row again by key and applies only the editable columns and the row version
through `Form::read`, so values are read and validated as a form would. A value for any other
column is refused with `403`. A stale version comes back as `409`, and validation errors come back
as `422` and show in their cells.

## Export

`exportable()` (and optionally `export_permission(...)`, `export_limit(n)`) lets the toolbar's
`Export::Csv` button download every matching row. The server pages through the grid's action and
streams each page at once, in the user's layout and language. Formulas are defused. Up to 100,000
rows by default.

## Selection

`selectable()` (implied by `bulk_action`) adds a checkbox column. Shift selects a range, Space
selects the focused row, and the header box selects the page. The selection bar offers every
matching row.

## Accessibility

The table has `role="grid"`, `aria-rowcount` (virtual grids), `aria-colcount`, `aria-colindex`
and `aria-sort`. Rows use a roving tab stop. Buttons inside the active row are reachable with Tab.
Menus use `role="menu"` with `menuitem`, `menuitemcheckbox` and `menuitemradio`, and focus returns
to where it was. Empty and error states are announced (`role="status"`).

## Performance

Measured on the showcase archive (1,000,000 rows, virtual scrolling, debug server, Chromium,
144 Hz display) with the Performance API:

| Scroll | Frame p50 | Frame p95 | Max | Long tasks |
|---|---|---|---|---|
| Wheel speed (60 px per frame) | 13.9 ms | 20.9 ms | 34.6 ms | 0 |
| Fast (2,000 px per frame) | 14.1 ms | 27.8 ms | 54.9 ms | 0 |
| Sweeps over the whole list | 13.9 ms | 27.8 ms | 48.6 ms | 0 |

The DOM holds 62 rows (about 660 elements), and the JS heap stays near 4 MB. Exporting 100,000 rows
from the same archive streamed 7 MB in 3.4 seconds.
