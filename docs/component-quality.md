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

Two columns fail for every component and are listed once here instead of in each row:

- **Screen reader run:** no recorded NVDA or VoiceOver check exists in the repository. ARIA roles
  are in place (and are counted under "Pattern"), but nobody has recorded a run.
- **Browser tests:** there is no automated browser test suite. `AGENTS.md` asks for a manual
  check in the showcase or the docs app.

Columns: **Pat** pattern and keys, **Foc** focus, **Con** contrast in both themes, **RTL**,
**i18n**, **Srv** server checks, **CSP** strict CSP and no reload, **Perf** budget measured,
**Tch** touch and small screens, **DPI** high DPI, **Rs** Rust tests, **Doc** live docs example,
**Shw** showcase entry, **MCP** schema.

| Component | Pat | Foc | Con | RTL | i18n | Srv | CSP | Perf | Tch | DPI | Rs | Doc | Shw | MCP |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `DataGrid` | pass | pass | pass | pass | pass | pass | pass | part | part | pass | pass | pass | pass | pass |
| `Form` | pass | part | pass | part | pass | pass | pass | fail | fail | pass | pass | pass | pass | pass |
| `Lines` | pass | part | pass | part | pass | pass | pass | fail | fail | pass | pass | pass | pass | pass |
| Text, number, money inputs | pass | pass | pass | part | pass | pass | pass | fail | fail | pass | part | pass | pass | pass |
| Check box, switch, radio, segmented | pass | pass | pass | part | pass | pass | pass | fail | fail | pass | part | pass | pass | pass |
| Select and listbox | pass | pass | pass | part | pass | pass | pass | fail | fail | pass | part | pass | pass | pass |
| Lookup and `lookup_many` | pass | part | pass | part | pass | pass | pass | fail | fail | pass | pass | pass | pass | pass |
| Date, time and range pickers | part | part | pass | part | pass | pass | pass | fail | fail | pass | pass | pass | pass | pass |
| `Row`, `Stack`, `Card` | n/a | n/a | pass | part | n/a | n/a | pass | fail | part | pass | pass | pass | pass | pass |
| `Panel` | pass | pass | pass | part | pass | n/a | pass | fail | fail | pass | pass | pass | pass | pass |
| `Tabs` | pass | pass | pass | part | pass | n/a | pass | fail | fail | pass | pass | pass | pass | pass |
| `Split` | pass | pass | pass | part | pass | n/a | pass | fail | part | pass | pass | pass | pass | pass |
| `Dock` | part | part | part | part | pass | part | pass | fail | fail | pass | pass | fail | pass | pass |
| `Tree` | part | pass | pass | part | pass | pass | pass | fail | fail | pass | part | pass | pass | pass |
| `Chart` | part | n/a | pass | fail | pass | n/a | pass | fail | fail | pass | part | pass | pass | pass |
| `Alert`, `Badge`, `Progress` | pass | n/a | pass | part | pass | n/a | pass | fail | part | pass | pass | pass | pass | pass |
| `DescriptionList`, `Timeline` | pass | n/a | pass | part | pass | n/a | pass | fail | part | pass | pass | part | pass | pass |
| `EmptyState`, `Skeleton` | pass | n/a | pass | part | pass | n/a | pass | fail | part | pass | part | part | pass | pass |
| `Breadcrumb` | part | n/a | pass | part | pass | pass | pass | fail | part | pass | pass | fail | pass | pass |
| `ActionButton` | pass | pass | pass | part | pass | pass | pass | fail | part | pass | pass | pass | pass | pass |
| Window and confirm | pass | pass | pass | part | pass | pass | pass | fail | fail | pass | part | fail | pass | n/a |
| Toast | part | n/a | pass | part | pass | n/a | pass | fail | fail | pass | part | fail | pass | n/a |
| Toolbar and `ContextMenu` | pass | pass | pass | part | pass | pass | pass | fail | fail | pass | pass | fail | pass | pass |
| `MenuBar`, command palette | pass | pass | pass | part | pass | pass | pass | fail | fail | pass | pass | pass | pass | pass |
| `Toolbox`, `StatusBar` | part | part | part | part | pass | n/a | pass | fail | fail | pass | pass | pass | pass | pass |
| `PropertyInspector` | pass | part | part | part | pass | pass | pass | fail | fail | pass | pass | fail | pass | pass |
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
- **Performance.** Only the grid (virtual scrolling, column virtualization, export), media serving
  and the XLSX and PDF exports have published measurements. No component has a stated budget.
- **Touch.** The grid shows its column menu button on `hover: none` devices and controls keep
  24 px targets ([theming.md](theming.md#6-other-accessibility-behavior)); nothing else shows a
  touch check.
- **Rust tests.** Test files per area: `grid.rs` (50 tests), `ui_controls.rs` (4), `ui_data.rs`
  (4, tree and chart), `ui_date.rs` (11), `ui_layout.rs` (8), `ui_lines.rs` (7), `ui_media.rs`
  (12), `ui_pro.rs` (12), `ui_widgets.rs` (5). `EmptyState`, `Live` and `AssistantChat` were not
  found by name in any test file; window and toast are covered only through other tests.
- **Docs.** Live examples exist for `datagrid`, `form`, `controls`, `dates`, `lines`, `layout`,
  `display`, `action`, `tree`, `chart` and `shell` (`examples/docs/src/live.rs`).
- **MCP schema.** `Component::schema` is a required method, so every component has one; windows
  and toasts are effects, not components, and are described in the action and screen schemas.

## 3. What to harden first

In short: add the browser test suite and recorded screen reader runs, set and enforce performance
budgets, make RTL real, check touch, and give every component a live docs example.
