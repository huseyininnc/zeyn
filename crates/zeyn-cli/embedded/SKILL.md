---
name: zeyn
description: Build and change apps with the Zeyn Framework (Rust, server rendered screens, no JavaScript to write). Use when a project depends on the `zeyn` crate, has `src/features/` with `#[entity]`, `#[action]` or `#[screen]` code, or the user asks to add a screen, grid, form, action, API, permission, translation or feature to a Zeyn app. Covers where to read the docs, the rules every Zeyn app follows, the language in short, and how to verify a change.
---

# Building apps with Zeyn

Zeyn is a Rust web framework for data heavy business apps. You write models, actions and screens
in Rust; the server renders the pages, a small runtime in the browser applies changes, and you
never write HTML, CSS or JavaScript. Every action is also an API endpoint and a tool for AI
agents, with the same permission checks.

## 1. Read the docs first

Do not guess an API. Find it in the docs, in this order:

1. **The project's `AGENTS.md`**: commands, structure and a framework index for this app.
2. **The zeyn MCP server** (set up in `.mcp.json` by `zeyn new`): `zeyn_docs` with topic `rules`,
   `language`, `protocol` or `architecture`; `zeyn_describe` lists the app's features, screens,
   routes, access and problems; `zeyn_check` compiles and reports; `zeyn_test` runs the tests.
3. **The documentation site**: run `cargo run -p zeyn-example-docs` in the Zeyn repository and
   open http://127.0.0.1:3008 (Alt+Shift+P searches). The same pages are Markdown files in
   `examples/docs/pages/` and `docs/` of the Zeyn repository.

Which page answers what:

| Task | Page |
|---|---|
| Start, project layout, CLI | `start/cli`, `start/structure` (`cli.md`, `structure.md`) |
| Entities, models, rules, choices | `language/entities` (`entities.md`) |
| Actions, errors, APIs | `language/actions` (`actions.md`), `guides/api` (`api.md`) |
| Screens, toolbar, windows, params | `language/screens` (`screens.md`) |
| What happens after a click: toast, navigate, open window | `language/effects` (`effects.md`) |
| Forms, validation, conflicts, file uploads | `language/forms` (`forms.md`) |
| DataGrid (columns, filters, totals, paging, export, editing) | `language/datagrid`, `language/datagrid-reference` (`datagrid.md`, `docs/grid.md`) |
| Texts and translations | `language/texts` (`texts.md`) |
| Layouts, controls, pickers, lines editor, display, tree, charts | `components/*` (`layouts.md`, `controls.md`, `lines.md`, `display.md`, `data.md`) |
| Command palette, menu bar, dock, status bar, site shell for public pages | `components/shell` (`shell.md`) |
| Headings, paragraphs, meta lines, previous and next links, auto grids, lists, avatars, spoilers, letter bars, week schedules | `components/content` (`content.md`) |
| Card grids, rails, search boxes, free text search (`DataQuery::q`) | `components/catalog` (`catalog.md`) |
| Video and audio | `components/media` (`docs/media.md`) |
| Dates, time zones, money, languages | `guides/time-and-languages` (`time-and-languages.md`) |
| Permissions, guards, CSRF, masking, audit | `guides/security` (`security.md`), `docs/enterprise.md` |
| Public pages: no cookies, HTTP caching, SEO head tags, sitemap, robots.txt | `guides/public-pages` (`public-pages.md`) |
| Sign in with an identity provider, JWT, MFA | `docs/auth.md` |
| Own user accounts: registration, passwords, two factor, passkeys | `guides/accounts` (`accounts.md`, `docs/accounts.md`) |
| Database | `guides/database` (`docs/database.md`) |
| Themes | `guides/theming` (`docs/theming.md`) |
| AI assistant, MCP, AI attributes | `guides/mcp` (`mcp.md`), `docs/assistant.md` |
| Shared libraries in other repositories | `guides/polyrepo` (`docs/polyrepo.md`) |
| Operations: Docker, tracing, Redis, licenses | `docs/operations.md` |
| Desktop exe, WASM islands, realtime | `docs/desktop.md`, `docs/islands.md`, `docs/realtime.md` |

## 2. The rules

These hold in every Zeyn app. `zeyn check` and the compiler enforce many of them.

1. **One folder per feature.** `src/features/<name>/` holds `mod.rs` (module declarations and the
   `Feature`) and the folders `models/`, `services/`, `actions/` and `screens/`, one file per
   model, service, action and screen. `zeyn g feature <name>` creates them with a working starter
   (a model, an in memory data service, a search action and a list screen); a feature that does
   not need a folder deletes it (the folder and its `mod` line in the feature's `mod.rs`). Add
   `components/` by hand, one function returning a `Node` per file (`components/<name>.rs`), when
   a part of a screen is shared or a screen file grows. `zeyn g screen <feature> <name>` adds
   `screens/<name>.rs`, registers it and adds its title to every locale file. Register screens in the feature's `mod.rs` and the
   feature and its services in `src/app.rs`. A service that several features use goes in
   `src/services/<name>.rs`, registered once in `src/app.rs`. The layout is a default: a single
   `screens.rs` still works, and `zeyn g screen` moves it to `screens/mod.rs` before it adds a file.
2. **Every action and screen declares access**: `permission = "feature.resource.verb"` or
   `public`. The server checks it on every call, from the page, the API and AI agents alike.
3. **Inputs are validated by type.** Saving takes `Valid<Model>`. Put rules on fields
   (`#[rule(required, max_len = 50, email, min = 0, unique)]`); rules across fields go in
   `#[model(validate = f)]`. Do not validate by hand what a rule can express.
4. **Every text a user sees comes from `locales/*.json`** through `t!("key")`. All locale files
   have the same keys; the compiler checks them.
5. **Time and money have their own types.** `Date` for calendar days, `Time` for wall clock
   times, `DateTime` for exact moments (UTC), `LocalDateTime` for a wall clock time at a place.
   Use `ctx.now()` and `ctx.today()`, never the system clock. Money is `Money` (integer minor
   units), never `f64`.
6. **Lists are paged.** List actions take a `DataQuery` and return `Page<Model>`. Never return
   an unbounded list.
7. **No HTML, CSS or JavaScript.** Use the components. If one is missing, write a `Component`
   in Rust and escape user data with `zeyn::render::escape`.
8. **Nothing reloads the page.** Move between screens with effects (`ui.navigate`,
   `Effect::navigate`, `Window::open`) and links; the runtime swaps the content.
9. **Performance and memory come first.** Stream large results, keep services small and shared
   (`Arc`), cap anything that grows, and turn on only the Cargo features the app uses.
10. **Secrets stay in configuration** (`config.secret("key")`), never in code, logs or tests.
11. **AI access is explicit.** Doc comments become tool descriptions for models. Add
    `ai = "guidance"` to explain an action to models, `confirm` when a model may only run it
    after the user confirms, and `ai = false` to hide it from models entirely.
12. **Code reads top to bottom.** Once a chain of calls is longer than about 40 characters, put
    one call on each line, so a line holds one or two dots on average. Builders, screens and
    iterator chains read as a list, which is easier to review and to diff:

    ```rust
    Screen::new(t!("items.title"))
        .toolbar(Toolbar::new().add(new_item))
        .body(
            DataGrid::of(search_items)
                .paging(Paging::pages(25))
                .column(Item::code)
                .column(Item::qty),
        )
    ```

    `cargo fmt` keeps this shape with `chain_width = 40` in `rustfmt.toml`; do not raise it.
13. **Small files, small functions.** One component, screen or group of related actions per file.
    A source file holds at most 300 non blank lines (`build.rs` stops the build otherwise) and a
    function at most 60 lines (clippy `too_many_lines`). When `screens.rs` or `actions.rs` grows,
    turn it into a folder (`screens/mod.rs` plus one file per screen) and move large parts of a
    screen (a toolbar, a form, a panel) into their own functions in their own files. The limits
    live in `Cargo.toml` (`[package.metadata.zeyn] max_file_lines`) and `clippy.toml`
    (`too-many-lines-threshold`); raise them only when the owner agrees.
14. **Every change is tested and checked** before you finish (section 5).

## 3. The language in short

```rust
use zeyn::prelude::*;

/// A customer order.
#[entity(table = "orders")]
pub struct Order {
    #[key]
    pub id: OrderId,
    #[rule(required, max_len = 20, unique)]
    pub code: String,
    pub status: OrderStatus,
    #[rule(min = 0)]
    pub total: Money,
    pub due: Option<Date>,
    #[version]
    pub version: i64,
}

/// Where an order is in its life.
#[choice]
pub enum OrderStatus { Open, Shipped, Cancelled }

/// Which order the editor opens; `None` creates one.
#[params]
pub struct OrderParams { pub id: Option<OrderId> }

/// Orders for the list and the API.
#[action(permission = "sales.order.read")]
pub async fn search_orders(q: DataQuery, store: OrderStore) -> Result<Page<Order>> {
    Ok(q.apply(store.all()?))
}

/// Creates or updates an order.
#[action(permission = "sales.order.write", audit, ai = "Use for one order at a time.")]
pub async fn save_order(order: Valid<Order>, store: OrderStore) -> Result<Order> {
    store.save(order.into_inner())
}

#[screen(route = "/", permission = "sales.order.read")]
pub fn order_list() -> Screen {
    Screen::new(t!("orders.title"))
        .toolbar(|t| t.button(t!("orders.new")).icon(Icon::Plus).shortcut("Alt+N")
            .opens(order_editor, OrderParams { id: None }))
        .body(
            DataGrid::of(search_orders)
                .mode(DataMode::Server)
                .column(Order::code).filter()
                .column(Order::status).filter()
                .column(Order::total).sum()
                .on_open(|o| Window::open(order_editor, OrderParams { id: Some(o.id) })),
        )
}

#[screen(permission = "sales.order.write")]
pub fn order_editor(params: OrderParams, store: OrderStore) -> Result<Screen> {
    let order = match params.id { Some(id) => store.get(id)?, None => Order::default() };
    Ok(Screen::window(t!("orders.order")).body(
        Form::of(order)
            .row(|r| r.field(Order::code).field(Order::status))
            .field(Order::total)
            .field(Order::due)
            .submit(t!("common.save"), save_order)
            .on_success(|_, ui| { ui.toast(t!("orders.saved")); ui.close_and_refresh(); }),
    ))
}

pub fn feature() -> Feature {
    Feature::new("sales")
        .route_prefix("/sales")
        .guard(SignedIn)
        .screen(order_list)
        .screen(order_editor)
        .api(search_orders)
        .api(save_order)
        .menu(|m| m.group(t!("sales.menu"), Icon::Cart).item(t!("orders.title"), order_list))
}
```

What to remember:
- The first argument of an action is its input; of a screen, its params (`#[params]` struct or
  `_: ()`). Every other argument is injected: `Ctx` or a registered `Service`.
- A screen with a `route` is a page; without one it opens as a window. A `#[params]` struct whose
  fields are all `Option` also opens from a menu link or a plain address (every field `None`).
- Page header: `.subtitle(t!("..."))` under the title and `.primary(|t| t.button(..))` for the
  main actions on the right; `.toolbar(..)` stays for the other tools.
- Path params: `#[screen(route = "/orders/{id}", ...)]` binds `{id}` to the params field `id`
  (integers, entity keys, `String`, `Slug`). `Slug::from_title("Çay Bardağı")` is `cay-bardagi`.
  Return `Err(Error::NotFound)` for a missing record, `Screen::moved_to(screen, params)` for a 301, `Screen::redirect(screen, params)` for a 303 that is never cached.
- Errors: `Error::NotFound`, `Error::Forbidden`, `Error::Conflict` (stale row version),
  `Error::Denied(t!("..."))`, `Error::Validation(errors)`, `Error::Internal("for the log")`.
- Quick screens: `DataGrid::auto(action)` and `Form::auto(model)`.
- Paging: `Paging::pages(25)`, `Paging::infinite(100)`, `Paging::virtual_scroll()` for very large
  lists. Grid extras: `.sort_by(field, desc)`, `.exportable()`, `.row_action(...)`,
  `.bulk_action(...)`, `.editable()` with `.edit_with(action)`, `.detail(screen)`, `.group_by(field)`,
  `.plain()` on a whole number column that holds codes or years (others are grouped by locale).
- Buttons that run an action: `ActionButton::new(t!("..."), action, input)`, with `.confirm(...)`.
  Destructive grid actions: `.row_action(..)` or `.bulk_action(..)` then `.confirm(t!("..."))`
  `.tone(Tone::Danger)`; workflow moves: `Transitions::of(..).confirm("cancel", t!("..."))`.
- Layouts: `Row`, `Stack`, `Panel`, `Card`, `Tabs`, `Split`.
- Settings pages: `ThemePicker::new()` is the theme choice inside a screen, in step with the shell.
- Icons (`Icon::`): Plus, Edit, Delete, Save, Search, Box, Cart, Table, Home, Chart, Users,
  Settings, Bell, Shield, Clock, Tag, Mail, SignOut, ArrowUp, ArrowDown.
- Key figures: `Kpi::new(t!("..."), value)` or `Kpi::money(t!("..."), money)` with `.delta(..)`
  and `.trend(..)`; four in a `Row` fill a desktop row and wrap two, then one per line.
- The app (`src/app.rs`): `App::new(t!("app.name")).i18n(i18n).service(store).feature(sales::feature())`,
  plus what the app needs: `.auth(...)`, `.mcp(Mcp::read_only())`, `.themes([...])`,
  `.datagrid_views(store)`, `.assistant(...)`.

## 4. Common mistakes

| Mistake | Instead |
|---|---|
| `Grid`, `GridQuery`, `GridMode` | `DataGrid`, `DataQuery`, `DataMode` (`zeyn check --fix` rewrites old names) |
| A literal string in the UI | `t!("key")` with the key in every `locales/*.json` |
| `f64` for prices | `Money` |
| `DateTime` for a birthday or a due day | `Date` |
| `std::time::SystemTime::now()` | `ctx.now()` |
| Returning `Vec<T>` from a list action | `Result<Page<T>>` from a `DataQuery` |
| Checking permissions inside the action body | `permission = "..."` on the attribute, guards on the feature |
| Saving a plain model | `Valid<Model>`; it is validated before your code runs |
| Writing JavaScript for a behavior | Effects, components, or a `Component` in Rust |
| `location.reload()` style refreshes | `ui.close_and_refresh()`, `ui.navigate(...)` |
| A secret in `config/app.json` or code | `config.secret("key")` from the environment or a vault |
| Forgetting to register | Screens in the feature's `mod.rs`, features and services in `src/app.rs` |

## 5. Verify before you finish

1. `cargo test`: the app's tests drive it through its router; add a test for new behavior.
2. `zeyn check` (or the `zeyn_check` MCP tool): compiles, then reports structural problems and
   missing translations. It must say `ok`. Its warnings name a `#[screen]` or `#[action]` that no
   feature registers and a flat `screens.rs` or `actions.rs` that has grown too large to stay flat.
3. `zeyn dev` and open the screen in a browser: the page follows each rebuild without reloading.
   Check that the screen works by keyboard, in both themes, and that the console has no errors.
4. For a container: `docker build -t <app> .` and `docker run -p 3000:3000 <app>`.

If a check fails, fix the cause. Do not loosen `clippy.toml`, `[lints]`, `deny.toml` or the
permission on an action to make it pass.
