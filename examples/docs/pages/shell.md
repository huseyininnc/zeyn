# Shell and commands

The shell is the frame around every screen: the menu, the language, theme and time zone pickers, toasts and the command palette. Apps that work like desktop tools add a menu bar, a toolbox, a property inspector, a status bar and docking panels.

## The menu

The menu comes from the features' `.menu(...)` calls. Items the user may not open are left out of the HTML, and a feature whose guards deny the user shows nothing. The link of the current screen carries `aria-current="page"` and is drawn on the brand's subtle tint.

`.icon(Icon::...)` after `.item(...)` puts an icon before the item's label; once the items of a group have icons, the group's label is drawn without its own. The side menu starts with the app's logo and name: `App::logo("/static/logo.svg")` (and `App::logo_dark(...)` for the dark theme) sets a mark served by the app, otherwise the shell draws a rounded square in `--z-brand` with the first letter of the name. The shell stays neutral; the brand color marks the current item, primary buttons, the focus ring and the logo.

Icons come from one set of 82 drawings (`Icon::ALL`) on a 20 px grid with a 1.5 px round stroke, shown at 16 or 20 px in the current text color. Each is a CSS mask in `/_zeyn/icons.css`, a cached stylesheet of about 25 KB (3 KB compressed) generated from `crates/zeyn/assets/icons/*.svg`, so an icon costs one class in the HTML and nothing in script.

```rust
Feature::new("sales")
    .screen(orders)
    .menu(|m| m.group(t!("sales.menu"), Icon::Cart).item(t!("sales.orders"), orders).icon(Icon::Table))
```

### Nested and collapsible groups

`.sub(label, |m| ...)` adds a nested group to the last group; inside it, `item`, `icon`, `open` and `sub` work as they do on a group. Nested groups come after the items of their group and nest at most three levels below a top level group (`MENU_DEPTH`); a deeper menu stops the app at startup with a message that names the group.

```rust
.menu(|m| {
    m.group(t!("sales.menu"), Icon::Cart)
        .item(t!("sales.orders"), orders)
        .sub(t!("sales.reports"), |m| m.item(t!("sales.by_city"), by_city).item(t!("sales.archive"), archive))
        .accordion()
})
```

- **Collapsible.** Every group heading is a button with `aria-expanded` and `aria-controls` (the W3C APG disclosure navigation pattern). The server renders the groups that hold the current screen open and the others closed, and `nav.js` opens the branch of the new screen after each navigation. Without the script every group stays open, so nothing becomes unreachable.
- **Always open.** `.open()` after a group keeps it open: its heading is plain text without a button.
- **Accordion.** `.accordion()` on the menu of any feature in the app shell makes the whole side menu an accordion: opening a group closes its open siblings.
- **Keyboard.** Up and Down move between the visible headings and links, Home and End go to the first and the last. Right opens a closed group or moves into an open one; Left closes an open group or moves to the heading of the group around it. Enter and Space toggle a group. Left and Right swap in right to left languages.
- **Permissions and merging.** Groups with the same label merge across features at every level, and a group whose items the user may not open is left out, nested groups included. The breadcrumb of `App::header(|h| h.breadcrumb())` shows the whole group path. The site shell lists the links of nested groups in their top level group.

### Menus built per request

`App::menu_source(source)` adds menu entries that are built for each request, for example from a menu table filtered by the user's roles. A source is an async function that takes the `Ctx` (identity, tenant, language) and returns `MenuNode`s: `MenuNode::group(label, children)` at the top level, with `MenuNode::screen(label, "screen_name")` links (`.params(json)` fills path segments and the query, as `screen_url` does), `MenuNode::path(label, "/reports/sales")` links and nested groups inside. `.icon(...)`, `.open()` and `.permission("menu.reports")` work on any node. A label is plain text, escaped like any other text, or a catalog key with `Text::key`.

The groups come after the static menus of the features, and a group with the label of a static group is merged into it. The breadcrumb, the command palette and `nav.js` treat their links like any other menu link.

The table below works on PostgreSQL and Oracle. One flat `SELECT` reads it, and the tree is built in Rust by `parent_id`:

```sql
CREATE TABLE menu_items (
    id          INTEGER PRIMARY KEY,
    parent_id   INTEGER REFERENCES menu_items (id),
    label       VARCHAR(100) NOT NULL,
    screen      VARCHAR(100),
    path        VARCHAR(500),
    icon        VARCHAR(40),
    permission  VARCHAR(100),
    sort_order  INTEGER DEFAULT 0 NOT NULL
);
```

```rust
#[model]
pub struct MenuRow {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub label: String,
    pub screen: Option<String>,
    pub path: Option<String>,
    pub icon: Option<String>,
    pub permission: Option<String>,
    pub sort_order: i64,
}

async fn menu(ctx: Ctx) -> Result<Vec<MenuNode>> {
    let db = ctx.service::<Db>().ok_or_else(|| Error::Internal("no database".into()))?;
    let rows: Vec<MenuRow> = db
        .query("SELECT id, parent_id, label, screen, path, icon, permission, sort_order FROM menu_items ORDER BY sort_order, id", vec![])
        .await?;
    Ok(tree(&rows, None))
}

fn tree(rows: &[MenuRow], parent: Option<i64>) -> Vec<MenuNode> {
    rows.iter()
        .filter(|r| r.parent_id == parent)
        .map(|r| {
            let mut node = match (&r.screen, &r.path) {
                (Some(screen), _) => MenuNode::screen(r.label.clone(), screen.clone()),
                (None, Some(path)) => MenuNode::path(r.label.clone(), path.clone()),
                (None, None) => MenuNode::group(r.label.clone(), tree(rows, Some(r.id))),
            };
            if let Some(icon) = Icon::ALL.iter().find(|i| Some(i.name()) == r.icon.as_deref()) {
                node = node.icon(*icon);
            }
            match &r.permission {
                Some(p) => node.permission(p.clone()),
                None => node,
            }
        })
        .collect()
}

App::new("Admin").menu_source(menu)
```

- **Never trusted.** The server checks every link on every request: links to unknown screens are dropped with a warning in the log, links to screens the user may not open (access, feature guards, the node's own `permission`) are dropped although the source returned them, and so are screen links whose path segments the params do not fill. A path must start with one `/`: no scheme, no `//`, no backslash and no control characters. Groups nest at most `MENU_DEPTH` levels, a source's answer keeps at most `MAX_MENU_NODES` (500) nodes, and labels are cut at `MAX_MENU_LABEL` (100) characters.
- **Cached.** Answers are cached for `MENU_TTL` (60 seconds) per tenant, language and set of permissions, so users with the same roles share one answer and the source runs once per minute for them. The cache holds at most `MAX_MENU_CACHE` (256) answers and drops the oldest. A source whose answer depends on the user, not only on the permissions, needs `App::menu_per_user()`. After an action edits the table, it calls `invalidate()` on the injected `Menus` service.
- **Never fails the page.** When the source returns an error, panics or takes longer than `MENU_TIMEOUT` (2 seconds), the page shows the static menu and the server logs the failure. Failures are not cached.
- **Where it shows.** The menu belongs to the app shell: full page loads of screens and error pages hold it, while the site shell and the sign in pages show the static menus only.

## The site shell

Work screens open in the app shell with its side menu. Public pages, such as a shop or a product catalog, can open in the site shell instead: the app's name as a brand link, a horizontal menu, a menu button on narrow screens, optional actions and a footer. A feature chooses it; screens of other features keep the app shell, so both live in one app.

```rust
Feature::new("shop")
    .screen(shop_home)
    .screen(shop_delivery)
    .screen(shop_returns)
    .menu(|m| {
        m.group(t!("shop.menu.home"), Icon::Home)
            .item(t!("shop.menu.home"), shop_home)
            .group(t!("shop.menu.help"), Icon::Box)
            .item(t!("shop.delivery.title"), shop_delivery)
            .item(t!("shop.returns.title"), shop_returns)
    })
    .shell(Shell::site().header(|h| {
        h.center(SearchBox::of(search_products).results(shop_search))
            .center_on_phone(CenterOnPhone::Icon)
            .end(LinkButton::new(t!("shop.cart"), shop_cart, ()))
            .sign_in()
            .language()
            .theme()
    })
    .footer(|f| {
        f.column(t!("shop.menu.help"), |c| {
            c.link(t!("shop.delivery.title"), shop_delivery).url(t!("shop.footer.source"), "https://example.com")
        })
        .text(t!("shop.footer.note"))
    })
    .bottom_nav([
        BottomNavItem::new(t!("shop.menu.home"), shop_home).icon(Icon::Home),
        BottomNavItem::new(t!("shop.menu.help"), shop_delivery).icon(Icon::Box),
    ]))
```

- **Menu.** The top menu lists the menus of every feature in the site shell. A group of one item is a plain link; a larger group is a button that opens its links (the W3C APG disclosure navigation pattern). Enter, Space or a click toggle a group, Down opens it and moves to its first link, Up, Down, Home and End move between its links, Escape closes it and returns to the button, and Tab out of the group or a click outside closes it. Left and Right move between the top items.
- **Narrow screens.** Below 1024 px the menu folds behind a menu button with `aria-expanded`; Escape closes it. Without the script the menu stays open, so nothing becomes unreachable.
- **Header slots.** The top bar has four slots that take any component, like an app bar with logical names: `start(node)` before the brand, `center(node)` between the menu and the end, `end(node)` at the end of the bar and `bottom(node)` as a full width row under it, such as chips. Each slot holds several nodes in order, and start and end swap sides in right to left languages. Components in slots render with the user's language, permissions and masks; those that load data through their own action, such as a `SearchBox`, work there, checked against the feature's guards and the action's permission. Grids and forms belong in the screen. On phones the start and end slots stay in the bar, and the center slot goes where the app says: `center_on_phone(CenterOnPhone::Row)` (the default) puts it in a row under the bar, `CenterOnPhone::Icon` behind a magnifier button that opens it (W3C APG disclosure) and moves focus into it; Escape closes it and returns to the button. `center_button(icon, label)` changes that button. The phone layout is set by CSS before the script runs, so the header does not move while the page loads.
- **Actions.** `sign_in()` shows a sign in link for visitors and a sign out link for signed in users when the app has a sign in page; `language()` and `theme()` show the same pickers as the app shell. They come after the end slot and fold into the menu on phones.
- **Bottom bar.** `bottom_nav([...])` adds up to five links fixed to the bottom of the screen on phones and tablets (narrower than 1024 px), for the places people go most; more than five do not compile. Each item is a `BottomNavItem` with a label, a screen with a route and an optional icon. The link of the page being shown has `aria-current="page"` (kept up to date by `nav.js`), links to screens the user may not open are left out, the bar keeps clear of the home indicator with `env(safe-area-inset-bottom)`, and the page gets room at its bottom so the bar hides nothing. The top menu stays for the rest. The bar needs no script.
- **Footer.** Columns of links to screens (`link`, left out for users who may not open them) or addresses (`url`: a path, `https:`, `http:` or `mailto:`; `App::check` refuses other schemes and links to screens that are not registered), and a line of text.

Moving between a site shell screen and an app shell screen goes through `nav.js` without a page load: the partial response says which shell it belongs to (`x-zeyn-shell`), and when that differs from the current frame, the runtime swaps the whole frame the way a language change does and loads the other shell's stylesheet and script. Links to `/_zeyn/route/{screen}` (the breadcrumb's) are followed the same way.

## The sign in shell

Sign in pages get a frame of their own: `Shell::auth()` has no menu, no top bar and no command palette. The app's logo (`App::logo`) and name sit over one card 400 px wide, and the language and theme pickers sit at the bottom of the page, so both stay reachable before sign in. `.split()` puts the form on one half and a brand panel in `--z-brand` on the other from 1024 px; narrower screens show the centered card. On phones the card becomes the page and its inputs and buttons are 44 px tall. Its styles (`auth.css`, 3.5 KB) load on these pages only.

Every sign in screen of `zeyn::accounts::ui` (sign in, sign up, forgot and reset password, two step sign in, recovery code, lockout, confirmation and sign out) opens in it, while the account and administration screens keep the app shell. A custom sign in screen gets it from its feature:

```rust
Feature::new("entry").screen(sign_in).shell(Shell::auth())
```

The sign in form fills the user name with `Control::Username` (`autocomplete="username"`), the password with `current-password`, shows the password toggle as an eye inside the field and says "Caps Lock is on" under it while Caps Lock is on (a polite live region).

## Theme picker in a screen

`ThemePicker::new()` puts the theme picker inside a screen, such as the Appearance section of a settings page. It offers the themes of `App::themes` (with System when the app follows the device) and shows the one in use.

```rust
#[screen(route = "/settings/appearance", permission = "settings.read")]
pub fn appearance() -> Screen {
    Screen::new(t!("settings.appearance")).body(Panel::new(t!("settings.theme")).add(ThemePicker::new()))
}
```

It is the same control as the shell's picker: a choice applies at once without a page load, is kept in the `zeyn_theme` cookie (and the view store for signed in users), and the other picker on the page shows it too. When the app offers no choice (`theme_picker(false)` or a single theme), it renders nothing.

## The app header

`App::header(|h| ...)` puts a bar over the screens of the app shell, with the slots of the site header: `start`, `center`, `end` and `bottom`. A search box goes in the center, a notifications link at the end. `sign_in()`, `language()` and `theme()` work there too, though the side menu already has the pickers.

```rust
App::new(t!("app.name"))
    .header(|h| {
        h.center(SearchBox::of(search_all).results(search_results))
            .end(LinkButton::new(t!("app.notifications"), notifications, ()))
    })
```

Three ready parts make the bar an app frame:

- `breadcrumb()` starts the bar with the menu group and the title of the current screen; `nav.js` rewrites it after each navigation.
- `search()` adds a search field that opens the command palette, with its shortcut shown; the palette button leaves the side menu. On phones it is an icon button.
- `account()` ends the bar with the user's initials on a round button. It opens a dialog (W3C APG non modal dialog; Escape and the close button close it) with who is signed in, the theme and language pickers and the sign out link, or a sign in link for visitors. The pickers leave the side menu.

```rust
App::new(t!("app.name")).header(|h| h.breadcrumb().search().account())
```

The bar stays at the top while the screen scrolls, and the screens change under it without a page load. On phones it sits under the menu button and the center slot takes a row of its own. Components in the bar render for the user like a screen's; a search box loads its suggestions through its action, whose permission the server checks. Without `App::header` the app shell has no top bar.

## Commands

A command is declared once in Rust with its label, shortcut and what it does. Registered on a feature, the palette lists it and its shortcut works on every page.

```rust
pub fn new_product() -> Command {
    Command::open("stock.new_product", t!("pro.cmd_new_product"), product_editor, ProductParams { id: None })
        .shortcut("Ctrl+Alt+N")
        .icon(Icon::Plus)
}

pub fn restock() -> Command {
    Command::action("stock.restock_low", t!("pro.cmd_restock"), restock_low, ()).shortcut("Ctrl+Alt+R")
}

Feature::new("pro").command(new_product()).command(restock())
```

| Constructor | Runs |
|---|---|
| `Command::open(id, label, screen, params)` | Opens a screen as a window |
| `Command::navigate(id, label, screen, params)` | Goes to a screen |
| `Command::effect(id, label, effect)` | An effect in the browser, such as a toast |
| `Command::action(id, label, action, input)` | An action on the server; the input never travels to the browser |

`App::check` reports shortcuts that cannot be read, that two commands share, or that the palette keeps for itself.

## The command palette

**Alt+Shift+P** opens the palette by default (`App::palette_shortcut` changes it, also to a browser key such as `Ctrl+Shift+P` on purpose): an ARIA combobox that searches the app's commands, the toolbar and context menu of the current screen and the screens of the menu. Recently used commands come first. `/` opens it too when focus is not in a field, and so does the search field of `App::header(|h| h.search())`. Results are grouped under their place ("Go to · Sales", the screen title, "Commands"), each with its icon, the letters that matched in bold and its shortcut at the end. The menu items are matched on the page itself, so the first keystroke already finds them.

A feature can add a server search to it. As the user types, the palette calls the action and lists its hits under the commands; choosing one opens the page in place. This site searches its own pages that way:

```rust
#[action(public)]
pub async fn search_docs(query: String, docs: Docs) -> Result<Vec<SearchHit>> {
    Ok(docs.search(&query))
}

Feature::new("docs").search(search_docs)
```

The server runs every search the user may run with the feature's guards, the action's permission and the interceptors, cuts the query at 100 characters, drops hits that leave the app and returns at most 20.

## Menu bar, toolbox and status bar

```rust
MenuBar::new(t!("pro.menubar"))
    .menu(t!("pro.menu_file"), |m| m.command(new_product()).separator().command(restock()))
    .menu(t!("pro.menu_view"), |m| m.panels())

Toolbox::new("Tools")
    .tool("select", "Select", Some(Icon::Search))
    .tool("draw", "Draw", Some(Icon::Edit))

StatusBar::new("Status").left("Ready").right("UTF-8")
```

@live shell

- `MenuBar` follows the ARIA menubar pattern and holds the same commands as the palette.
- `Toolbox` is a vertical group of toggle buttons; the chosen tool is sent as a `zeyn:tool` event.
- `PropertyInspector::new(title, form)` edits the selected object with a form split into `.section(...)` groups.
- `PropertyInspector::island(name)` edits what an island selects, with no form (see [Island inspector](#island-inspector)).
- `StatusBar` has a live region, so messages sent to it are read by screen readers.

## Island inspector

```rust
Row::new()
    .col(Island::new("designer"))
    .col(PropertyInspector::island("designer"))

let schema = FormSchema::new()
    .section("Position")
    .field(FieldSpec::length("x", "X", &[LengthUnit::Mm]).min(0))
    .section("Look")
    .field(FieldSpec::color("fill", "Fill", BRAND).required())
    .field(FieldSpec::choice("align", "Align", [("left", "Left"), ("right", "Right")]));
```

An island sends `zeyn:inspect` with `detail: { title, schema, values }`, where `schema` is a serialized `FormSchema`. Field kinds are `text`, `multiline`, `integer`, `decimal`, `length`, `color`, `boolean`, `choice` and `expression`; each takes `required`, `min`, `max`, `max_len`, `default` and `help`.

The inspector draws the fields with the controls of model forms and fills in names, labels and values as text, never as HTML. When a field is left, Enter is pressed or a picker closes, it checks the value and sends `zeyn:property` with `detail: { name, value }` to the island root. An invalid value shows its error and is not sent. A schema with more than 200 fields is ignored. `FieldSpec::check` runs the same checks on the server.

## Docking panels

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

Users drag tabs between areas, resize areas with the pointer or the keyboard, and close panels; `MenuList::panels` in a menu brings them back. The layout is saved per user and screen in the app's view store, or in the browser when the app has none.
