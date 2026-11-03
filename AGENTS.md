# AGENTS.md

Instructions for AI coding agents working on the Zeyn Framework repository. Codex, Claude Code,
Cursor, Copilot, Gemini and other agents read this file. People are welcome too.

Zeyn is an open source Rust web framework for data heavy business apps: grids, forms and
workflows. AI agents write the application code; people read, review and change it. Screens are
rendered on the server, a small runtime in the browser applies changes, and app authors write no
HTML, CSS or JavaScript.

If you are writing an app *with* Zeyn rather than changing Zeyn itself, read
[`skills/zeyn/SKILL.md`](skills/zeyn/SKILL.md) instead. `zeyn new` copies it into every project. The CLI ships a copy in `crates/zeyn-cli/embedded/`; a test fails until you copy your change there.

## Read before you change anything

Find the document for your area and read it first. The docs are the contract; the code follows
them. When code and docs disagree, fix the one that is wrong in the same change.

| You are working on | Read |
|---|---|
| Anything, the first time | `README.md`, then `docs/vision-and-architecture.md` |
| The language: entities, actions, screens, forms, texts | `examples/docs/pages/` (`entities.md`, `actions.md`, `screens.md`, `forms.md`, `effects.md`, `texts.md`) |
| DataGrid | `docs/grid.md`, `examples/docs/pages/datagrid.md` |
| Components and layouts | `examples/docs/pages/` (`layouts.md`, `controls.md`, `lines.md`, `display.md`, `data.md`, `catalog.md`, `shell.md`) |
| Browser runtime, endpoints, events | `docs/runtime-protocol.md` |
| Security, audit, masking, sessions, jobs, workflows | `examples/docs/pages/security.md`, `docs/enterprise.md` |
| Sign in, JWT, OIDC, MFA | `docs/auth.md` |
| User accounts, passwords, passkeys (feature `accounts`) | `docs/accounts.md`, `examples/docs/pages/accounts.md` |
| Database (PostgreSQL, Oracle) | `docs/database.md` |
| Time zones, licenses, Redis, tracing, Docker | `docs/operations.md` |
| Themes and design tokens | `docs/theming.md` |
| AI assistant, MCP, AI attributes | `docs/assistant.md`, `examples/docs/pages/mcp.md` |
| WASM islands and canvas | `docs/islands.md` |
| WebSocket rooms and CRDT | `docs/realtime.md` |
| Media players and ranged serving | `docs/media.md` |
| Desktop packaging | `docs/desktop.md` |
| CLI, `zeyn new`, templates | `examples/docs/pages/cli.md`, `crates/zeyn-cli/templates/` |
| Apps and shared libraries in separate repos | `docs/polyrepo.md` |

`docs/language-example.md` and `docs/framework-analysis.md` are early design notes; some APIs in
them were never built. Trust `examples/docs/pages/` and the code.

The same docs run as a Zeyn app: `cargo run -p zeyn-example-docs`, then open
http://127.0.0.1:3008. Alt+Shift+P searches every page.

## Repository map

```
crates/zeyn/           the framework (feature gated modules, see Cargo.toml [features])
  src/model.rs         Model, Entity, Field<M, T>, Valid<T>, rules
  src/value.rs         FieldType, FieldKind, Value, Money
  src/action.rs        Action, Error, Page, DataQuery, Cursor, page size limits
  src/screen.rs        ScreenDef, Screen, Component, Node, toolbar, Effect, UiCtx
  src/grid.rs          DataGrid
  src/form.rs          Form, SubmitForm
  src/ui/              layouts, controls, pickers, lines editor, display widgets, tree, charts, shell
  src/server.rs        App, the router, the page shell; server/*.rs holds the endpoints
  src/sql/             data layer (features postgres, oracle)
  src/assistant/       embedded AI assistant (feature assistant)
  assets/              the browser runtime: zeyn/*.js (joined into /_zeyn/zeyn.js), nav.js,
                       components.js and their CSS
  locales/             the framework's own texts (en.json, tr.json)
  tests/               integration tests, one file per area
crates/zeyn-macros/    entity, model, choice, params, action, screen, t!
crates/zeyn-cli/       the zeyn command and the project templates
crates/zeyn-island/    browser side of WASM islands
crates/zeyn-desktop/   desktop window around an app
examples/              showcase (every component), docs, adisyon (POS), inventory, big-grid,
                       billing + crm + shared-domain (shared libraries)
docs/                  design documents and guides
```

## Commands

```
cargo build --workspace
cargo test --workspace
cargo test -p zeyn --features assistant,oidc,totp,islands,realtime,xlsx,email,accounts   tests behind features
cargo check -p zeyn --all-features --tests                                      everything compiles
cargo run -p zeyn-example-showcase      http://127.0.0.1:3002, every component
cargo run -p zeyn-example-docs          http://127.0.0.1:3008, the documentation
```

- Database, Redis and identity provider tests skip unless their variable is set
  (`ZEYN_TEST_PG`, `ZEYN_TEST_ORACLE`, `ZEYN_TEST_REDIS`, `ZEYN_TEST_OIDC`); each test file says
  how to start the server in Docker.
- Disk space is limited on shared machines. Use `CARGO_INCREMENTAL=0`, and delete your own target
  directory when you are done. The `pdf` feature links Typst (about 40 MB); do not turn it on for
  the whole workspace.
- `cargo test` does not rebuild a binary you then run by hand; run `cargo build -p <example>` first.

## The rules

These are not style preferences. A change that breaks one is not done.

### Performance first
Performance and a small memory footprint are the most important rule.
- Stream instead of buffering whole results; use bounded channels and fixed size buffers.
- Borrow instead of clone; no allocation in hot paths you can avoid.
- No per user UI state on the server. Requests are stateless; state lives in the database, a
  signed cookie or the browser.
- Every cache, queue and map that grows with traffic has a cap.
- Heavy integrations go behind a Cargo feature, so apps that do not use them pay nothing.
- When you add something that costs memory, binary size or time, measure it before and after and
  say the numbers in the commit or the report.

### No component reloads the page
Navigation, effects, language and theme changes go through `nav.js`: `zeyn:navigate`,
`zeyn:refresh`, `zeyn:refresh-page`. A full page load is only the fallback when that script is
missing. Never call `location.reload()` or `location.assign()` for in-app navigation.

### Security
- Every action and screen declares access: a permission or `public`. Check access on the server
  for every request, also for things the page does not show.
- Every `POST` endpoint calls `check_csrf`. Endpoints that write or cost money go in the
  `is_write` list for the write rate limit.
- Endpoints that address a node of a screen take `crate::ui::NodePath` (string paths such as
  `"1.0.2"`), never `usize`, and look it up with `screen.node(&path)`. Test with a nested path.
- User data in HTML always goes through `render::escape` (or `Render::text` / `Render::value`).
  Never trust limits or ids from the client (`DataQuery::limit`, keys, file names).
- Render with the user's context (`.with_identity(...)`, `.with_ctx(...)`) so masked fields stay
  masked and reads are audited. Masked values never reach HTML, exports, logs or AI models.
- CSP stays strict: no inline scripts or event handlers. New browser code is a separate asset.
- Secrets never go in code, logs, tests that look real, or commit messages.

### Accessibility and UI
- WCAG 2.2 AA is the baseline: keyboard use, visible focus, labels, W3C APG patterns for
  widgets, live regions for changing text.
- Both the light and the dark theme must work. Colors come from theme tokens (`docs/theming.md`).
- Every user facing text comes from the locale files, in English and Turkish.

### Code
- Everything in English: code, doc comments, docs, test names, UI strings (Turkish goes only in
  `tr.json`).
- No inline `//` comments. Use `///` and `//!` doc comments on items.
- Write code top to bottom, not across. Once a chain of calls is longer than about 40 characters,
  put one call on each line, so a line holds one or two dots on average. Builders, screens and
  iterator chains read as a list. `cargo fmt` does this with `chain_width = 40` in
  `rustfmt.toml`; do not raise it.
- Generated code from macros always uses `::zeyn::` paths.
- Every rule the compiler can enforce gets a `compile_fail` doctest and an error message written
  for an AI agent to act on (`#[diagnostic::on_unimplemented]` where it fits).
- Keep the existing structure and patterns; do not add a second way to do something that already
  has one.
- Public prose (README, docs, UI text) uses no em dashes or en dashes, and stays neutral: it
  describes the framework, not who might buy it.

### Tests
- New logic gets a test in the matching file under `crates/zeyn/tests/`. Security fixes get a
  regression test that fails without the fix.
- Browser behavior is checked in a real browser (the showcase or the docs app) with no console
  errors, before the work is called done.
- Run `cargo test --workspace` before you commit; it must pass.

### Commits
- One commit per finished piece of work. The message says what was done, in plain words, starting
  with the ticket id when there is one: `ZEYN-42 PDF list reports for grids`.
- No `feat:`, `fix:` or other type prefixes. No AI attribution lines.

## When something is missing

Search the docs and the code first; the feature may exist under another name (`DataGrid`, not
`Grid`; `DataQuery`, not `GridQuery`). If it does not exist, follow the pattern of the closest
existing feature and add docs and tests with it.
