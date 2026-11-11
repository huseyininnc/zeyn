# Runtime Protocol

Status: draft, implemented by `zeyn::server` and `assets/zeyn/*.js` (served as `/_zeyn/zeyn.js`).

This is the contract between the Zeyn server and the browser runtime. Screens are rendered on the server. The browser runtime is a small script that sends user actions to the server and applies what comes back. Application code never writes JavaScript.

## 1. Page load

`GET {route}?params={json}` returns a full HTML document. `params` is the URL encoded JSON of the screen params and is omitted when the screen has none.

A route with path segments, such as `/orders/{id}`, reads those params fields from the path (`/orders/42`) and the other fields from `params`. A segment value that does not parse answers 404. Values are percent encoded, dots included (`%2E`), so a screen URL never looks like a file to the runtime. A screen that answers `Screen::moved_to` gets `301 Moved Permanently` with the new URL in `Location`; one that answers `Screen::redirect` gets `303 See Other` with `Cache-Control: no-store`. `nav.js` follows both without a page load and shows the target's address, also when the load adds no history entry (back and forward).

A failed page answers with its status (404, 403, 500, ...) and the app's error screen (`App::not_found`, `App::server_error`) or a short built in message, inside the shell. A request with `x-zeyn-partial: 1` gets only that screen with `x-zeyn-partial: 1` and `x-zeyn-title`, so the runtime swaps it in without a page load. Error details never reach the browser.

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="zeyn-csrf" content="TOKEN">
  <title>Products · Inventory</title>
  <link rel="stylesheet" href="/_zeyn/zeyn.css?v=BUILD">
  <script type="module" src="/_zeyn/zeyn.js?v=BUILD"></script>
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

Every framework script and stylesheet in the head carries `?v=` and the build id (`meta[name=zeyn-build]`, from `ZEYN_BUILD_ID` or a hash of the executable); `theme.css` carries a hash of the theme text. A request with the current value gets `Cache-Control: public, max-age=31536000, immutable`, so a warm page load asks for none of them; any other request (an old tab after a deploy, no `v`) gets `no-cache` and the ETag. Modules the runtime imports later take the same `v` from the URL of the script that imports them.

### 1.1 Browser modules

A page loads and runs only the browser code its screen uses. Every page runs the core: `zeyn.js` (requests, effects, windows, forms, menus, shortcuts, the event delegation and the module loader), `nav.js` and `enterprise.js` (response hooks on `fetch` and the idle timeout of the session), 90 KB raw and 29 KB gzip together. Everything else is a module of a component, `/_zeyn/{name}.js` and for some `/_zeyn/{name}.css`, declared in `MODULES` of `crates/zeyn/src/server/modules.rs` with the markers its HTML carries: attribute names (`data-zeyn-date`) and classes (`z-select`), matched as whole names.

| Module | Components |
|---|---|
| `components.js` | the shared part every control module imports: helpers, the popup and listbox cores, selects (the pickers of the app shell), confirm dialogs, alerts, breadcrumbs |
| `tabs.js`, `dates.js`, `choices.js`, `inputs.js`, `lines.js`, `steps.js`, `tree.js`, `overlays.js`, `menubutton.js`, `busy.js` | panels, tabs and split panes; date and time pickers (also grid date filters); listboxes, lookups and multiple selects; numbers, growing text areas, masks and passwords; the lines editor; forms in steps; trees; tooltips and popovers; menu and split buttons; busy overlays |
| `grid.js` | grids, linked as `<link rel="modulepreload">` because `zeyn.js` imports it itself |
| `palette.js`, `pro.js` | the command palette and command buttons; menu bars, toolboxes, status bars, docks and property inspectors |
| `colors.js`, `length.js`, `inspector.js` | color pickers; lengths with a unit switch; property inspectors of islands |
| `forms.js`, `reveal.js`, `import.js` | drafts and workflow transitions of forms; revealing masked values; imports |
| `catalog.js`, `media.js`, `files.js`, `charts.js` and their stylesheets | card grids, rails and search boxes; players, embeds and images; uploads; charts, sparklines, KPI cards and bullets |
| `share.js`, `chips.js`, `reactions.js`, `hero.js`, `state.js`, `preview.js`, `assistant.js` | share buttons, filter chips, reactions, hero slides, the preference store, card previews (first hover), the assistant (chat panels, or the first press of the launcher) |
| `topics.js`, `vlist.js` | nodes that listen to topics and the controls that drive them (`data-zeyn-listens`, `data-zeyn-drives`); virtual lists (`data-zeyn-vlist`) |
| `inbox.js`, `kanban.js`, `calendar.js` and their stylesheets, `diff.css`, `code.css`, `barcode.css` | the inbox and the notification bell; Kanban boards; calendars; diffs, code views and barcodes, which need no script (the copy button of a code view is in `share.js`) |
| `codeedit.js`, `codeedit.css` | code editors (`data-zeyn-code`): the highlighted copy, the gutter, suggestions and error markers; it posts to `/_zeyn/data` only after a pause in typing |
| `range.js`, `quantity.js`, `gallery.js`, `countdown.js` and `commerce.css` | range sliders, quantity steppers, product galleries and countdowns; the stylesheet of every shop component (prices, variant pickers, facet panels, review summaries) |
| `thread.js` and `thread.css`, `stream.js`, `composer.js` and `composer.css`, `typing.js` and `typing.css` | message threads; the streamed Markdown and JSON lines shared by threads and the assistant; chat composers; typing indicators |
| `feed.js`, `lightbox.js` and their stylesheets, `stories.css` | feeds of posts; the lightbox of images and videos; story rings and their rows, which need no script |

A full page links, in its head, the stylesheets every page uses (`zeyn.css`, `components.css`, `content.css`, `pro.css`, `enterprise.css`), the stylesheets of the modules whose markers are in its HTML, and their scripts after the modules they import. `zeyn.js` is served with the same table (`[name, script selector, style selector]`) and loads what is missing when content arrives later: `nav.js` fires `zeyn:need` with `{ root, waits }` before it swaps a screen in and waits for the new stylesheets, windows wait the same way before they open, and `zeyn:content`, grid pages and grid filters load the scripts of what they hold. The browser keeps one instance per URL, so a module runs once. A module sets up what it finds in the document when it runs and listens to `zeyn:content` after that; it never scans for, or listens to, components that are not on the page. Modules import the shared part with the `?v=` of their own URL. `tests/ui_modules.rs` fails when a page links a module it does not use or a module stylesheet styles a class outside its markers.

Measured on the showcase (raw, wire with gzip): a page with only text loads 132 KB of scripts (46 KB) instead of 269 KB (80 KB), a grid page 223 KB (74 KB) instead of 357 KB (106 KB), a form page 186 KB (65 KB) instead of 269 KB (80 KB), and every page 151 KB of styles (35 KB) instead of 177 KB (44 KB). A page that holds every component, such as the component catalog, loads more files than before (68 requests instead of 41, 836 KB instead of 774 KB raw), since each module is a file of its own.

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
| Navigate | `{"effect":"navigate","screen":"...","params":{...}}` | Loads `/_zeyn/route/{screen}?params=` which redirects to the page, in the path form when its route has segments |
| Toast | `{"effect":"toast","message":"Saved"}` | Shows a toast for 4 seconds |
| Close and refresh | `{"effect":"close_and_refresh"}` | Closes the top dialog and reloads every grid on the screen under it |

## 4. Endpoints

All `POST` requests send JSON, must carry the header `x-zeyn-csrf` with the token from the `zeyn-csrf` meta tag, and are rejected with `403` otherwise.

A public screen opened without a session comes with an empty `zeyn-csrf` meta tag and no `Set-Cookie`, so that it can be cached. Before its first `POST` the runtime calls `GET /_zeyn/csrf`, which answers `{ "token": "..." }` with `Cache-Control: no-store`, sets the `zeyn_csrf` cookie when the browser has none, and refuses requests whose `Sec-Fetch-Site` is not `same-origin`. The token goes into the meta tag; when the call fails the `POST` goes out without a token and is refused like any other. Nothing is retried silently.

In-app navigation (`nav.js`) asks for a screen with `x-zeyn-partial: 1` and gets only its HTML, with the page title in `x-zeyn-title`. When the screen sets head tags (description, canonical URL, preview image, robots, JSON-LD), the body starts with `<template data-zeyn-head>` holding them; `nav.js` removes every `[data-zeyn-head]` element of the document head, moves the template's tags there and drops the template. A screen without head tags sends no template, so the old tags are only removed. `nav.js` uses an answer only when its final URL, after redirects, is on the page's origin; otherwise it loads the requested address in full. A `zeyn:navigate` to another origin (also `javascript:` and `data:` addresses) is canceled and ignored with one console warning.

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

### 4.2.2 Virtual scrolling

Grids with `data-zeyn-paging="virtual"` show one scrollable list over every row, however many there are. The server renders the first block (`data-zeyn-page-size` rows, offset 0) with `data-zeyn-total`: the first 64 rows live in the table and the rest in `<template data-zeyn-rows-later>` after it, or every row in the template when the grid shows 40 columns or more. The grid root also has `data-zeyn-row-height` (pixels) and the CSS variable `--z-row-height`.

The runtime:

- Puts the table body in a scroll container whose inner spacer is `total * rowHeight` pixels high, capped at 8,000,000 px. Above the cap, scroll position maps to row index by ratio (`row = scrollTop / (spacerHeight - viewport) * (total - visibleRows)`), so 1,000,000 rows still reach the end.
- Keeps only the visible rows plus 20 rows above and below in the DOM, positioned with `transform: translateY(...)`. Rows have a fixed height; long text is cut with an ellipsis.
- Loads blocks of `data-zeyn-page-size` rows with `POST /_zeyn/grid` and `page: { "type": "offset", "offset": blockStart, "limit": blockSize }`, `with_total: false` after the first call. Keeps up to 20 blocks and at most 100,000 cells in a cache (least recently used out; 7 blocks for a 63 column grid, never fewer than 3), with the blocks kept for a `reload` counted in the same cap and dropped first, prefetches the next block in the scroll direction, and cancels requests for blocks that left the view (`AbortController`).
- Never clears the visible rows while a block loads. Missing rows show a placeholder row with `aria-busy="true"` until their block arrives, so the screen does not flash.
- Sort and filter changes drop the cache and scroll to the top.
- Keyboard: Arrow keys, Page Up, Page Down, Home and End move the active row and scroll it into view. The table has `aria-rowcount` = total and each row `aria-rowindex`.
- The position text (`data-zeyn-page-info`) uses `data-zeyn-text-range-total`.

Card grids (`zeyn::ui::CardGrid`, not rails or lists of rows) window the page instead of a scroll container once they hold 200 cards: `catalog.js` marks the list `data-zeyn-window`, keeps the rows within 800 px of the viewport in it and turns the rows above and below into padding, gives every row the height of the tallest of the first cards (`grid-auto-rows`), and sets `aria-posinset` and `aria-setsize` (`-1` while more pages may come) on each card. A focused card out of the window stays in the DOM with `z-cards-kept`, placed over its own cell. Cards from "Show more" or another page join the same window. While the page scrolls faster than 3 px per ms, cards come in with their images held back (`data-zeyn-park`, the `src` and `srcset` kept aside) and get them back after 120 ms without scrolling.

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

### 4.6 Theme

The shell renders a theme selector (`select[data-zeyn-theme-picker]`) unless the app hides it or offers a single theme. Each option carries `data-scheme` (`light`, `dark`, empty for System) and app themes `data-custom`. On change the runtime sets `data-theme` and `data-zeyn-theme` on `<html>` and the `color-scheme` meta at once, then sends `POST /_zeyn/theme` with `{ "theme": "dark" }`. The server answers `204` with the `zeyn_theme` cookie, or `404` for a theme the app does not offer. Pages are rendered with the theme from that cookie, so there is no reload and no flash. See `docs/theming.md`.

### 4.7 Enterprise endpoints

`enterprise.js` and `enterprise.css` load next to the runtime. Details are in `docs/enterprise.md`.

| Endpoint | Purpose |
| --- | --- |
| `POST /_zeyn/reveal {screen, params, node, field, key?}` | Raw value of a masked field, `{value}`; needs the unmask permission, always audited |
| `POST /_zeyn/reason {screen, reason}` | Reason for a screen marked `reason`; `204` with an `HttpOnly` cookie |
| `POST /_zeyn/import {screen, params, node, csv, commit}` | CSV preview (`commit: false`) or import |
| `POST /_zeyn/keepalive` | "Stay signed in" after the idle warning; `204` |
| `GET /_zeyn/health`, `GET /_zeyn/ready` | Liveness and readiness; health is not rate limited, readiness only from `probe_ips` |
| `GET /_zeyn/jobs` | Job status for users with the jobs permission |

Every response carries `x-zeyn-build`. Refusals the runtime acts on: `428 reason_required` (asks for a
reason and repeats the request) and `401 reauth_required` (asks the user to sign in again and repeats
the request).

## 5. Security

- Every response sets `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'`, `X-Content-Type-Options: nosniff` and `Referrer-Policy: same-origin`. `App::csp` adds named `https` hosts to `img-src`, `media-src` (and `blob:`), `frame-src`, `connect-src` and `font-src`; the other directives never change. The header text is built once at startup.
- No inline scripts or inline event handlers. Inline `style` attributes are used only for CSS custom properties (`--z-width`, `--z-flex`) and fixed column widths, so the policy adds `style-src-attr 'unsafe-inline'`. Scripts stay strict.
- The CSRF token is stored in an `HttpOnly`, `SameSite=Strict` cookie and compared in constant time with the `x-zeyn-csrf` header.
- Pages, the `totp` form and `GET /_zeyn/csrf` never carry the token as it is in the cookie: each response gets a fresh random pad followed by the token XOR the pad (128 hex digits), so gzip cannot leak it (BREACH). The server unmasks before it compares and still accepts the plain 64 digit token.
- Dynamic responses of 1 KB or more (pages, partial screens, grid blocks, exports) are gzipped at level 2 when the browser accepts it, with `Vary: Accept-Encoding`. Never compressed: responses that already have a `Content-Encoding` (the framework assets, compressed once and cached), ranges (`Content-Range`, and any request with `Range`), video, audio, images other than SVG, `application/octet-stream`, archives and PDFs, `text/event-stream`, `application/x-ndjson` (the assistant's stream) and WebSocket upgrades.
- Each call checks, in order: authentication, the screen's access, the feature's guards, the node action's access, then runs the interceptors and the action.

## 6. Runtime behavior

- Without JavaScript, pages still render with the first page of every grid. Forms and paging need the runtime.
- Keyboard: arrow keys move between grid rows, Enter opens a row, Escape closes the top dialog, shortcuts from `aria-keyshortcuts` work everywhere except inside text inputs. In grids: Space selects the row, F2 edits it, Ctrl+Z undoes the last edit, Shift+F10 or the context menu key opens the menu of the row or header, Shift+click sorts by several columns, Right and Left (mirrored in right to left pages) open and close a row's detail, group or children, and Left on a child row goes to its parent. Right on a row moves into its cells: arrows, Home, End, Ctrl+Home, Ctrl+End, Tab and Shift+Tab move the cell cursor, Shift grows a range (`aria-selected` on cells), Ctrl+C copies the range as tab separated text and Ctrl+V pastes into editable cells, one `POST /_zeyn/grid/edit` per changed row. Grids with 40 or more columns keep only the columns in view in the page (a `colgroup` with fixed widths, `td.z-col-gap` spacers with `colspan`, `aria-colindex` on cells).
- Right click opens Zeyn's menu where one is defined (headers, rows with `data-zeyn-menu`, screens with `data-zeyn-menu`); everywhere else the browser's own menu stays.
- Focus moves into a dialog when it opens and returns to the opener when it closes.
- Pending state in the frame of the click: `nav.js` marks the target link current and sets `html[data-zeyn-loading]` while a screen loads, windows and form submits set it too, and grids fade their rows (`aria-busy`) as soon as a sort, page or filter key is pressed (filters wait 150 ms for more keys, and a newer request cancels the older one). A thin bar at the top of the window shows only when the answer takes longer than 150 ms; it is fixed, so nothing moves, and it does not run with `prefers-reduced-motion`.
- Client side checks (`required`, `minlength`, `maxlength`, `min`, `max`, `type="email"`) run before submit with the browser's constraint validation API. The server always validates again.

## 7. Component library

`zeyn::ui` components are served by the modules of section 1.1 and the stylesheet `/_zeyn/components.css`. Like the runtime, they need no sign in.

### 7.1 Endpoints

All are `POST` with JSON and the CSRF header, and check the screen's access and the feature's guards first.

| Endpoint | Body | Answer |
|---|---|---|
| `/_zeyn/node` | `{ screen, params, node }` | HTML of one node with the first page of its grids. Used by lazy tabs. |
| `/_zeyn/nodes` | `{ screen, params, nodes: ["1.0", "2"] }` | `{ nodes: { "1.0": "<div class=\"z-node\" ...>" } }`: several nodes rendered from one run of the screen, for the listeners of a refreshed or driven topic. At most 32 distinct paths; a path not in the screen answers `404`. The screen's access, guards and interceptors run once, and params are read as the screen's typed params. |
| `/_zeyn/data` | `{ screen, params, node, key, input }` | JSON from the data source `key` of the node: lookup options (`key` is the field name), tree children (`key` is empty), step checks (`key` is `_step`). The source's action permission is checked and the interceptors run, as for grids. Input that does not fit the action is refused with `404`. |
| `/_zeyn/data` with `screen` = `_site.{feature}` | as above | The same for a component in a site header slot: `node` is the slot (0 start, 1 center, 2 end, 3 bottom), the node's index in it, then child indexes. The feature's guards and the action's permission are checked. |
| `/_zeyn/parse` | `{ kind, text, label }` | How the server reads a typed date or time in the user's language, zone and clock: `{ ok, text, iso, utc, zone }` or `{ ok: false, message }`. `kind` is `date`, `time`, `datetime`, `local` or `presets` (today and quick ranges). |

### 7.2 Values the forms submit

- Lookups submit ids as text: `"7"`, or `"3,7,9"` for several.
- Lines submit `"{field}": "rows"` and one value per cell as `"{field}.{line}.{column}"`, plus `"{field}.{line}._i"`, the index of the loaded line the row came from (empty for new lines). The server rebuilds each line from its loaded line, so only shown, editable columns change.
- Validation errors of a line use `{ "field": "lines", "rule": { "rule": "line", "value": { "row": 0, "field": "quantity" } } }`.

### 7.3 Events

| Event | On | Detail |
|---|---|---|
| `zeyn:content` | `document` | Element with new content (navigation, lazy panels). The runtime loads the modules it needs and every loaded module sets up what it holds. |
| `zeyn:errors` | the form | The `422` errors, after the runtime showed them. The line editor and stepped forms use it. |
| `zeyn:effect` | `document` | An effect the runtime does not know: `{ effect, source, run }`. `run(next)` runs another effect. Used by `confirm`. |
| `zeyn:topics` | `document` | `{ topics }`: topics to refresh, from another script (`live.js` on a notification). |
| `zeyn:reload` | an element | Reloads the grids inside it in place: the current page, or the first one with `{ reset: true }`. |
| `zeyn:density` | `document` | None. The user picked another density in the account menu (`data-density` on `<html>` already changed); windowed card grids measure their rows again. |

Effect `{"effect":"confirm","message":"...","title":"...","ok":"...","cancel":"...","then":{...}}` asks in a modal alert dialog and runs `then` only after the user confirms.

Effect `{"effect":"refresh","topics":["sales"]}` refreshes the listeners of the topics. `[data-zeyn-listens="a,b"]` marks a listener (the node at its `data-zeyn-node` parent); `[data-zeyn-drives="a"]` marks a control that drives topics: a filter form (`data-zeyn-kinds` says which params are numbers or booleans), a search box (its field is named `q`) or filter chips (each chip carries its params in `data-zeyn-params`). Driving merges the control's values into the screen root's `data-zeyn-params`, replaces the address with them when the screen is the main one, and refreshes the listeners. `topics.js` handles the topics named in one animation frame at once: a listener holding a grid gets `zeyn:reload`, the others of a screen go in one `POST /_zeyn/nodes`, and a newer one for the same screen aborts it and takes over its nodes. Focus and open `<details>` are kept by id, test id, `data-zeyn-key` row and position.

A virtual list is `section[data-zeyn-vlist]` with `data-zeyn-row-height`, `data-zeyn-block` and `data-zeyn-total`; `vlist.js` asks `POST /_zeyn/grid` for blocks with `{ page: { type: "offset", offset }, with_total: true }` and places each `<li>` at `(aria-posinset - 1) * height`. Lookups that depend on another field carry `data-depends-on="field"` and send that field's value as `parent` in their `/_zeyn/data` input.

Client preferences (`zeyn::ClientPref`) live in the browser module `/_zeyn/state.js`, which pages import only when they hold an element with `data-zeyn-remember` or `data-zeyn-prefs-clear`. `GET /_zeyn/prefs` answers `{ preferences }`, whether the app's consent hook allows keeping preference values beyond the session, with `Cache-Control: no-store`. The server reads no stored value.

### 7.4 Media

`Video` and `Audio` are served by `/_zeyn/media.js` and `/_zeyn/media.css`, linked after the component library. See `docs/media.md`.

Players set their sources and poster (`data-zeyn-poster`, `data-zeyn-poster-low` first) when they first come into view, through one `IntersectionObserver` for the page; a root with `data-zeyn-preview` plays muted and looping while a quarter of it is visible, unless `prefers-reduced-motion`, `prefers-reduced-data` or Save-Data says otherwise, until the visitor uses its controls. Images with loading options sit in a box (`.z-img-host`) that shows the placeholder without script and carry `data-zeyn-img`, the marker that loads `media.js`. One capture listener on the document handles `load` and `error` of those images: loaded marks the box `z-img-done` (and `z-img-fade` for a fade over a low file); failed tries `data-zeyn-fallback` once, then shows the box's `<template data-zeyn-img-error>` or the alt text with an icon. Images that ended before the module ran are checked once when it starts.

| Endpoint | Does |
|---|---|
| `GET /_zeyn/media/{source}/{id}` | A file of a registered media source with range requests: `200`, `206`, `304`, `416`. |
| `GET /_zeyn/media/position?source=&id=` | `{ at, kept }`, where the signed in user stopped; `kept` is `false` when the source keeps no positions (the player then uses local storage). |
| `POST /_zeyn/media/position` | `{ source, id, at }` with the CSRF header saves it: `204`, `422` for a bad position. |

### 7.5 Islands

With the `islands` feature, a screen that holds `div[data-zeyn-island]` is served with
`<meta name="zeyn-islands">` (island name to `{ js, wasm }` URLs), `/_zeyn/islands.js` and a policy
with `'wasm-unsafe-eval'`; other pages are unchanged. A partial answer that holds an island carries
`x-zeyn-islands: 1`, and `nav.js` loads it in full when the document lacks the island policy.

| Attribute or endpoint | Meaning |
|---|---|
| `data-zeyn-island`, `data-zeyn-load`, `data-zeyn-props` | Island name, `visible`, `interaction` or `idle`, props JSON |
| `data-zeyn-island-fallback` | Fallback content, hidden once the island mounts |
| `data-zeyn-island-state` | Set by the loader: `loading`, `ready`, `failed` |
| `GET /_zeyn/islands/{file}` | A built file listed in the manifest, cached for a year |
| `POST /_zeyn/data` with `key` = remote name | A typed call from the island, with all the checks of 7.1 |

See `docs/islands.md`.

### 7.6 Realtime

With the `realtime` feature, `div[data-zeyn-live="topic"]` marks nodes that refresh when the server
notifies the topic. Pages holding one link `/_zeyn/live.js`; a partial answer holding one carries
`x-zeyn-live: 1` and `nav.js` imports the script. `zeyn:reload` on an element reloads the grids
inside it in place.

| Endpoint | Does |
|---|---|
| `GET /_zeyn/live/topics?t=a,b` | WebSocket; text frames `{ "topic": "a" }` |
| `GET /_zeyn/live/doc/{kind}/{doc}` | WebSocket; binary frames `0x01` + Y update (v1) both ways |

Handshakes check the origin, the CSRF token (offered as the subprotocol `csrf.TOKEN` next to `zeyn`, never in the URL) against the cookie, the signed in user and the
permissions before upgrading. See `docs/realtime.md`.

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

`App::check` refuses shortcuts that cannot be read, single character shortcuts (WCAG 2.1.4), the palette's own shortcut, shortcuts or ids used by two commands, shortcuts the browser never passes to the page (`BROWSER_RESERVED`: `Ctrl+N`, `Ctrl+T`, `Ctrl+W`, `Ctrl+Tab`, `Alt+F4` and their macOS forms) and, unless the command calls `.override_browser()`, shortcuts the browser uses (`BROWSER_DEFAULTS`: print, save, find, address bar, reload, zoom, developer tools, copy and paste, `F1` to `F12`). Toolbar buttons follow the same rules at run time: such a shortcut is not bound and the reason is logged. Free combinations are `Alt+Shift+<letter>` and `Ctrl+Alt+<letter>`.

`Alt+Shift+P` (Option+Shift+P on macOS) opens the palette by default; `App::palette_shortcut("Ctrl+Shift+P")` changes it, also to a key the browser uses, on purpose. The shortcut is in `data-palette-keys`, and letters match by physical key so Option and AltGr do not break it. The palette also opens also from `button[data-zeyn-palette-open]` in the menu. It is a modal `<dialog>` with an ARIA combobox and listbox. It lists the app's commands, the toolbar buttons, menu bar commands and context menu items of the current screen (the top window when one is open) and the screens in the menu. Matching is fuzzy and ignores case and accents; the eight most recently run commands come first (`localStorage` key `zeyn.palette.recent`). Escape closes it and focus returns to where it was.

| Endpoint | Body | Answer |
|---|---|---|
| `POST /_zeyn/command` | `{ command }` | Runs an action command with the input fixed on the server: CSRF, feature guards, the action's permission and the interceptors. `{ effects }`; `404` for unknown ids and effect commands |
| `POST /_zeyn/search` | `{ query }` | Runs every `Feature::search` the user may run (CSRF, feature guards, the action's permission, interceptors) with the query cut at 100 characters. `{ hits: [{ label, group, url }] }`, at most 20, only app paths |

When the container has `data-search`, the palette sends the query 150 ms after the last key (two characters or more) and lists the hits under the commands. Only the answer to the latest query is drawn. Choosing a hit sends `zeyn:navigate` with the base path in front of `url`; a `#anchor` in it scrolls to that element after the screen is swapped.

The runtime listens to `zeyn:run` on `document` so other scripts run work through it: `{ effects, source }` runs effects, `{ screen, item }` runs a screen context menu item, `{ error }` shows an error toast.

### 8.2 Menu bar, toolbox, property inspector, status bar

| Markup | Behavior |
|---|---|
| `ul[role=menubar][data-zeyn-menubar]` | W3C APG menubar. Top items are `button[role=menuitem][aria-haspopup=menu]` with a roving tab stop; each controls a `ul[role=menu]` that is `hidden` while closed. Items carry the command attributes of 8.1. Left and Right move between menus, Down, Enter and Space open, Up opens at the last item, Home and End jump, a letter moves to the next item starting with it, Escape closes and returns to the bar, Tab and a click outside close. Menus and items the user may not run are not rendered. |
| `div[role=toolbar][aria-orientation=vertical][data-zeyn-toolbox]` | Toggle group of `button.z-tool[aria-pressed]`. Arrows, Home and End move the roving tab stop; a click, Enter or Space turns one tool on, sends `zeyn:tool` (`detail.tool`) and writes `data-text-selected` into the status bar. |
| `section.z-inspector` | A form with `Form::section` groups (`details.z-form-section` with `role=group` bodies) saved through `POST /_zeyn/form` at the inspector's node path. Sections holding failing fields open before the browser's checks and after a `422`. |
| `section.z-inspector[data-zeyn-inspector=name]` | An island inspector with one `template[data-kind]` per field kind. `zeyn:inspect` (`detail: { title, schema, values }`) from inside `[data-zeyn-island=name]` draws the schema with DOM calls, never `innerHTML`; a schema over `data-limits` is ignored and one seen before keeps its fields. A valid, changed value goes to the island root as `zeyn:property` (`detail: { name, value }`); an invalid one fills the field's error line and sets `aria-invalid`. |
| `div[data-zeyn-statusbar]` | Items at the start and end and `[data-zeyn-status-live]` (`role=status`, `aria-live=polite`). `zeyn:status` (`detail.text`) from any element of the screen replaces the live text. |

### 8.3 Docking panels

`div.z-dock[data-zeyn-dock]` is a grid of four `section.z-dock-area[data-area]` (`left`, `center`, `bottom`, `right`) and three `div.z-dock-bar[role=separator][data-edge]` dividers. Sizes are the custom properties `--z-dock-left`, `--z-dock-right` (percent of the width) and `--z-dock-bottom` (percent of the height), 10 to 60. Each area has a `role=tablist` of draggable `button.z-dock-tab[data-panel]`, an options button and `role=tabpanel` panels holding the child nodes (paths as in any container; they never change when a panel moves). Side areas without open panels are hidden.

Keyboard: Left and Right (mirrored in right to left pages), Home and End move between the open tabs of an area; Delete closes the focused panel; Shift+F10 or the context menu key opens the area's options (move to another area, close, show closed panels); arrows on a divider resize by 5 percent, Home and End go to the limits. A menu bar entry made with `MenuList::panels` lists every panel as a `menuitemcheckbox`. Changes are announced in `[data-zeyn-dock-live]`.

| Endpoint | Body | Answer |
|---|---|---|
| `POST /_zeyn/dock` | `{ screen, params, node, save: false }` | `{ layout }`: the signed in user's saved layout or `null` |
| `POST /_zeyn/dock` | `{ screen, params, node, save: true, layout }` | `204`; `layout: null` deletes it |

`layout` is `{ sizes: { left, right, bottom }, panels: [{ id, area, open, active }] }` in tab order. The server checks CSRF, the screen's access and the feature's guards, keeps only the dock's own panels (each once) and clamps the sizes, then stores it in the app's view store (`App::datagrid_views`) under the tenant, screen and node. Without a store the endpoint answers `404` and without a user `403`; the browser then keeps the layout in `localStorage`.

## 9. AI agents: MCP and WebMCP

Outside AI agents reach the app through two doors, both off by default and turned on with
`App::mcp`:

| Setting | Endpoint MCP | WebMCP tools | Writes |
|---|---|---|---|
| `Mcp::off()` (default) | `404` | `404` | no |
| `Mcp::read_only()` | on | on | no |
| `Mcp::on()` | on | on | form saves and API actions that write |

`.server(false)` and `.webmcp(false)` turn one door off. New projects from `zeyn new` start with
`Mcp::read_only()`.

Why off: the endpoint lets any holder of a valid `Authorization` header drive the app through a
model, and a model can be steered by the data it reads. The framework makes every app ready for
agents (tools are generated from the code, nothing to write), but whether an app opens that door,
and whether agents may write, is a decision for the people who run it. Read only is one line away.

### 9.1 Tools

Tools are built from the app for the signed in user:

- one search tool per grid (`{screen}_{node}_search`, input: a `DataQuery`) and, with writes, one
  save tool per form (`{screen}_{node}_save`, input: the form values as typed) of every routed
  screen that opens without params;
- one tool per published API action (`api_{feature}_{action}`). An action whose input is a
  `DataQuery` reads; every other API action counts as a write.

The description is built from the screen title, the first paragraph of the screen's and the
action's `///` doc comment and their `ai = "..."` guidance. Grids and forms the user may not use,
and screens and actions marked `ai = false`, are never listed and are refused when called.

`#[action(confirm)]` marks an action a model may run only after the user confirms it in the app.
`/_zeyn/mcp` neither lists nor runs such tools (nobody on that channel can confirm). WebMCP tools
ask in a modal dialog (`zeyn:confirm`) and then send `confirmed: true` with the form; the built in
assistant asks in its chat (see `docs/assistant.md`).

### 9.2 Endpoints

| Endpoint | Does |
|---|---|
| `GET /_zeyn/tools?screen&params` | `{ tools: [{ name, description, inputSchema, kind, screen, node, writes, confirm }], texts }` for the screen in view. `zeyn.js` registers them with `document.modelContext` |
| `POST /_zeyn/mcp` | JSON-RPC: `initialize`, `tools/list`, `tools/call`. Needs an `Authorization` header and a signed in user |
| `POST /_zeyn/form` with `via_agent: true` | A WebMCP form save; needs `Mcp::on()` with WebMCP. `confirmed: true` only after the user confirmed |

Every agent call runs as the user: the screen's access, guards, the action's permission,
interceptors, rate limits and the time limit. Agent calls are always audited (`via_agent: true`)
and masked fields are always masked for them, even when the user may reveal them. OpenAPI marks
each operation with `x-zeyn-ai`: `false` for `ai = false`, otherwise `{ hint, confirm, writes }`.
