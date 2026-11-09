# Data Grid

Status: implemented by `zeyn::DataGrid`, the grid endpoints in `zeyn::server` and `assets/zeyn/*.js` (served as `/_zeyn/zeyn.js`).

The grid is rendered on the server. Its rows come from an action that takes a `DataQuery` and
returns `Page<E>`. The browser runtime asks for pages, sends what the user does (sort, filter,
layout, edits, actions) and puts the HTML it gets back in place. Every call checks permissions on
the server, and nothing the client sends is trusted.

```rust
DataGrid::of(search_products)
    .mode(DataMode::Server)
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

## Columns known at run time

`DataGrid::dynamic(action)` shows a result whose columns are known only after the query runs,
such as a SQL preview or a data source browser. The action takes a `DataQuery` and returns a
`DynamicPage`: the columns (`DynamicColumn { name, label, kind }`) and one page of rows, each a
JSON array in column order or a JSON object by column name.

```rust
#[action(permission = "reports.preview")]
pub async fn preview(q: DataQuery, db: Reports) -> Result<DynamicPage> {
    let span = q.span();
    db.run_preview(span.offset, span.limit).await
}

DataGrid::dynamic(preview).paging(Paging::virtual_scroll())
```

The grid builds its columns from each page and renders the rows through the typed grid's own
code, so paging, virtual scrolling, column virtualization, keyboard use, copying, resizing,
moving and hiding columns work the same. `DynamicKind` picks how a column reads and shows its
values: `Text`, `Integer` (grouped in the user's locale), `Decimal`, `Bool` (yes and no in the
user's language), `Date`, `Time`, `DateTime` (in the user's time zone) and `LocalDateTime`. A
value that does not read as its kind is shown as text. Each column starts at a width that suits
its kind (180 px for text, 130 for numbers, 120 for dates), so a wide result scrolls sideways.

What the action returns is treated as user data. Column names, labels and cells are escaped like
any other value. The server keeps at most `MAX_DYNAMIC_COLUMNS` (200) columns and the query's page
size of rows (`DataQuery::limit`, at most 500), and cuts names and labels at `MAX_COLUMN_NAME`
(200) and text cells at `MAX_CELL_TEXT` (1,000) characters. Sort keys, filters, totals and groups
are removed from the query before the action runs, and the headers have no sort buttons, so the
action never gets a column name from the client. Access is the action's, as for every grid.

The header carries a fingerprint of the columns (`data-zeyn-shape`). The browser sends it with
every page request and the server sends a new header only when the columns changed, for example
after `.listens(topic)` reloads the grid with a new query. Rows are read only: a dynamic grid has
no editing, row actions, selection, details, grouping or export.

## Column totals

`sum()` and `avg()` work on whole numbers and money, `min()` and `max()` also on dates, times and
moments, `count()` on every column. Using them on another type does not compile. The totals cover
every row that matches the filters, not only the page.

`DataQuery::apply` computes them for in memory data. A data layer that pages in SQL reads
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
by dragging or with the arrow keys (Shift for bigger steps). Dragging a header onto another header
of the same pinned group moves the column; a short press still sorts. All of it works right to left.

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

## Master detail

`detail(|row| (screen, params))` lets every row expand into a screen built from the row, loaded only
when the user opens it:

```rust
DataGrid::of(search_today)
    .column(Order::id)
    .detail(|o| (order_lines, OrderParams { id: o.id }))

#[screen(permission = "sales.order.read")]
pub fn order_lines(params: OrderParams) -> Screen {
    Screen::window(t!("sales.order_lines", id = params.id.0.to_string())).body(
        DataGrid::of(search_lines)
            .scope(OrderLine::order_id, params.id)
            .column(OrderLine::product)
            .column(OrderLine::amount).sum()
            .detail(|l| (product_editor, ProductParams { id: Some(l.product_id) })),
    )
}
```

The params are typed by the screen (`ScreenDef::Params`), so the wrong params do not compile. A
detail is a full screen: a form, a nested grid, or a grid with its own details, to any depth. The
expander column shows only for users who may open the screen, and the server checks the screen's
permission again when the detail loads. The params travel through the browser as a window's do,
so the detail screen must check that the user may see the record they name.

`scope(field, value)` keeps a grid to the rows whose `field` equals `value` on every request. The
filter goes first in `DataQuery::filters` and the client cannot remove it, so a nested grid stays
on its parent row whatever the browser sends. A data layer that builds SQL from the filters reads
it like any other (`Filter::range` with equal bounds for numbers and dates, `equals` for text,
`any_of` for choices, `is` for yes/no, `blank` for empty values).

The expander button, Enter on it, or Right and Left on the row open and close the detail.
Closing removes it, so it loads fresh the next time.

Virtual grids keep every row at one fixed height so any position can be reached at once; they do
not show expanders. Use pages or infinite paging for grids with details.

## Grouping

`group_by(field)` groups rows on the server; call it again to group inside each group:

```rust
DataGrid::of(search_today)
    .paging(Paging::pages(20))
    .group_by(Order::city)
    .group_by(Order::status)
    .column(Order::amount).sum().avg()
```

The grid shows a header row per group with the column label, the value, the row count and every
column total over that group, under its column. Opening a group (its button, or Right and Left)
loads its subgroups or rows, one page at a time with a "Load more" row when more remain; closing
removes them. The top level pages over groups, so the pager counts groups. The footer keeps the
totals over every matching row. Rows inside groups keep everything else: editing, row actions,
menus, selection and details.

The data layer answers `DataQuery::group`: when it is set, it returns `Page::groups` (value, count
and the totals the query asks for, per group) sorted by the value in the direction of the sort key
on that field, paged like rows, with `total` counting groups. `DataQuery::apply` does this for in
memory data; SQL uses `GROUP BY`. `group` is set by the grid and never read from the client. The
client sends only the path of group values it opens; the server checks its length and turns each
value into a filter on that group's column, in front of the user's filters (`blank` for the empty
group).

## Tree data

`tree(Model::parent)` shows rows as a tree by an optional parent key: rows without a parent at the
top, and the children of a row loaded when it opens, a page at a time, to at most 32 levels.

```rust
DataGrid::of(search_employees)
    .tree(Employee::manager_id)
    .column(Employee::name)
    .column(Employee::salary).sum()
```

The first column carries the indent and the button. To know which rows have children, the grid
asks its action once more per page: the rows whose parent is one of the page's keys, grouped by the
parent field (`Filter::any_of` on keys and `DataQuery::group`). Filters and sorting apply on every
level; the footer totals cover every matching row. Tree grids do not group and show no row
details. After an inline edit the row keeps its place in the tree.

Virtual grids neither group nor show trees, since every row there has one fixed height.

## Cells: navigation, copy and paste

Right on a row (or a click on a cell) puts a cell cursor in it, as in a spreadsheet. Arrow keys move
it, Shift with an arrow (or Shift+click) grows a range, Home and End go to the ends of the row and
Ctrl+Home and Ctrl+End to the first and last cell of the grid. Tab and Shift+Tab move along the
row and on to the next one, and leave the grid at its ends. Left on the first cell goes back to the
row. Enter or F2 edits an editable cell, Enter presses the only button or link of a cell, Escape
drops the range. Cells in the range carry `aria-selected`.

Ctrl+C on a cell copies the range as tab separated text, the values as the user sees them. Ctrl+C
on a row still copies the selected rows.

Ctrl+V on a cell pastes tab separated text from a spreadsheet from the cursor; one value fills the
whole range. Only editable cells change (pasting into read only cells says so), choices may be
pasted by label, yes and no in the user's words. Each changed row is saved through the inline edit
path (`/_zeyn/grid/edit` with the row version), one row after another, so every row is read,
validated and checked on the server like an inline edit; a row that fails shows its errors in its
cells and the others are saved. More than 10 rows ask first, more than 500 are refused, and Ctrl+Z
undoes the whole paste. Grid edits, row actions, bulk actions and menu actions count against
`App::write_limit`, so a large paste can run into the app's write limit.

## Column virtualization

A grid with 40 columns or more keeps only the pinned columns and the columns in view (plus up to six
on each side) in the page. Every column gets a fixed width, and one spacer cell stands for the
columns left out on each side, so widths, pins and scrolling stay as they were. Rows keep their full
list of cells, so copy, paste and editing see every column, and cells carry `aria-colindex`.
The window moves only when a column outside it comes into view. The server still sends every
cell of a row. A virtual grid this wide (`VIRTUAL_WIDE_COLUMNS`, counting the selection, detail
and row action columns) sends its whole first block inside `<template data-zeyn-rows-later>`
instead of 64 live rows, so the browser never lays out a row with every column: on the wide
gallery grid the longest startup task went from 259 to 112 ms and DOMContentLoaded from 402 to
228 ms (medians of 8 interleaved loads, release server, busy machine).

Measured on the gallery's wide grid (62 columns, 100,000 rows, virtual scrolling, debug server,
Chromium, 1400 by 900 window, 144 Hz display), with and without column virtualization:

| | Cells in the page | Sideways, 60 px per frame (p50 / p95) | Down, 60 px per frame (p50 / p95) | Down, 2,000 px per frame (p50 / p95) |
|---|---|---|---|---|
| Without | 2,976 | 6.9 / 7.5 ms | 42.7 / 64.8 ms | 78.5 / 141 ms, 178 long tasks |
| With | 1,056 | 7.0 / 28.8 ms | 24.6 / 41.7 ms | 28.9 / 48.9 ms, 7 long tasks |

Scrolling down is about twice as fast and the page holds a third of the cells. Sideways scrolling
was already smooth; frames where the window moves cost up to 29 ms. The rest of the cost is
parsing blocks of 200 rows with every column, which column virtualization does not change.

## Export

`exportable()` (and optionally `export_permission(...)`, `export_limit(n)`) lets the toolbar's
`Export::Csv` button download every matching row. The server pages through the grid's action and
streams each page at once, in the user's layout and language. Formulas are defused. Up to 100,000
rows by default.

With the `xlsx` Cargo feature the toolbar's `Export::Xlsx` button downloads the same rows as an
Excel workbook (`GET /_zeyn/grid/export?format=xlsx`). Cells keep their types: numbers, money as a
number with its currency format, dates and date times as real Excel dates in the user's time zone,
and booleans. Choices are written as their labels, masked fields as their masked text, and text is
always text, so nothing becomes a formula. The header row holds the labels in the user's language,
is bold and frozen, and columns keep the user's order and widths.

The workbook is written with `rust_xlsxwriter` in constant memory mode: each page goes to a
temporary file as it arrives (at most two pages wait in memory), and the zipped workbook is
streamed to the response at the end. Exporting 100,000 rows of six columns kept the heap at about
134 KiB while writing rows (the same after 10,000 and after 100,000 rows) and 460 KiB while
zipping; the file was 3.4 MB.

With the `pdf` Cargo feature, `Export::Pdf` prints the same rows as a list report (at most 2,000
rows); see [PDF documents](operations.md#pdf-documents).

## Selection

`selectable()` (implied by `bulk_action`) adds a checkbox column. Shift selects a range, Space
selects the focused row, and the header box selects the page. The selection bar offers every
matching row.

## Accessibility

The table has `role="grid"`, `aria-rowcount` (virtual grids), `aria-colcount`, `aria-colindex`
and `aria-sort`. Rows use a roving tab stop. Buttons inside the active row are reachable with Tab. Grouped and tree
grids are `role="treegrid"` with `aria-level` and `aria-expanded` on rows.
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

### Keep the screen builder cheap

Requests are stateless, so every grid request rebuilds the screen that holds the grid: each block
a virtual grid fetches (every 200 rows scrolled), each page, sort, filter and export, each inline
edit and each row action runs the `#[screen]` function again before it runs the grid's action.
The server needs the screen to find the grid by its node path and to check access, so this is
the price of not keeping state per user.

A builder that only lays out components costs microseconds. A builder that queries the
database pays that query on every one of those requests. So:

- Load the rows in the grid's action (`DataGrid::of(search)`). It runs once per request anyway
  and gets the `DataQuery` with the page, the sort and the filters.
- Choices and lookups belong in the field type (`#[choice]`), in a form lookup
  (`Form::lookup`, which asks its action when the user types) or in a service that keeps them in
  a capped cache, not in a query the builder runs.
- When a screen must load a record (`Form::of(record)`), keep large or virtual grids off it: put
  them on their own screen, in a detail screen (`DataGrid::detail`) or in a window, so their
  block requests rebuild a screen that loads nothing.
- Screen params are the input the builder needs; keep them to keys, and let the grid's action
  filter by them (`DataGrid::scope`).
