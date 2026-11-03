# Shell and commands

The shell is the frame around every screen: the menu, the language, theme and time zone pickers, toasts and the command palette. Apps that work like desktop tools add a menu bar, a toolbox, a property inspector, a status bar and docking panels.

## The menu

The menu comes from the features' `.menu(...)` calls. Items the user may not open are left out of the HTML, and a feature whose guards deny the user shows nothing. The link of the current screen carries `aria-current="page"`.

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
    .shell(Shell::site().header(|h| h.sign_in().language().theme()).footer(|f| {
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
- **Narrow screens.** Below 800 px the menu folds behind a menu button with `aria-expanded`; Escape closes it. Without the script the menu stays open, so nothing becomes unreachable.
- **Actions.** `sign_in()` shows a sign in link for visitors and a sign out link for signed in users when the app has a sign in page; `language()` and `theme()` show the same pickers as the app shell.
- **Bottom bar.** `bottom_nav([...])` adds up to five links fixed to the bottom of the screen on phones (800 px wide and less), for the places people go most; more than five do not compile. Each item is a `BottomNavItem` with a label, a screen with a route and an optional icon. The link of the page being shown has `aria-current="page"` (kept up to date by `nav.js`), links to screens the user may not open are left out, the bar keeps clear of the home indicator with `env(safe-area-inset-bottom)`, and the page gets room at its bottom so the bar hides nothing. The top menu stays for the rest. The bar needs no script.
- **Footer.** Columns of links to screens (`link`, left out for users who may not open them) or addresses (`url`: a path, `https:`, `http:` or `mailto:`; `App::check` refuses other schemes and links to screens that are not registered), and a line of text.

Moving between a site shell screen and an app shell screen goes through `nav.js` without a page load: the partial response says which shell it belongs to (`x-zeyn-shell`), and when that differs from the current frame, the runtime swaps the whole frame the way a language change does and loads the other shell's stylesheet and script. Links to `/_zeyn/route/{screen}` (the breadcrumb's) are followed the same way.

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

**Alt+Shift+P** opens the palette by default (`App::palette_shortcut` changes it, also to a browser key such as `Ctrl+Shift+P` on purpose): an ARIA combobox that searches the app's commands, the toolbar and context menu of the current screen and the screens of the menu. Recently used commands come first.

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
- `StatusBar` has a live region, so messages sent to it are read by screen readers.

## Docking panels

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

Users drag tabs between areas, resize areas with the pointer or the keyboard, and close panels; `MenuList::panels` in a menu brings them back. The layout is saved per user and screen in the app's view store, or in the browser when the app has none.
