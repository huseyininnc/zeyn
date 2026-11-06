# Changelog

All notable changes to Zeyn are recorded here. The format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/). The project is in alpha, so APIs can change between versions.

## [Unreleased]

### Fixed

- `ActionButton` with `.tone(Tone::Brand)` is drawn as a primary button, so a card can carry its main action. `Badge` with `Tone::Brand` gets its tinted background like the other tones and stays readable over images.

### Security

- `Feature::files` takes a `FileAccess`: `FileAccess::read(p).write(q)` lets readers download files they may not upload. A plain permission name still covers both. Before, a store readable by a role was also writable by it.
- Form submits no longer write the values of password and one time code fields (`Control::Password`, `NewPassword`, `PasswordMeter`, `OneTimeCode`) into the audit trail; the change is recorded with both values left out. This covered the sign in, register, reset and change password forms of the `accounts` feature.

### Added

- `RangeSlider`: two native `<input type="range">` handles over one track for whole numbers, money (`.money(Currency)`) or dates (`RangeSlider::dates`, sent as ISO dates), named like screen params so it works in any form, in a facet panel and with `.drives(topic)` as a filter of its own. Arrow keys, Home and End come from the browser; Page Up and Page Down move a tenth of the range; values from the params are clamped and swapped into order. `range.js` (1 KB gzip, only on pages with a slider) keeps the handles in order, fills the track and reads the track's box once per press, never during a move.
- `Price`: the current price, the old price struck through with the whole percent off (left out when it is not higher or in another currency), a unit price (`PriceUnit::Kg`, `Item`, `Grams100`, `Liter`, `Ml100`, `Meter`) and `.large()`, all `Money` in the page's number format. Screen readers hear one sentence, "Was ₺1,599.00, now ₺1,299.00, 19% off". No script; shop styles are one module stylesheet, `commerce.css`, linked only by pages that hold a shop component.
- Topics: `.listens(topic)` on any component refreshes it in place when an action ends with `ui.refresh(topic)` (`Effect::refresh`), when a filter that `.drives(topic)` changes (`Chips::filter`, `SearchBox`, and a `Form` without submit whose fields are screen params: selects, date ranges, text) or when the realtime hub notifies the topic. Grids reload their page with their sort and filters; other listeners are rendered again with one `POST /_zeyn/nodes` per screen (at most 32 nodes, the screen's access checked once). Driven values go into the screen's typed params and the address; only topic names travel. Refreshes within one frame are merged and a newer one cancels the one running. A filter driving a KPI, a chart and a grid costs two requests (about 2.5 KB) and shows the new content about 280 ms after the key press at 4x CPU throttling. `App::check_page` reports a driven topic nothing listens to; `zeyn check` warns about topics refreshed or driven but never listened to, and the reverse.
- `Form::depends_on(field, source)`: the lookup `field` searches with the value of `source` (`LookupQuery::parent`, `parent_id()`), is cleared when `source` changes, and the submit asks the lookup again and refuses a pair it does not return.
- Builders: `.builder(|row| node)` and `.separator(node)` on `List::of`, `CardGrid` and `Rail`, next to `.item` and `.card`; `Repeat::of(items, |item| node)` for in memory items. Built rows carry their model key (`data-zeyn-key`), so refreshes keep focus and open details. `List::of(..).paging(Paging::virtual_scroll())` is now a virtual list (`vlist.js`): rows of one fixed height (`.row_height(px)`, required with a builder) with `aria-posinset` and `aria-setsize`, fetched in blocks by offset.
- Fast images for long product lists: `Image::placeholder(Placeholder::color | blur | image)`, `.low(src)` (a low resolution file first, the full one fades in), `.fallback(src)` (tried once), `.on_error(..)`, `.priority()` (`fetchpriority="high"`, never lazy), `.sizes(..)` (`sizes="auto"` for lazy images without it) and `.fit(Fit::Cover | Fit::Contain)`. `ImageCard` takes `placeholder`, `low`, `fallback` and `priority`, `CardGrid` takes `placeholder` and `priority(n)` for the first cards. One delegated listener in `media.js` serves every image; the placeholder needs no script. A blur placeholder is a base64 WebP, AVIF, JPEG or PNG data URI of at most 600 bytes, never SVG. Measured on 2,000 product cards at 4x CPU: no layout shift, 40 of 2,000 photos requested for the first screen, script work during a fast scroll 11 to 18 ms; the page HTML grows 213 bytes per card raw, 4 KB with Brotli for the whole page.
- Long card grids stay fast: cards and list rows off screen skip layout and paint (`content-visibility: auto` with a reserved size from the image ratio), and a card grid with 200 cards or more keeps only the rows near the viewport in the DOM (with `aria-posinset` and `aria-setsize`, focus kept, "Show more" feeding it), holding back the images of cards that only fly by in a fling. Card images no longer fade in. Measured on 2,000 product cards with raster photos (WebP and JPEG), scrolling through all of them in 10 s: at full CPU 142 fps with a p95 frame of 7 ms (57 to 88 fps and 35 to 56 ms before), 511 elements in the document instead of 14,271, 220 photo requests instead of 4,100; at 4x CPU slowdown 4 to 24 fps (5 to 6 before), still short of 30.
- `Video` and `Audio` fetch nothing until they come into view (`preload="none"`, sources and poster set by one observer for the page), and `Video` takes `.poster_low(id)`, `.placeholder(..)`, `.on_error(..)`, `.sources([(id, media)])` and `.preview_when_visible()` for muted previews in lists that never start with reduced motion or data saving.
- Density modes: comfortable (default: 40 px rows, 36 px controls) and compact (32 px), switched by tokens under `<html data-density="compact">`, picked in the account menu, kept in the `zeyn_density` cookie (`POST /_zeyn/density`, in the write limit) and rendered by the server. `Ctx::compact()` tells the screen. `Paging::virtual_scroll()` rows follow the density (40 or 32 px); `Paging::virtual_rows(px)` is unchanged.
- `Shell::auth()` (and `.split()`): a sign in shell without menu, top bar or palette, with the logo and name over one 400 px card and the language and theme pickers at the bottom; its styles (`auth.css`, 3.5 KB) load only there. Every sign in screen of `accounts::ui` uses it; the account and admin screens keep the app shell. `Control::Username` gives a field `autocomplete="username"`; fields with an `email` rule get `autocomplete="email"`; password inputs show the eye toggle inside the field and a "Caps Lock is on" hint.
- Top bar parts for `App::header`: `breadcrumb()` (menu group and current screen, kept up to date by `nav.js`), `search()` (a search field that opens the command palette) and `account()` (an initials button opening a dialog with the signed in user, the theme and language pickers and sign out). With them the palette button and the pickers leave the side menu.
- `Card::action(node)` for one action at the end of the title row and `Card::footer(node)` for a footer under a divider.
- `App::logo(path)` and `App::logo_dark(path)`: the logo mark before the app name in the side menu (a rounded square with the first letter without it), and `Menu::icon(Icon)` for an icon before a menu item. `App::check` reports a logo that is not a path on the app origin.
- Badges: `Badge::dot()` draws an 8 px dot in the tone before plain text and `Badge::count()` a round count pill; `DataGridColumn::status(|v| tone)` shows a choice column as dot status, the quieter form of `.badge(..)`. Badges are one 22 px shape with radius 4 and no border plus inset shadow; neutral badges are muted text on surface 2 with one outline.
- Size limits in new projects: `build.rs` stops the build when a source file has more than `max_file_lines` (default 300) non blank lines, set under `[package.metadata.zeyn]` in `Cargo.toml`, and clippy `too_many_lines` denies functions longer than 60 lines (`too-many-lines-threshold` in `clippy.toml`). AGENTS.md and the skill tell agents to keep one component or screen per file.
- `zeyn g screen <feature> <name>` (short form `g s`, MCP tool `zeyn_new_screen`) adds `screens/<name>.rs`, registers the screen in the feature and adds its title to every locale file. A flat `screens.rs` is first moved to `screens/mod.rs`, and a deleted `screens/` is created again. It never overwrites a file.
- `zeyn check` warns about a `#[screen]` or `#[action]` that no feature registers and about a flat `screens.rs` or `actions.rs` with several items in more than about 150 lines.
- `Kpi::money(title, Money)`: the value and the target with the currency in the user's locale.
- Confirmation for destructive grid and workflow actions: `.confirm(message)` and `.tone(Tone::Danger)` after `DataGrid::row_action` or `bulk_action`, and `Transitions::confirm(name, message)` with `Transitions::tone(name, Tone::Danger)`, in the dialog `ActionButton::confirm` uses.
- `DataQuery::span()` and `Page::window(rows, total, span)` for tables the app computes or indexes itself: the asked rows as an offset and a limit from either page form, and a page with its total and next cursor. `Page::from_slice` uses them.
- `App::header(|h| ...)`: a top bar over the screens of the app shell with the site header's slots (`start`, `center`, `end`, `bottom`), for a search box and a notifications link. Components there load data through their action's permission, like in the site header.
- `ThemePicker::new()`: the theme picker inside a screen (a settings page), with the shell's themes; both pickers stay in step.
- Ten more icons for admin screens: `Icon::Users`, `Settings`, `Bell`, `Shield`, `Clock`, `Tag`, `Mail`, `SignOut`, `ArrowUp` and `ArrowDown`, drawn like the others (24 px grid, 2 px round stroke). They add 2.9 KB to `zeyn.css` (0.4 KB gzip).
- `DataGridColumn::plain()` shows whole numbers without group separators, for codes and years.
- Screen header: `Screen::subtitle(text)` and `Screen::primary(|t| ...)` put a subtitle under the title and the main actions at the end of the title row, wrapping under the title on phones.
- `DiffView` shows the fields that changed in a record, old struck out and new marked as added, side by side in a table or inline (`DiffLayout::Inline`): `DiffView::of(&before, &after)` compares two model values like the audit trail, `DiffView::changes(entry.changes)` takes an audit record and `.model::<M>()` adds field and choice labels. Masked fields stay masked, also in a hand built change list. No script; `diff.css` (1.9 KB, 0.7 KB gzip) loads only on pages that hold one.
- `CodeView` and `JsonView` show read only code and data: highlighted on the server by a hand written highlighter for Rust, JSON, TOML, SQL, JavaScript and shell (`zeyn::ui::highlight`, which the documentation app now uses too), with line numbers, folding of bracketed blocks through `<details>` (no script) and a copy button in `share.js`. At most 10,000 lines and 1 MB, with a notice when cut. `JsonView::of(value, &ctx)` masks masked fields first. A 5,000 line Rust file renders in 30 ms (debug build) into 645 KB of HTML; `code.css` is 2.7 KB (0.8 KB gzip) and loads only with it.
- `Barcode::code128(value)` and `QrCode::new(value)` draw Code 128 barcodes and QR codes as SVG on the server, written by hand after the public standards (no new crate, no script): Code 128 with digit runs in code set C, the check symbol and 10 module quiet zones; QR codes in byte mode at versions 1 to 40, levels `QrLevel::Low` to `High`, Reed Solomon error correction, the lowest penalty mask and 4 module quiet zones. Each symbol is a figure named by the text under it (`.label(..)`), black on a white plate in both themes. Tested against known check symbols, error correction codewords and format and version bits. `barcode.css` (1 KB) loads only with them.
- `Inbox::of(action)` lists the user's notifications newest first, grouped by day in the user's time zone, unread ones marked (`.when(field)`, `.read(field)`, `.item(..)`, at most 200), with bulk `.mark_read(action)` and `.archive(action)` that take an `InboxSelection` through `POST /_zeyn/data` (CSRF, write limit, the action's permission, at most 200 keys; buttons and check boxes are left out for users without the permission). After a bulk action the inbox names its topic (`"notifications"`) and refreshes in place. `NotificationCenter::of(count, screen, params)` is the bell for the end slot of `App::header`: the unread count from one bounded query (`UnreadCount { limit: 100 }`, `99+` above), loaded with one request and again when the topic is named, by `hub.notify` too with `.live()` and the `realtime` feature (`live.js` leaves elements with `data-zeyn-live-self` to refresh themselves). `inbox.js` (6.1 KB, 2.3 KB gzip) and `inbox.css` (3.1 KB) load only on pages that hold one.
- `Kanban::of(flow, cards, move_card)` shows records as cards in a column per state of a workflow (`.state(field)`, `.card(|row| node)`, `.wip(state, n)`, `.confirm(name, message)`). Each column is a virtual list (`vlist.js`) over a `DataQuery` scoped to its state, 50 cards a block. Moving a card runs the workflow transition between the two states with a `TransitionRequest` through `POST /_zeyn/data`: only the user's moves are offered, the server checks the transition's and the action's permission, and `Workflow::apply` refuses a card moved by someone else (409). Drag with a mouse or pen; keyboard and touch use a move menu (APG menu button) and Shift with Left or Right; the board is one tab stop with arrow key navigation, mirrored under RTL. The card moves at once and goes back with the reason when the server refuses; on success both columns render again with one `POST /_zeyn/nodes`. Measured in the component catalog (debug server): the optimistic move paints 1 ms after the key at 1x CPU and 4 to 10 ms at 4x, the saved columns are back after 12 to 19 ms at 1x and 66 to 153 ms at 4x. `kanban.js` (15.5 KB, 5 KB gzip) and `kanban.css` (4 KB) load only with a board.
- `Calendar::of(action)` shows events in a month, week or day view in the user's time zone (`.start(field)`, `.end(field)`, `.label(..)`, `.open(|e| effect)`, `.create(action)`, `.hours(from, to)`, `.at(CalendarParams)`). The action gets only the visible range as two date filters in the user's zone, at most 500 events; the server places and renders everything. View and date are screen params changed in place by the toolbar through the calendar's topic, so a view switch is one `POST /_zeyn/nodes` (21 to 34 ms after the click at 1x CPU, 60 to 163 ms at 4x, debug server) and the address can be shared. The days are an APG grid (one tab stop, arrows, Home, End, Page Up and Down, Enter into the events, mirrored under RTL). Choosing a slot (click, drag over hours, Shift with arrows, then Enter or N) sends local wall times through `POST /_zeyn/data`; the server reads them in the user's zone, refuses skipped hours and slots over 31 days and runs the action with a `CalendarSlot`, whose effect runs. `calendar.js` (9.6 KB, 3.3 KB gzip) and `calendar.css` (4.4 KB) load only with a calendar.

### Changed

- Command palette: results are grouped under headings with an icon per row and the matched letters in bold, `/` opens it outside fields, the dialog is raised with radius 12 and shadow 3, and an empty index is built again before the query is ranked, so typing "ord" right after opening finds the Orders menu item (ZEYN-185).
- Design system v2, states: empty states without the dashed box (an icon in a 40 px circle, 360 px wide, 48 px padding), skeletons that pulse in opacity instead of a shimmer (static with reduced motion), toasts as raised 360 px cards with an icon in their tone, and dialogs on `--z-raised` with radius 12, `--z-shadow-3`, a scale and fade entrance and a bottom sheet on phones.
- Grids: pinned cells carry a class the server writes (`z-pin z-ps1` to `z-ps5`, `z-pe1` to `z-pe3`, with the filter row copying it), replacing the `:nth-child` rules that matched every cell of every grid; the last pinned column has a 1 px divider. On phones (640 px and less) paged and infinite grids read as a list: the first column on its own line at 14 px 500 and the other cells after it in muted text; virtual grids keep their rows.
- Design system v2, forms: forms up to 720 px with a card or panel as wide as its form, labels 13/20 500 in the text color 6 px above the input, 20 px between fields, inputs with a focus border and a soft 3 px ring, a danger border when invalid, read only fields without a border, units and `%` drawn muted inside the input, and the number stepper as minus and plus buttons inside the end of the field.
- Design system v2, charts: bar, line, area and combo charts draw only geometry in an SVG that stretches with the card (`--z-chart-h`, 240 px, 200 px on phones) and put tick labels in HTML beside it, so text stays 12 px at any width; ticks are shortened (`600K`, `1.2M`, locale texts in en and tr) and category labels thin out; bars take 60% of their band; lines are 2 px with round HTML point marks shown on hover, focus and the last value; a single series has no legend; hovering or focusing a category shows a crosshair and a tooltip with every series at it. Eight series colors (`--z-chart-1` to `--z-chart-8`, new values ordered for color blindness, series 7 and 8 dashed), checked at startup for 3:1 against `--z-surface`.
- Design system v2, KPI cards: the value at 30/36 600 with the currency symbol or unit smaller and muted, values of seven digits and more shortened (`₺1.23M`) with the full value as tooltip and for screen readers, the change as a chip (`+48.8%`, neutral under 0.5%) with "vs previous period", a full width 40 px trend in the chip tone over a fading area, 20 px padding and at least 132 px height; Kpi rows keep two per line on phones and drop the trend under 360 px. The showcase dashboard shows its numbers as Kpi cards.
- Design system v2, cards: 20 px padding, a grid or list that is the content of a card runs edge to edge with the card border as the only one, a card inside a card drops its box, and cards in a `Row` stretch to the tallest (`Row` items now stretch instead of starting at the top).
- Design system v2, page header: two or more exports in one toolbar fold into one "Export" menu button (W3C APG menu button) with an item per format (`zeyn.export.menu`, `zeyn.export.format.*`), and the header keeps 24 px before the content. The showcase products screen puts "New product" in the header as its primary action.
- Design system v2, buttons: 36 px controls (`--z-control`, 32 px in compact), the default button with a quiet `--z-border` outline and `--z-shadow-1`, sizes `.z-button-sm` (32 px) and `.z-button-xs` (28 px), `ToolButton::ghost()` and `ToolButton::danger()`, and danger buttons filled in `--z-danger`. A toolbar button shortcut is shown in its tooltip instead of a key chip in the label, so pages with shortcut buttons load `overlays.js` (7.9 KB).
- Icons: a set of 82 icons drawn on a 20 px grid with a 1.5 px round stroke (`Icon::ALL`, `Icon::name()`), served as CSS masks from `/_zeyn/icons.css` (24.9 KB, 2.9 KB gzip, cached; generated from `assets/icons/*.svg` by `tests/icons.rs`). `Icon::Delete` is now `Icon::Trash` and `Icon::Chart` is `Icon::ChartBar`; `zeyn check --fix` rewrites both, and it now renames names written with a path (`Icon::Delete`) only where that path is written. Icons serialize in kebab case (`sign-out`), which fixes icons with two words in menus built in the browser. Sort arrows and the column menu dots use the same drawings.
- Design system v2, shell: the app name no longer sits on a brand band; the side menu (248 px) starts with the logo and name in a 56 px row, items are 13/20 500 with an optional icon, and the current item is the brand tint without an inset bar. In dark the menu sits on `--z-bg`. Layout rules use three size classes: phone up to 640 px, tablet up to 1023 px (the side menu and the site menu fold behind their menu buttons below 1024 px, before 800 px) and desktop.
- Design system v2, grids: rows 40 px (`--z-row`) and headers 36 px (`--z-row-head`), headers at 12/16 500 with no vertical rules, rows divided by lines only (`DataGrid::zebra()` brings the shaded rows back), cells on one line ending in an ellipsis (`DataGridColumn::wrap()` lets one column wrap), links in cells drawn as text at 500 that underline on hover, 28 px pager buttons and radius 8 on the grid.
- Design system v2, type: a type scale (`--z-text-2xs` to `--z-text-num-lg` with `--z-leading-*`), body line height 20 px, page titles 24/32 (20/28 on phones), card, panel, chart and card grid titles 16/24, window titles 20/28, and weight 500 for labels, buttons, grid headers, badges and current items; 600 stays for titles and numbers, 700 is gone.
- Design system v2, colors and shape: cool graphite neutrals in both themes (`--z-bg` `#f5f6f8` and `#0d0f12`, new `--z-raised` for popovers and dialogs), dark tints at 16%, `--z-radius` 6 px with `--z-radius-xl` and `--z-radius-full`, three shadow levels (`--z-shadow-1` to `--z-shadow-3`, a ring instead of a shadow in dark), a spacing scale `--z-s-*`, `--z-gap` 16 px and motion tokens (`--z-dur-*`, `--z-ease-*`) that drop to zero under reduced motion. The startup contrast check now covers 37 pairs per theme, including raised surfaces, the selected tint, the status tints and the primary hover and pressed colors, computed in Oklab the way the browser computes them. `--z-shadow-color` and `--z-shadow-strength` are gone; use the shadow tokens.
- `zeyn g feature` (short form `g f`) creates `models/`, `services/`, `actions/` and `screens/`, each with a `mod.rs`, holding a working starter: a model, an in memory data service, a search action and a list screen with its menu entry and texts in every locale file. It registers the feature and its service in `src/app.rs` after the last `.feature(features::...)` line, or prints the two lines when there is none. A feature that does not need a folder deletes it with its `mod` line; `components/` is added by hand when a part of a screen is shared or a screen file grows.
- The `store.rs` of a feature is now `services/` in the app and library templates; services shared by several features go in `src/services/<name>.rs`.
- Whole numbers are grouped by the user's locale in grid cells and display values (`1,234,567`, `1.234.567` in Turkish); entity ids stay plain. Decimal cells are right aligned like whole numbers and money.
- A screen whose `#[params]` fields are all `Option` opens without params (menu links, plain addresses) with every field `None`, instead of answering 404. The accounts screens use plain `#[params]` for this now.
- Kpis in one `Row` or `AutoGrid` are as tall as the tallest, with or without a trend; a `Row` of four Kpis fills the width and wraps to two, then one per line. Bar, line, area, combo, scatter and funnel charts fill the width of their container instead of stopping at 18rem of height; donuts and gauges keep the 18rem cap and sit centered.

### Fixed

- `zeyn` builds again without the `server` feature (as `zeyn-cli` uses it): `ThemePicker` renders only with it. CI now builds `zeyn-cli` on its own, so this cannot break the crates.io publish unnoticed.
- App shell: menu groups with the same label from several features are merged under one heading, and elastic overscroll no longer drags the sticky side menu down.

### Changed

- Pages load only the browser modules of the components they hold. `components.js` is split into a shared part and one module per component (`tabs.js`, `dates.js`, `choices.js`, `inputs.js`, `lines.js`, `steps.js`, `tree.js`, `overlays.js`, `menubutton.js`, `busy.js`), `pro.js` into `palette.js` and `pro.js`, and the form drafts, masked value reveal and import of `enterprise.js` into `forms.js`, `reveal.js` and `import.js`. Modules are declared with their HTML markers in `server/modules.rs`; full pages link what their HTML holds, and `zeyn.js` loads missing modules (and stylesheets) when `nav.js`, a window, a lazy panel or a grid brings new content. The media, files, charts and catalog stylesheets are linked only where used. A page with only text now loads 132 KB of scripts instead of 269 KB (46 KB instead of 80 KB with gzip), a grid page 223 KB instead of 357 KB, a form page 186 KB instead of 269 KB; see section 1.1 of `docs/runtime-protocol.md`.
- Clicks show their pending state in the same frame: `nav.js` marks the target link current and sets `html[data-zeyn-loading]` (windows and form submits set it too), which shows a thin top bar only after 150 ms; grids fade their rows as soon as a filter key is pressed, filters wait 150 ms instead of 300 ms, and a newer grid request cancels the older one. At 4x CPU throttling the first visible change after a menu click went from 77 ms to 6 ms, after typing in a grid filter from 323 ms to 15 ms, and the filtered rows arrive after 197 ms instead of 355 ms.
- `nav.js` no longer forces a layout in the middle of a swap: it scrolls in the next animation frame and focuses the new heading after that frame paints, once every write (screen, head, menu, `zeyn:content`) is done. At 4x CPU throttling the forced style and layout of a navigation went from 15 to 25 ms to none, and a swap finishes in 71 ms instead of 86 to 99 ms.
- `DataQuery::apply` reads each sort value once per row (text with its lower case form) instead of twice per comparison, and puts only the rows of the asked page in order; the result is the same as the stable sort before, ties included. Lists of 50,000 rows or more run through `tokio::task::block_in_place` on the multi threaded runtime, so other requests on that worker move to another thread. Sorting one million rows by a text column takes 313 ms instead of 4.2 s (2.1 million allocations instead of 78 million), by money 136 ms instead of 489 ms; a deep page (offset 500,000) costs about the same as the first.
- Dynamic responses of a known size (pages, partial screens, grid blocks, JSON) are gzipped in one pass with a deflate compressor kept per worker thread and reset for each response, instead of a new 300 KB compressor per response; streamed bodies (exports) still compress as they stream. A full page with a 200 row grid now allocates 8 KB for its compression instead of 389 KB and takes 431 µs instead of 455 to 494 µs; same level, same bytes on the wire.
- With `Prefetch::Moderate`, pressing a link with a mouse or pen starts its fetch at once, as touch already did, instead of waiting for the 100 ms hover delay or the click. With 60 ms of network latency and a press after 40 ms of hover, the screen is in place 198 ms after the press instead of 214 ms.

## [0.0.8] - 2026-11-05

### Added

- `Db::query`, `query_one` and `query_max` (also on `Tx`): hand written SELECTs with a static SQL string, bound parameters only and a row cap.
- `Screen::redirect(screen, params)`: 303 to the target's canonical path, never cached, typed targets only.
- `.scope(field, value)` on `CardGrid`, `Rail` and `List::of`.
- `zeyn::text::fold` for search keys and queries (Turkish dotted and dotless i, accents, lower case).
- `LinkButton`, `EmptyState::link` and `Hero::link`.
- Site header slots `start`, `center`, `end` and `bottom` that take any component, with a phone layout for the center slot.
- `.shortcut(keys)` on link chips and `PrevNext`, and `App::check_page`.
- Client preference store: `App::client_prefs`, `Embed::remember(KEY)`, `Tabs::remember(KEY)`, `App::client_prefs_table()` and `zeyn.prefs.clear()` in the browser.
- `App::prefetch(Prefetch::Moderate)`, `#[screen(no_prefetch)]` and `ctx.is_prefetch()`.
- `SearchBox::item(|row| ListItem ...)`: suggestions with an image, a subtitle and a link straight to the record.

### Changed

- `ListItem::meta` takes any inline component.
- `MAX_EMBED_CONSENT` is now `MAX_PREF_LIFETIME` (`zeyn check --fix` rewrites it).

### Fixed

- Paged card grids and lists without a known total show "Page 1" and keep Next enabled while a cursor exists.

## [0.0.7] - 2026-11-03

### Changed
- Rail hides its scrollbar by default and scrolls by mouse drag; `Rail::scrollbar(true)` keeps a thin bar.
- Card links to screens with a route use the canonical path instead of the route redirect.
- JSON-LD address properties are written as absolute URLs with the site URL.

### Fixed
- The phone menu, automatic loading of more cards and the sort picker no longer shift the layout (CLS stays at 0).

## [0.0.6] - 2026-11-03

### Added
- Content components: List, ImageCard extras (subtitle, badges, progress, horizontal), Heading, Paragraph, MetaLine, PrevNext, AutoGrid, Avatar, Spoiler, AlphaIndex, WeekSchedule, Rating, RelativeTime, Share, Chips, Hero and Reactions.
- Bottom navigation bar for the site shell on phones and card hover preview.
- Docs and landing page in nine languages; framework texts in Chinese, Russian, Spanish, Korean, Japanese, Hindi and Arabic behind the `lang-*` Cargo features; `App::locale_paths` puts the language in the URL.
- Right to left layout for every framework component.
- Component catalog site (`zeyn-example-components`).
- `DataGridColumn` can opt out of sorting with `no_sort()`.
- `zeyn new` sets up CI, Dependabot, a pull request checklist, `SECURITY.md` and `CHANGELOG.md` for new projects.
- GitHub Actions CI for this repository.

### Changed
- Dynamic responses are gzipped and the CSRF token is masked in every response.
- Framework asset URLs carry the build id and are cached for a year.
- Grids open faster: grid setup reads geometry once, rows are written without a string per cell, wide virtual grids send their first block in the row template, and the virtual block cache is capped.
- Browser modules load only on pages that use them.
- The repository is formatted with `max_width = 100` and `chain_width = 40`.

### Removed
- The old names kept as deprecated aliases (`Grid`, `GridColumn`, `GridMode`, `GridQuery`, `Agents`, `App::agents`, `RESERVED_SHORTCUTS`, `Card::poster`, `PosterCard`). `zeyn check --fix` still rewrites them.

## [0.0.5] - 2026-11-03

### Added
- `Feature::public_media` serves posters, covers and logos to anyone with no cookie and with public cache headers.
- Embed notice text on the click to load button, and `Embed::remember` and `Embed::remember_for_session` to keep a consent for a limited time or for the tab.
- `Card::image`.

### Changed
- Reads of public media sources skip the per IP rate limit.
- Wide grids open about three times faster, and assets support ETag and gzip.

## [0.0.4] - 2026-11-03

### Added
- Path parameter routes and the `Slug` type.
- Custom error pages: `App::not_found` and `App::server_error`.
- `App::csp` allow lists for images, media, frames, connections and fonts.
- `Embed` and `Image` components, `ExternalLink`, and a safe `Markdown` component (feature `markdown`).
- Public screens are sent with no cookie and no token and can be cached; the runtime fetches the CSRF token on its first POST.
- SEO and head support: description, canonical, `og_image`, JSON-LD and robots; sitemap.
- Site shell with a top bar, navigation menu, language and theme actions and footer links.
- `CardGrid`, `Rail` and `SearchBox`.
- Persistent jobs with a `JobStore` trait, leases, heartbeats and retries.
- `App::llms_txt` serves `/llms.txt` as public plain text.
- Docs pages for sign in, operations, assistant, islands, realtime, desktop and database.

## [0.0.3] - 2026-11-02

### Added
- Accounts (feature `accounts`): users, roles, passkeys, sign in, registration, recovery, two factor and a default UI, with memory and SQL stores.
- PostgreSQL data layer (feature `postgres`) and Oracle driver (feature `oracle`).
- Redis store for shared rate limits and saved views (feature `redis`).
- OIDC sign in, bearer JWT validation and MFA.
- Embedded AI assistant (feature `assistant`) and MCP server, off by default.
- Signed licenses with ed25519.
- tracing and OpenTelemetry spans.
- XLSX export and import (feature `xlsx`), PDF documents from Typst templates (feature `pdf`), e-mail notifications (feature `email`).
- WASM islands (`zeyn-island`), canvas surface, realtime rooms with Y-CRDT documents (feature `realtime`) and desktop packaging (`zeyn-desktop`).
- File upload fields with a `FileStore` trait, decimal, percent, masked and password inputs, multi select, drop down and split buttons, tooltip, popover, busy overlay and more chart types.
- Video and Audio players with ranged media serving.
- Command palette, central keyboard shortcuts, dockable panels, menu bar, toolbox, property inspector and status bar.
- Time zones from the bundled IANA database; `zeyn dev` with hot reload.
- The documentation site written with Zeyn (`zeyn-example-docs`), `AGENTS.md` and the `zeyn` skill for AI agents, Docker files for new projects, and installation from crates.io with `cargo install zeyn-cli`.

### Changed
- `server.rs` and the browser runtime were split into modules with no change in behavior.
- Every endpoint goes through the interceptors, and one masking filter covers every output path.

### Fixed
- Double counting of totals in virtual grids after sorting.
- Smaller security fixes: the page shell escapes its CSRF value, CSV import checks row and column limits while reading, and the per tenant license cache is capped.

## [0.0.2] - 2026-11-02

### Changed
- The grid API is named `DataGrid` (`DataGridColumn`, `DataQuery`, `DataMode`); the old names stay one release as deprecated aliases.

### Added
- `zeyn check --fix` rewrites old Zeyn names in `src`, `tests` and `examples`.
- `zeyn new --lib` creates a library of features shared by several apps; GitLab CI and rustfmt templates for new projects.
- Polyrepo guide.

## [0.0.1] - 2026-11-02

### Added
- First working version: the language (entities, actions, screens, forms), the server, the browser runtime, the CLI and the examples.
- DataGrid with server side paging, sorting, typed filters, multi sort, subtotals, grouping, tree data, pinning, resizing, hiding and moving columns, saved views, row selection, bulk actions, inline editing, CSV export and import, and cell navigation with TSV copy and paste.
- Layout components (Row, Stack, Panel, Card, Tabs, Split), form controls, date and time pickers, master and detail rows, tree view and SVG charts.
- Role based access, field masking, audit of reads and writes, typed approval workflows, idle warning and re-authentication.
- Navigation without page reloads.
- MIT and Apache-2.0 licenses.

[Unreleased]: https://github.com/huseyininnc/zeyn/compare/v0.0.7...HEAD
[0.0.7]: https://github.com/huseyininnc/zeyn/compare/v0.0.6...v0.0.7
[0.0.6]: https://github.com/huseyininnc/zeyn/compare/v0.0.5...v0.0.6
[0.0.5]: https://github.com/huseyininnc/zeyn/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/huseyininnc/zeyn/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/huseyininnc/zeyn/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/huseyininnc/zeyn/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/huseyininnc/zeyn/releases/tag/v0.0.1
