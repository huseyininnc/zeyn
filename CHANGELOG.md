# Changelog

All notable changes to Zeyn are recorded here. The format follows [Keep a Changelog 1.1](https://keepachangelog.com/en/1.1.0/). The project is in alpha, so APIs can change between versions.

## [Unreleased]

### Security

- `Feature::files` takes a `FileAccess`: `FileAccess::read(p).write(q)` lets readers download files they may not upload. A plain permission name still covers both. Before, a store readable by a role was also writable by it.
- Form submits no longer write the values of password and one time code fields (`Control::Password`, `NewPassword`, `PasswordMeter`, `OneTimeCode`) into the audit trail; the change is recorded with both values left out. This covered the sign in, register, reset and change password forms of the `accounts` feature.

### Added

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

### Changed

- `zeyn g feature` (short form `g f`) creates `models/`, `services/`, `actions/` and `screens/`, each with a `mod.rs`, holding a working starter: a model, an in memory data service, a search action and a list screen with its menu entry and texts in every locale file. It registers the feature and its service in `src/app.rs` after the last `.feature(features::...)` line, or prints the two lines when there is none. A feature that does not need a folder deletes it with its `mod` line; `components/` is added by hand when a part of a screen is shared or a screen file grows.
- The `store.rs` of a feature is now `services/` in the app and library templates; services shared by several features go in `src/services/<name>.rs`.
- Whole numbers are grouped by the user's locale in grid cells and display values (`1,234,567`, `1.234.567` in Turkish); entity ids stay plain. Decimal cells are right aligned like whole numbers and money.
- A screen whose `#[params]` fields are all `Option` opens without params (menu links, plain addresses) with every field `None`, instead of answering 404. The accounts screens use plain `#[params]` for this now.
- Kpis in one `Row` or `AutoGrid` are as tall as the tallest, with or without a trend; a `Row` of four Kpis fills the width and wraps to two, then one per line. Bar, line, area, combo, scatter and funnel charts fill the width of their container instead of stopping at 18rem of height; donuts and gauges keep the 18rem cap and sit centered.

### Fixed

- App shell: menu groups with the same label from several features are merged under one heading, and elastic overscroll no longer drags the sticky side menu down.

### Changed

- Pages load only the browser modules of the components they hold. `components.js` is split into a shared part and one module per component (`tabs.js`, `dates.js`, `choices.js`, `inputs.js`, `lines.js`, `steps.js`, `tree.js`, `overlays.js`, `menubutton.js`, `busy.js`), `pro.js` into `palette.js` and `pro.js`, and the form drafts, masked value reveal and import of `enterprise.js` into `forms.js`, `reveal.js` and `import.js`. Modules are declared with their HTML markers in `server/modules.rs`; full pages link what their HTML holds, and `zeyn.js` loads missing modules (and stylesheets) when `nav.js`, a window, a lazy panel or a grid brings new content. The media, files, charts and catalog stylesheets are linked only where used. A page with only text now loads 132 KB of scripts instead of 269 KB (46 KB instead of 80 KB with gzip), a grid page 223 KB instead of 357 KB, a form page 186 KB instead of 269 KB; see section 1.1 of `docs/runtime-protocol.md`.
- Clicks show their pending state in the same frame: `nav.js` marks the target link current and sets `html[data-zeyn-loading]` (windows and form submits set it too), which shows a thin top bar only after 150 ms; grids fade their rows as soon as a filter key is pressed, filters wait 150 ms instead of 300 ms, and a newer grid request cancels the older one. At 4x CPU throttling the first visible change after a menu click went from 77 ms to 6 ms, after typing in a grid filter from 323 ms to 15 ms, and the filtered rows arrive after 197 ms instead of 355 ms.
- `nav.js` no longer forces a layout in the middle of a swap: it scrolls in the next animation frame and focuses the new heading after that frame paints, once every write (screen, head, menu, `zeyn:content`) is done. At 4x CPU throttling the forced style and layout of a navigation went from 15 to 25 ms to none, and a swap finishes in 71 ms instead of 86 to 99 ms.
- `DataQuery::apply` reads each sort value once per row (text with its lower case form) instead of twice per comparison, and puts only the rows of the asked page in order; the result is the same as the stable sort before, ties included. Lists of 50,000 rows or more run through `tokio::task::block_in_place` on the multi threaded runtime, so other requests on that worker move to another thread. Sorting one million rows by a text column takes 313 ms instead of 4.2 s (2.1 million allocations instead of 78 million), by money 136 ms instead of 489 ms; a deep page (offset 500,000) costs about the same as the first.
- Dynamic responses of a known size (pages, partial screens, grid blocks, JSON) are gzipped in one pass with a deflate compressor kept per worker thread and reset for each response, instead of a new 300 KB compressor per response; streamed bodies (exports) still compress as they stream. A full page with a 200 row grid now allocates 8 KB for its compression instead of 389 KB and takes 431 µs instead of 455 to 494 µs; same level, same bytes on the wire.

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
