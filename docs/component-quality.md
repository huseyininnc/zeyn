# Component quality bar

The definition of done for every Zeyn component, and how the components that exist today score
against it.

A component is done only when it meets every criterion below that applies to it. "Applies" is
decided per criterion: a `Stack` has no keyboard map, a `Badge` has no server action. A criterion
that does not apply is marked `n/a`, never skipped silently.

## 1. Definition of done

### 1.1 Pattern and keyboard

- The component follows a named [W3C ARIA Authoring Practices](https://www.w3.org/WAI/ARIA/apg/)
  pattern (grid, treegrid, tree, tabs, menu, menubar, menu button, combobox, listbox, dialog,
  slider, switch, tooltip, disclosure, accordion, feed) or says in its docs why none fits.
- Its doc page has a keyboard table: every key, what it does, and where focus goes. Keys that
  differ from the APG pattern are listed with the reason.
- Everything a pointer can do, the keyboard can do: drag has a keyboard alternative (WCAG 2.5.7),
  resize and move have arrow keys, and shortcuts can be turned off or remapped when they are a
  single character (WCAG 2.1.4).

### 1.2 Screen reader check

- Roles, names, states and `aria-*` relations are checked in the accessibility tree.
- A recorded run with NVDA on Firefox and Chrome, and VoiceOver on Safari, covers the main task of
  the component (for a grid: find a row, sort, filter, edit a cell). The script and the date live
  next to the browser test.
- Changing text (totals, counts, errors, status) is announced through a live region with the
  right politeness; nothing is announced twice.

### 1.3 Focus management

- Focus is always visible (2 px ring in `--z-focus`, 3:1 against its background) and never hidden
  behind sticky headers or overlays (WCAG 2.4.11).
- Composite widgets use one tab stop (roving `tabindex` or `aria-activedescendant`).
- Overlays (windows, menus, pickers, popovers) move focus in when they open, trap it when modal,
  and return it to the element that opened them when they close.
- After a server round trip that replaces HTML (paging, refresh, save), focus returns to the same
  logical place, not to the document body.

### 1.4 Contrast, themes, direction and language

- Text 4.5:1, large text and UI parts 3:1, in the light and the dark theme, measured on rendered
  colors as in [theming.md](theming.md#5-verified-contrast). Colors come only from theme tokens.
- `forced-colors` mode keeps every state visible; nothing is shown by color alone.
- Right to left: the component renders correctly under `dir="rtl"` (logical CSS properties,
  mirrored arrows and keys, charts and sliders that run the other way), checked with an RTL locale
  fixture.
- Every user facing string comes from the locale files in English and Turkish; numbers, money,
  dates and times are formatted on the server in the user's language and time zone.
- Long words and long Turkish labels do not break the layout (test with a label twice as long as
  the English one).

### 1.5 Server side permission and validation

- Every request the component makes names an action, a screen or a node path; the server checks
  the permission, the feature guards and the interceptors on every call.
- Inputs are validated on the server with the same rules the browser shows; client limits
  (page size, counts, ids, file names) are never trusted.
- Masked fields stay masked in HTML, exports, logs and AI tool output.
- Writes count against the write rate limit and call `check_csrf`.

### 1.6 Runtime rules

- CSP stays strict: no inline scripts, styles set from script use CSSOM, no `eval`.
- No page reloads: navigation and refreshes go through `nav.js`.
- The component works without its script where that is possible (a form still posts, a link
  still navigates), and shows a usable fallback where it is not.

### 1.7 Performance budget

Each component states its budget in its doc page and a test checks it. Starting budgets, to be
tightened as measurements come in:

| Measure | Budget |
|---|---|
| DOM nodes for the component in its default state | 300 (grids: 1,000 for the visible window) |
| Requests to first useful paint of the component | 0 after the screen HTML (lazy parts: 1 per user action) |
| Added browser JS, gzip, for a new component | 8 KB; heavy components load their own asset only on pages that use them |
| JS heap growth after 100 open and close cycles | under 1 MB |
| Server memory per request | streamed; no buffer that grows with the data |
| Interaction | 60 fps scrolling (p95 frame under 20 ms on the reference machine), input response under 100 ms |

Measured today (2026-11-02, raw and gzip): `zeyn.js` 131 KB and 37 KB, `components.js` 96 KB and
26 KB, `pro.js` 43 KB and 12 KB, `media.js` 21 KB and 6 KB, `nav.js` 6 KB and 2 KB.

### 1.8 Touch, small screens and high DPI

- Targets at least 24 by 24 CSS pixels at every density (WCAG 2.5.8); 44 by 44 is the aim on
  touch screens.
- Hover only behavior has a touch path (long press, a visible button).
- Works at 320 CSS pixels wide without two way scrolling, except grids and canvases which scroll
  inside their own box (WCAG 1.4.10).
- Icons and charts are vector; canvases scale by `devicePixelRatio`.

### 1.9 Tests

- Rust integration tests in `crates/zeyn/tests/` for rendering, schema, permissions and every
  server endpoint, including a nested node path and a refused request.
- Browser tests (Playwright) for keys, focus, live announcements and console errors, in both
  themes and in RTL, run in CI against the showcase or the docs app.
- `compile_fail` doctests for rules the compiler can enforce.

### 1.10 Docs, showcase and AI

- A page in `examples/docs/pages/` with a `@live` example, the keyboard table, the budget and the
  server checks.
- A showcase entry that uses the component with realistic data.
- `Component::schema` describes the component for MCP tools and test generation; actions it calls
  carry doc comments and `ai` attributes where models need guidance.

## 2. Scores

Scored from the code, the docs and the tests on 2026-11-02. `pass` means the docs, code or
tests show it; `part` means some of it is shown and the note in the matrix or below says what is
missing; `fail` means there is no evidence it meets the bar (it may still work, but nothing shows
it); `n/a` means the criterion does not apply.

Two criteria are tracked outside the matrix:

- **Screen reader run:** no recorded NVDA or VoiceOver run exists yet. The accessibility tree of
  every catalog page is checked in as a snapshot (section 4), so a change to roles, names or
  states shows up in review; the manual run below is still to be recorded.
- **Browser tests:** the Playwright suite in `e2e/` runs every catalog page in CI (section 4).
  Components that are not in the catalog (`Dock`, windows, `ContextMenu`, media players, islands,
  `Live`, `Import`, `Transitions`, `PdfLink`, `AssistantChat`) are still checked by hand in the
  showcase or the docs app.

Columns: **Pat** pattern and keys, **Foc** focus, **Con** contrast in both themes, **RTL**,
**i18n**, **Srv** server checks, **CSP** strict CSP and no reload, **Perf** budget measured,
**Tch** touch and small screens, **DPI** high DPI, **Rs** Rust tests, **Doc** live docs example,
**Shw** showcase entry, **MCP** schema.

| Component | Pat | Foc | Con | RTL | i18n | Srv | CSP | Perf | Tch | DPI | Rs | Doc | Shw | MCP |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `DataGrid` | pass | pass | pass | pass | pass | pass | pass | part | part | pass | pass | pass | pass | pass |
| `Form` | pass | part | pass | part | pass | pass | pass | part | part | pass | pass | pass | pass | pass |
| `Lines` | pass | part | pass | part | pass | pass | pass | part | part | pass | pass | pass | pass | pass |
| Text, number, money inputs | pass | pass | pass | part | pass | pass | pass | part | part | pass | part | pass | pass | pass |
| Check box, switch, radio, segmented | pass | pass | pass | part | pass | pass | pass | part | part | pass | part | pass | pass | pass |
| Select and listbox | pass | pass | pass | part | pass | pass | pass | part | part | pass | part | pass | pass | pass |
| Lookup and `lookup_many` | pass | part | pass | part | pass | pass | pass | part | part | pass | pass | pass | pass | pass |
| Date, time and range pickers | part | part | pass | part | pass | pass | pass | part | part | pass | pass | pass | pass | pass |
| `Row`, `Stack`, `Card` | n/a | n/a | pass | part | n/a | n/a | pass | part | part | pass | pass | pass | pass | pass |
| `Panel` | pass | pass | pass | part | pass | n/a | pass | part | part | pass | pass | pass | pass | pass |
| `Tabs` | pass | pass | pass | part | pass | n/a | pass | part | part | pass | pass | pass | pass | pass |
| `Split` | pass | pass | pass | part | pass | n/a | pass | part | part | pass | pass | pass | pass | pass |
| `Dock` | part | part | part | part | pass | part | pass | fail | fail | pass | pass | fail | pass | pass |
| `Tree` | part | pass | pass | part | pass | pass | pass | part | part | pass | part | pass | pass | pass |
| `Chart` | part | n/a | pass | fail | pass | n/a | pass | part | part | pass | part | pass | pass | pass |
| `Alert`, `Badge`, `Progress` | pass | n/a | pass | part | pass | n/a | pass | part | part | pass | pass | pass | pass | pass |
| `DescriptionList`, `Timeline` | pass | n/a | pass | part | pass | n/a | pass | part | part | pass | pass | part | pass | pass |
| `EmptyState`, `Skeleton` | pass | n/a | pass | part | pass | n/a | pass | part | part | pass | part | part | pass | pass |
| `Breadcrumb` | part | n/a | pass | part | pass | pass | pass | part | part | pass | pass | fail | pass | pass |
| `ActionButton` | pass | pass | pass | part | pass | pass | pass | part | part | pass | pass | pass | pass | pass |
| Window and confirm | pass | pass | pass | part | pass | pass | pass | fail | fail | pass | part | fail | pass | n/a |
| Toast | part | n/a | pass | part | pass | n/a | pass | part | part | pass | part | fail | pass | n/a |
| Toolbar and `ContextMenu` | pass | pass | pass | part | pass | pass | pass | fail | fail | pass | pass | fail | pass | pass |
| `MenuBar`, command palette | pass | pass | pass | part | pass | pass | pass | part | part | pass | pass | pass | pass | pass |
| `Toolbox`, `StatusBar` | part | part | part | part | pass | n/a | pass | part | part | pass | pass | pass | pass | pass |
| `PropertyInspector` | pass | part | part | part | pass | pass | pass | part | part | pass | pass | fail | pass | pass |
| `Video`, `Audio` | pass | pass | part | part | pass | pass | pass | part | part | part | pass | fail | pass | pass |
| `Island`, `Canvas` | part | part | n/a | part | pass | pass | pass | part | fail | part | pass | fail | pass | pass |
| `Live` | n/a | part | n/a | n/a | n/a | pass | pass | fail | n/a | n/a | part | fail | pass | pass |
| `Import` | part | part | pass | part | pass | pass | pass | fail | fail | pass | pass | fail | pass | pass |
| `Transitions` | pass | part | pass | part | pass | pass | pass | fail | fail | pass | pass | fail | pass | pass |
| `PdfLink` | pass | n/a | pass | part | pass | pass | pass | part | part | pass | pass | fail | part | pass |
| `AssistantChat` | part | part | part | part | pass | pass | pass | fail | fail | pass | part | fail | pass | pass |

Evidence behind the scores:

- **Pattern.** `DataGrid` documents grid and treegrid roles, `aria-sort`, `aria-colindex` and a
  full keyboard map ([grid.md](grid.md#accessibility)); controls follow their ARIA patterns
  ([controls.md](../examples/docs/pages/controls.md)); tabs, tree, menubar, palette (combobox),
  media sliders and menu buttons are named in their docs. Pickers, toolbox, breadcrumb, toast,
  import and the assistant do not document a keyboard table. Charts expose a text summary and a
  data table but no keyboard path to data points.
- **Contrast.** The token pairs in [theming.md](theming.md#5-verified-contrast) cover the grid,
  forms, menus, alerts and focus; `pro.css`, `media.css`, `assistant.css` and the dock were not
  in that measurement, so they score `part`.
- **RTL.** Arabic pages render `<html dir="rtl">`. The framework stylesheets use logical
  properties, and `tests/rtl.rs` fails on a new physical side unless it is listed with a reason.
  Arrows that point in reading direction mirror; Left and Right swap in grid cells, tabs, menus,
  trees, calendars, sliders, split panes and docks; popups and menus anchor to the start side.
  Numbers align to the end of the cell (`text-align: end`), so left in a right to left grid.
  `Chart` scores `fail` because its SVG lays axes out with fixed left margins (`chart.rs`).
- **Performance.** The grid (virtual scrolling, column virtualization, export), media serving
  and the XLSX and PDF exports have published measurements. Every catalog page has a budget for
  its script and stylesheet bytes and its first paint in `e2e/budgets.json`, and
  `tests/size_budgets.rs` caps each module file and each kind of page. DOM size, heap growth and
  frame times are not measured yet, so the column scores `part`.
- **Touch.** The browser suite opens every catalog page on an emulated touch phone and fails on a
  pointer target under 24 by 24 CSS pixels whose spacing does not make up for it (WCAG 2.5.8),
  and on a page that scrolls sideways at 412 px. Hover only paths and high DPI canvases are not
  checked, so the column scores `part`.
- **Rust tests.** Test files per area: `grid.rs` (50 tests), `ui_controls.rs` (4), `ui_data.rs`
  (4, tree and chart), `ui_date.rs` (11), `ui_layout.rs` (8), `ui_lines.rs` (7), `ui_media.rs`
  (12), `ui_pro.rs` (12), `ui_widgets.rs` (5). `EmptyState`, `Live` and `AssistantChat` were not
  found by name in any test file; window and toast are covered only through other tests.
- **Docs.** Live examples exist for `datagrid`, `form`, `controls`, `dates`, `lines`, `layout`,
  `display`, `action`, `tree`, `chart` and `shell` (`examples/docs/src/live.rs`).
- **MCP schema.** `Component::schema` is a required method, so every component has one; windows
  and toasts are effects, not components, and are described in the action and screen schemas.

## 3. What to harden first

In short: record the screen reader runs, extend the budgets to DOM size, heap growth and frame
times, cover the components that are not in the catalog with browser tests, and give every
component a live docs example.

## 4. Browser tests

The suite lives in `e2e/` (Playwright with `@axe-core/playwright`) and runs against the
component catalog app. It starts the app on `ZEYN_E2E_PORT` (3110 by default) unless one already
runs there:

```
cd e2e && npm ci && npx playwright install chromium && npm test
```

There is one spec per catalog category (`e2e/tests/<category>.spec.ts`), and the pages of each
category come from `e2e/components.json`, which a test of the catalog
(`examples/components/tests/components.rs`) rewrites when a component is added. Every page runs
in four projects:

| Project | Checks |
|---|---|
| `light` | renders; axe with the WCAG 2.2 A and AA rules has no serious or critical finding; Tab moves through the examples and every stop shows focus; in each composite widget (tabs, menu bar, radio group, toolbar, tree, listbox, grid) the arrow key moves focus and keeps it inside; the accessibility tree matches its snapshot; no console error |
| `dark` | renders in the dark theme; axe, including contrast; no console error |
| `rtl` | renders with `dir="rtl"`; the page does not scroll sideways; no console error |
| `touch` | a touch phone (Pixel 7); pointer targets are 24 by 24 CSS pixels or spaced enough (WCAG 2.5.8); the page does not scroll sideways; no console error |

Requests to other hosts get an empty answer, so the suite needs no network. A finding that is not
fixed yet goes in `e2e/known-issues.json` as `slug:check` with the reason, and stays listed until
it is fixed; today that is the kanban board, which widens the page on a phone.

**Budgets.** `e2e/tests/budgets.spec.ts` loads every catalog page in the light project and
measures the script and stylesheet bytes it loads (as sent, so compressed) and its first
contentful paint. A page fails when it goes more than 10 percent over its entry in
`e2e/budgets.json`. First paint budgets are one and a half times the recorded time, at least
1000 ms, since timing differs between machines. After a change that earns its bytes, record new
budgets and give the before and after numbers in the commit:

```
ZEYN_E2E_RECORD_BUDGETS=1 npx playwright test budgets --project light
```

**Accessibility tree snapshots.** The `light` project compares the accessibility tree of each
page's main area with `e2e/tests/__snapshots__/<slug>.aria.yml`. When a change to roles, names
or states is intended, update them with `npm run update` and review the diff like code. Pages
whose text follows the date or the clock (calendar, week schedule, feed, message thread,
composer) have no snapshot.

### Manual screen reader check

The snapshots show what the browser exposes, not what a screen reader says. Before a component
is marked `pass` for the screen reader run, someone records this check and adds a row to the
log below.

1. Run the catalog (`cargo run -p zeyn-example-components`) and open the component's page.
2. NVDA on Windows with Firefox and with Chrome; VoiceOver on macOS with Safari.
3. Reach the component with Tab and with the screen reader's quick keys (headings, landmarks,
   form fields). Its role, name and state are read once and correctly.
4. Do the main task with the keyboard only (for a grid: find a row, sort, filter, edit a cell; for
   a picker: open, choose, close). Every change of state is announced.
5. Trigger the live text (totals, counts, errors, status). It is announced once, politely unless
   it is an error.
6. Open and close every overlay. Focus moves in, stays in while modal, and returns to the opener.
7. Repeat steps 3 to 5 with the page in Turkish and with `dir="rtl"`.

| Date | Component | Reader and browser | Result | By |
|---|---|---|---|---|
| | | | | |
