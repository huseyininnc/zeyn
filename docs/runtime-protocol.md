# Runtime Protocol

Status: draft, implemented by `zeyn::server` and `assets/zeyn.js`.

This is the contract between the Zeyn server and the browser runtime. Screens are rendered on the server. The browser runtime is a small script that sends user actions to the server and applies what comes back. Application code never writes JavaScript.

## 1. Page load

`GET {route}?params={json}` returns a full HTML document. `params` is the URL encoded JSON of the screen params and is omitted when the screen has none.

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="zeyn-csrf" content="TOKEN">
  <title>Products · Inventory</title>
  <link rel="stylesheet" href="/_zeyn/zeyn.css">
  <script type="module" src="/_zeyn/zeyn.js"></script>
</head>
<body>
  <a class="z-skip" href="#z-main">Skip to content</a>
  <div class="z-app">
    <nav class="z-nav" aria-label="Main">...</nav>
    <main class="z-main" id="z-main">SCREEN</main>
  </div>
  <div class="z-toasts" role="status" aria-live="polite"></div>
</body>
</html>
```

The menu lists only the screens the current user may open. The current item has `aria-current="page"`.

## 2. Screen markup

Every screen root carries its name and params:

```html
<section class="z-screen" data-zeyn-screen="product_list" data-zeyn-params="null" aria-labelledby="product_list-title">
```

Windows use `class="z-window" role="dialog" aria-modal="true"` and a `--z-width` style.

Each body item is wrapped in a node element. Its path is the address used by the endpoints below. Containers (`Row`, `Stack`, `Panel`, `Card`, `Tabs`, `Split`) wrap each child the same way, and the child's path adds its index: the third child of the first child of body item 1 is `"1.0.2"`.

```html
<div class="z-node" data-zeyn-node="1">
  <div class="z-node" data-zeyn-node="1.0">...</div>
</div>
```

Endpoints take `node` as a dotted string or, for body items, as a plain number (`0`). Paths have at most 16 levels of digits; anything else is refused with `422`.

## 3. Attributes the runtime reads

| Attribute | On | Meaning |
|---|---|---|
| `data-zeyn-effect` | button, `tr` | JSON effect to run on click (rows: on double click or Enter) |
| `aria-keyshortcuts` | button | Keyboard shortcut, for example `Control+N` |
| `data-zeyn-export` | button | Export format of the grid on the same screen (`csv`, `xlsx`) |
| `data-zeyn-grid` | grid root | Marks a grid; also has `data-zeyn-mode`, `data-zeyn-paging`, `data-zeyn-page-size`, `data-zeyn-total`, `data-zeyn-next`, `data-zeyn-offset` |
| `data-field`, `aria-sort`, `aria-colindex` | `th` | Column field name, current sort, position |
| `data-filter`, `data-options` | `th` | Filter input kind (`text`, `number`, `money`, `date`, `datetime`, `time`, `choice`, `bool`) and choice options `[{value,label}]` |
| `data-pin` | `th` | `start` or `end` for pinned columns (also the selection and row action columns) |
| `data-edit`, `data-optional` | `th` | Input kind of an editable column, for users who may save |
| `data-zeyn-texts`, `data-zeyn-columns`, `data-zeyn-view` | grid root | Localized control texts, every column `[{field,label,hidden,pin,width}]`, the saved view |
| `data-zeyn-views` | grid root | `server` when views are saved on the server |
| `data-zeyn-version-field`, `data-zeyn-exportable` | grid root | Row version field name for inline edits; `csv` when the grid can be exported |
| `data-zeyn-key`, `data-zeyn-version`, `data-zeyn-menu` | `tr` | Row key, row version, context menu items |
| `data-v` | `td` | Input value of an editable cell |
| `data-zeyn-detail`, `aria-expanded` | `tr` | Detail screen of the row as `{screen, params}`; whether it is open |
| `data-zeyn-expand` | button in a row | Opens or closes the row's detail, group or children |
| `data-zeyn-level`, `aria-level`, `data-zeyn-path` | `tr` in a `treegrid` | Depth (0 at the top) in a grouped or tree grid; the path of group values or ancestor keys to send when the row opens |
| `z-group` | `tr` | A group header row; `z-branch-more` is a "Load more" row of a branch |
| `data-zeyn-run` | button in a row | Row item to run (`a0` is the first row action) |
| `data-agg` | `tfoot td` | Totals the column shows (`sum avg min max count`) |
| `data-zeyn-bulk` | div before the grid | Selection bar with `data-zeyn-bulk-run` buttons |
| `data-zeyn-menu` | screen root | Context menu of the screen |
| `data-zeyn-native-menu="off"` | `html` | The browser's context menu is off except in text inputs |
| `data-zeyn-rows` | `tbody` | Where rows go |
| `data-zeyn-page` | pager button | `prev` or `next` |
| `data-zeyn-page-info` | span | Text such as `1-50 of 230`, built from the localized templates on the grid root |
| `data-zeyn-text-range`, `data-zeyn-text-range-total`, `data-zeyn-text-loaded` | grid root | Localized templates: `{from}-{to}`, `{from}-{to} of {total}`, `{count} rows loaded` |
| `data-zeyn-more` | div | Infinite scroll sentinel, contains the "Load more" button |
| `data-zeyn-submit` | form | Name of the submit action |
| `data-zeyn-editor` | input | Client side editor of a custom field type |

Effects:

| Effect | JSON | Runtime does |
|---|---|---|
| Open window | `{"effect":"open_window","screen":"product_form","params":{...}}` | Loads the screen (section 4.1) into a modal `<dialog>` |
| Navigate | `{"effect":"navigate","screen":"...","params":{...}}` | Loads `/_zeyn/route/{screen}?params=` which redirects to the page |
| Toast | `{"effect":"toast","message":"Saved"}` | Shows a toast for 4 seconds |
| Close and refresh | `{"effect":"close_and_refresh"}` | Closes the top dialog and reloads every grid on the screen under it |

## 4. Endpoints

All `POST` requests send JSON, must carry the header `x-zeyn-csrf` with the token from the `zeyn-csrf` meta tag, and are rejected with `403` otherwise.

### 4.1 Screen fragment

`GET /_zeyn/screen/{screen}?params={json}` returns the screen HTML only, without the page shell. Used for windows and for row details: opening a row with `data-zeyn-detail` loads its screen into a new `tr.z-detail` right under it (one cell spanning the row, holding a `role="region"`), and closing removes that row. The screen's permission is checked like any screen; grids inside the detail talk to the server with the detail screen's own name and params.

### 4.2 Grid rows

`POST /_zeyn/grid`

```json
{
  "screen": "product_list",
  "params": null,
  "node": 0,
  "query": {
    "page": { "type": "offset", "offset": 0, "limit": 50 },
    "sort": [{ "field": "code", "desc": false }],
    "filters": [{ "field": "name", "contains": "tea" }, { "field": "price", "from": "10", "to": "99.90" }],
    "with_total": true,
    "aggregates": [{ "field": "price", "op": "sum" }]
  },
  "view": { "columns": [{ "field": "code", "width": 140, "pin": "start" }, { "field": "name" }] },
  "head": false
}
```

Filters may set `contains`, `starts_with`, `equals`, `from`, `to`, `any_of` and `is` (see `docs/grid.md`). `view.columns` is the column layout to render the rows in; with `head: true` the response also has `head`, the header row.

For infinite scroll `page` is `{ "type": "after", "cursor": "...", "limit": 100 }` and `cursor` is `null` for the first batch.

Response `200`:

```json
{ "rows": "<tr>...</tr>", "foot": "<tr>...</tr>", "head": null, "count": 50, "total": 230, "next": "50" }
```

`foot` is the totals row when totals were asked for, otherwise `null`.

`total` and `next` may be `null`. The runtime replaces the rows for paged grids and appends them for infinite scroll.

Grouped and tree grids (`<table role="treegrid">`) send `path` with `POST /_zeyn/grid` to load a branch: the group values (or ancestor keys) of the row being opened, as found in its `data-zeyn-path`. The response has the same shape; `rows` holds group rows or rows one level deeper, and `total` counts them. The runtime puts them after the opened row and removes every deeper row that follows it when the row closes. A path longer than the grid's group levels (or 32 in a tree) is `404`.

### 4.2.1 More grid endpoints

| Endpoint | Body | Does |
|---|---|---|
| `POST /_zeyn/grid/view` | `{screen, params, node, view}` | Saves the user's view (`null` deletes it). `204`; `404` without a view store |
| `POST /_zeyn/grid/run` | `{screen, params, node, key, item}` | Runs a row item (`a0` row action, `m2.1` menu path). The server loads the row by key and builds the input. `{effects}` |
| `POST /_zeyn/grid/bulk` | `{screen, params, node, action, keys, all, query}` | Runs a bulk action with the picked keys, or with every row matching `query.filters` when `all`. `{effects, count}` |
| `POST /_zeyn/grid/edit` | `{screen, params, node, key, values, view}` | Saves an inline edit; `values` hold the edited columns and the row version as a form sends them. `{row}`; `403` for columns that are not editable, `409`, `422` |
| `GET /_zeyn/grid/export` | `?screen&params&node&q={sort,filters,columns}` | Streams every matching row as CSV |
| `POST /_zeyn/menu` | `{screen, params, item}` | Runs an action item of the screen's context menu |

### 4.3 Form submit

`POST /_zeyn/form`

```json
{ "screen": "product_form", "params": { "id": 7 }, "node": 0, "values": { "code": "P-001", "price": "1.234,56", "active": true } }
```

Values are sent as the user typed them: strings for inputs and selects, booleans for checkboxes. The server parses them by field type.

The server starts from the model the screen loaded and overwrites only the fields that are on the form and not read only. Other values from the client are ignored. Keys never come from the client.

Responses:

- `200 { "effects": [ ... ] }`: saved; run the effects in order.
- `422 { "errors": [ { "field": "code", "rule": { "rule": "required" }, "message": "Code is required." } ] }`: show each message under its field (`#{scope}-{field}-error`), set `aria-invalid="true"` on the input and focus the first invalid input. Errors with an empty `field` are shown at the top of the form.

### 4.4 Errors

Any endpoint may answer with:

| Status | Body | Meaning |
|---|---|---|
| `403` | `{ "error": "forbidden", "message": "..." }` | Missing permission, guard denied or bad CSRF token |
| `404` | `{ "error": "not_found", "message": "..." }` | Unknown screen or node |
| `409` | `{ "error": "conflict", "message": "..." }` | Someone else changed the record |
| `422` | see 4.3 | Validation |
| `504` | `{ "error": "timeout", "message": "..." }` | The call took longer than the server limit |
| `500` | `{ "error": "internal", "message": "An unexpected error occurred." }` | Details are only in the server log |

The runtime shows the `message` as an error toast.

### 4.5 Language

`POST /_zeyn/locale` with `{ "locale": "tr" }` stores the choice in the `zeyn_locale` cookie and answers `204`. The runtime reloads the page. Without a cookie the server picks from `Accept-Language`, then the app default. `<html lang>` always matches the chosen locale.

The shell renders a language selector (`select[data-zeyn-locale]`) when the app has more than one locale.

### 4.7 Enterprise endpoints

`enterprise.js` and `enterprise.css` load next to the runtime. Details are in `docs/enterprise.md`.

| Endpoint | Purpose |
| --- | --- |
| `POST /_zeyn/reveal {screen, params, node, field, key?}` | Raw value of a masked field, `{value}`; needs the unmask permission, always audited |
| `POST /_zeyn/reason {screen, reason}` | Reason for a screen marked `reason`; `204` with an `HttpOnly` cookie |
| `POST /_zeyn/import {screen, params, node, csv, commit}` | CSV preview (`commit: false`) or import |
| `POST /_zeyn/keepalive` | "Stay signed in" after the idle warning; `204` |
| `GET /_zeyn/health`, `GET /_zeyn/ready` | Liveness and readiness; not rate limited |
| `GET /_zeyn/jobs` | Job status for users with the jobs permission |

Every response carries `x-zeyn-build`. Refusals the runtime acts on: `428 reason_required` (asks for a
reason and repeats the request) and `401 reauth_required` (asks the user to sign in again and repeats
the request).

## 5. Security

- Every response sets `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'`, `X-Content-Type-Options: nosniff` and `Referrer-Policy: same-origin`.
- No inline scripts or inline event handlers. Inline `style` attributes are used only for CSS custom properties (`--z-width`, `--z-flex`) and fixed column widths, so the policy adds `style-src-attr 'unsafe-inline'`. Scripts stay strict.
- The CSRF token is stored in an `HttpOnly`, `SameSite=Strict` cookie and compared in constant time with the `x-zeyn-csrf` header.
- Each call checks, in order: authentication, the screen's access, the feature's guards, the node action's access, then runs the interceptors and the action.

### 4.6 Virtual scrolling

Grids with `data-zeyn-paging="virtual"` show one scrollable list over every row, however many there are. The server renders the first block (`data-zeyn-page-size` rows, offset 0) with `data-zeyn-total`. The grid root also has `data-zeyn-row-height` (pixels) and the CSS variable `--z-row-height`.

The runtime:

- Puts the table body in a scroll container whose inner spacer is `total * rowHeight` pixels high, capped at 8,000,000 px. Above the cap, scroll position maps to row index by ratio (`row = scrollTop / (spacerHeight - viewport) * (total - visibleRows)`), so 1,000,000 rows still reach the end.
- Keeps only the visible rows plus 20 rows above and below in the DOM, positioned with `transform: translateY(...)`. Rows have a fixed height; long text is cut with an ellipsis.
- Loads blocks of `data-zeyn-page-size` rows with `POST /_zeyn/grid` and `page: { "type": "offset", "offset": blockStart, "limit": blockSize }`, `with_total: false` after the first call. Keeps up to 20 blocks in a cache (least recently used out), prefetches the next block in the scroll direction, and cancels requests for blocks that left the view (`AbortController`).
- Never clears the visible rows while a block loads. Missing rows show a placeholder row with `aria-busy="true"` until their block arrives, so the screen does not flash.
- Sort and filter changes drop the cache and scroll to the top.
- Keyboard: Arrow keys, Page Up, Page Down, Home and End move the active row and scroll it into view. The table has `aria-rowcount` = total and each row `aria-rowindex`.
- The position text (`data-zeyn-page-info`) uses `data-zeyn-text-range-total`.

## 6. Runtime behavior

- Without JavaScript, pages still render with the first page of every grid. Forms and paging need the runtime.
- Keyboard: arrow keys move between grid rows, Enter opens a row, Escape closes the top dialog, shortcuts from `aria-keyshortcuts` work everywhere except inside text inputs. In grids: Space selects the row, F2 edits it, Ctrl+Z undoes the last edit, Shift+F10 or the context menu key opens the menu of the row or header, Shift+click sorts by several columns, Right and Left (mirrored in right to left pages) open and close a row's detail, group or children, and Left on a child row goes to its parent. Right on a row moves into its cells: arrows, Home, End, Ctrl+Home, Ctrl+End, Tab and Shift+Tab move the cell cursor, Shift grows a range (`aria-selected` on cells), Ctrl+C copies the range as tab separated text and Ctrl+V pastes into editable cells, one `POST /_zeyn/grid/edit` per changed row. Grids with 40 or more columns keep only the columns in view in the page (a `colgroup` with fixed widths, `td.z-col-gap` spacers with `colspan`, `aria-colindex` on cells).
- Right click opens Zeyn's menu where one is defined (headers, rows with `data-zeyn-menu`, screens with `data-zeyn-menu`); everywhere else the browser's own menu stays.
- Focus moves into a dialog when it opens and returns to the opener when it closes.
- Client side checks (`required`, `minlength`, `maxlength`, `min`, `max`, `type="email"`) run before submit with the browser's constraint validation API. The server always validates again.

## 7. Component library

`zeyn::ui` components are served by two more assets, linked in the page head after the runtime: `/_zeyn/components.js` and `/_zeyn/components.css`. Like the runtime, they need no sign in.

### 7.1 Endpoints

All are `POST` with JSON and the CSRF header, and check the screen's access and the feature's guards first.

| Endpoint | Body | Answer |
|---|---|---|
| `/_zeyn/node` | `{ screen, params, node }` | HTML of one node with the first page of its grids. Used by lazy tabs. |
| `/_zeyn/data` | `{ screen, params, node, key, input }` | JSON from the data source `key` of the node: lookup options (`key` is the field name), tree children (`key` is empty), step checks (`key` is `_step`). The source's action permission is checked and the interceptors run, as for grids. Input that does not fit the action is refused with `404`. |
| `/_zeyn/parse` | `{ kind, text, label }` | How the server reads a typed date or time in the user's language, zone and clock: `{ ok, text, iso, utc, zone }` or `{ ok: false, message }`. `kind` is `date`, `time`, `datetime`, `local` or `presets` (today and quick ranges). |

### 7.2 Values the forms submit

- Lookups submit ids as text: `"7"`, or `"3,7,9"` for several.
- Lines submit `"{field}": "rows"` and one value per cell as `"{field}.{line}.{column}"`, plus `"{field}.{line}._i"`, the index of the loaded line the row came from (empty for new lines). The server rebuilds each line from its loaded line, so only shown, editable columns change.
- Validation errors of a line use `{ "field": "lines", "rule": { "rule": "line", "value": { "row": 0, "field": "quantity" } } }`.

### 7.3 Events

| Event | On | Detail |
|---|---|---|
| `zeyn:content` | `document` | Element with new content (navigation, lazy panels). Both scripts set up what it holds. |
| `zeyn:errors` | the form | The `422` errors, after the runtime showed them. The line editor and stepped forms use it. |
| `zeyn:effect` | `document` | An effect the runtime does not know: `{ effect, source, run }`. `run(next)` runs another effect. Used by `confirm`. |

Effect `{"effect":"confirm","message":"...","title":"...","ok":"...","cancel":"...","then":{...}}` asks in a modal alert dialog and runs `then` only after the user confirms.

### 7.4 Media

`Video` and `Audio` are served by `/_zeyn/media.js` and `/_zeyn/media.css`, linked after the component library. See `docs/media.md`.

| Endpoint | Does |
|---|---|
| `GET /_zeyn/media/{source}/{id}` | A file of a registered media source with range requests: `200`, `206`, `304`, `416`. |
| `GET /_zeyn/media/position?source=&id=` | `{ at, kept }`, where the signed in user stopped; `kept` is `false` when the source keeps no positions (the player then uses local storage). |
| `POST /_zeyn/media/position` | `{ source, id, at }` with the CSRF header saves it: `204`, `422` for a bad position. |

The player dispatches the cancelable `zeyn:media-adaptive` event (`{ media, kind, url, hls, dash }`) on its root before falling back to progressive files; a streaming library calls `preventDefault()` to take over.

## 8. Desktop class shell

`/_zeyn/pro.js` and `/_zeyn/pro.css` load after the component library. Like the runtime they need no sign in.

### 8.1 Commands and the palette

Commands are declared in Rust (`Command::open`, `Command::navigate`, `Command::effect`, `Command::action`) and registered with `Feature::command`. The shell renders one hidden button per command the user may run (feature guards pass and the command's access allows it) inside `div[data-zeyn-commands]`:

| Attribute | Meaning |
|---|---|
| `data-zeyn-command` | Command id |
| `data-zeyn-effect` | Effect of an effect command, run by the runtime on click |
| `data-zeyn-command-run` | Id of an action command, run with `POST /_zeyn/command` |
| `aria-keyshortcuts` | Normalized shortcut (`Control+Alt+N`); the runtime's shortcut handling clicks the button |
| `data-hint`, `data-icon` | Shortcut as shown to users, icon name |
| `data-text-*` on the container | Localized palette texts |

`App::check` refuses shortcuts that cannot be read, single character shortcuts (WCAG 2.1.4), the palette's own keys (`Control+Shift+P`, `F1`) and shortcuts or ids used by two commands.

Ctrl+Shift+P (Command+Shift+P on macOS) and F1 open the palette, also from `button[data-zeyn-palette-open]` in the menu. It is a modal `<dialog>` with an ARIA combobox and listbox. It lists the app's commands, the toolbar buttons, menu bar commands and context menu items of the current screen (the top window when one is open) and the screens in the menu. Matching is fuzzy and ignores case and accents; the eight most recently run commands come first (`localStorage` key `zeyn.palette.recent`). Escape closes it and focus returns to where it was.

| Endpoint | Body | Answer |
|---|---|---|
| `POST /_zeyn/command` | `{ command }` | Runs an action command with the input fixed on the server: CSRF, feature guards, the action's permission and the interceptors. `{ effects }`; `404` for unknown ids and effect commands |

The runtime listens to `zeyn:run` on `document` so other scripts run work through it: `{ effects, source }` runs effects, `{ screen, item }` runs a screen context menu item, `{ error }` shows an error toast.

### 8.2 Menu bar, toolbox, property inspector, status bar

| Markup | Behavior |
|---|---|
| `ul[role=menubar][data-zeyn-menubar]` | W3C APG menubar. Top items are `button[role=menuitem][aria-haspopup=menu]` with a roving tab stop; each controls a `ul[role=menu]` that is `hidden` while closed. Items carry the command attributes of 8.1. Left and Right move between menus, Down, Enter and Space open, Up opens at the last item, Home and End jump, a letter moves to the next item starting with it, Escape closes and returns to the bar, Tab and a click outside close. Menus and items the user may not run are not rendered. |
| `div[role=toolbar][aria-orientation=vertical][data-zeyn-toolbox]` | Toggle group of `button.z-tool[aria-pressed]`. Arrows, Home and End move the roving tab stop; a click, Enter or Space turns one tool on, sends `zeyn:tool` (`detail.tool`) and writes `data-text-selected` into the status bar. |
| `section.z-inspector` | A form with `Form::section` groups (`details.z-form-section` with `role=group` bodies) saved through `POST /_zeyn/form` at the inspector's node path. Sections holding failing fields open before the browser's checks and after a `422`. |
| `div[data-zeyn-statusbar]` | Items at the start and end and `[data-zeyn-status-live]` (`role=status`, `aria-live=polite`). `zeyn:status` (`detail.text`) from any element of the screen replaces the live text. |

### 8.3 Docking panels

`div.z-dock[data-zeyn-dock]` is a grid of four `section.z-dock-area[data-area]` (`left`, `center`, `bottom`, `right`) and three `div.z-dock-bar[role=separator][data-edge]` dividers. Sizes are the custom properties `--z-dock-left`, `--z-dock-right` (percent of the width) and `--z-dock-bottom` (percent of the height), 10 to 60. Each area has a `role=tablist` of draggable `button.z-dock-tab[data-panel]`, an options button and `role=tabpanel` panels holding the child nodes (paths as in any container; they never change when a panel moves). Side areas without open panels are hidden.

Keyboard: Left and Right (mirrored in right to left pages), Home and End move between the open tabs of an area; Delete closes the focused panel; Shift+F10 or the context menu key opens the area's options (move to another area, close, show closed panels); arrows on a divider resize by 5 percent, Home and End go to the limits. A menu bar entry made with `MenuList::panels` lists every panel as a `menuitemcheckbox`. Changes are announced in `[data-zeyn-dock-live]`.

| Endpoint | Body | Answer |
|---|---|---|
| `POST /_zeyn/dock` | `{ screen, params, node, save: false }` | `{ layout }`: the signed in user's saved layout or `null` |
| `POST /_zeyn/dock` | `{ screen, params, node, save: true, layout }` | `204`; `layout: null` deletes it |

`layout` is `{ sizes: { left, right, bottom }, panels: [{ id, area, open, active }] }` in tab order. The server checks CSRF, the screen's access and the feature's guards, keeps only the dock's own panels (each once) and clamps the sizes, then stores it in the app's view store (`App::datagrid_views`) under the tenant, screen and node. Without a store the endpoint answers `404` and without a user `403`; the browser then keeps the layout in `localStorage`.
