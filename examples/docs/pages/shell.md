# Shell and commands

The shell is the frame around every screen: the menu, the language, theme and time zone pickers, toasts and the command palette. Apps that work like desktop tools add a menu bar, a toolbox, a property inspector, a status bar and docking panels.

## The menu

The menu comes from the features' `.menu(...)` calls. Items the user may not open are left out of the HTML, and a feature whose guards deny the user shows nothing. The link of the current screen carries `aria-current="page"`.

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

**Ctrl+Shift+P** or **F1** opens the palette: an ARIA combobox that searches the app's commands, the toolbar and context menu of the current screen and the screens of the menu. Recently used commands come first.

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
