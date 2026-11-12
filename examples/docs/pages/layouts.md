# Layouts

Layout components place other components. They hold no data and add no requests; a screen built from them is one HTML response.

```rust
Stack::new()
    .add(Row::new().col(Card::new().title("Orders")).col(Card::new().title("Stock")).col(Card::new().title("People")))
    .add(
        Panel::new("Products").collapsible().add(
            Tabs::new("Views")
                .tab("Card", Card::new().title("A card inside a tab"))
                .tab("Split", Split::new(Card::new().title("Left"), Card::new().title("Right")).ratio(55)),
        ),
    )
```

@live layout

## Stack

Nodes one under the other. `.gap(units)` sets the space between them in spacing units (4 px at regular density), 0 to 12.

## Row

Columns side by side. Each `.col(node)` takes a share of the width; `.weight(n)` after a column makes it wider. On narrow screens the columns stack. `.compact()` packs small items such as badges.

```rust
Row::new().col(customer_form).weight(2).col(order_totals)
```

## Card and Panel

`Card` is a plain surface with an optional title: 20 px padding, one border and no shadow. `.action(node)` puts one action at the end of the title row (a link button or a menu), `.footer(node)` adds a footer under a divider, such as a "View all" link. A grid or list that is the content of a card runs edge to edge, so there is one border, not two; a card inside a card drops its own box; cards in a `Row` stretch to the tallest one. `Panel` is a titled section with an optional toolbar; `.collapsible()` lets the user fold it and `.collapsed()` starts it folded.

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`Tabs::new(label)` with `.tab(title, node)`. Arrow keys move between tabs, as the ARIA tabs pattern describes. The content of a closed tab is not rendered until the tab is first opened, so a screen with many tabs stays light.

`.remember(KEY)` keeps the tab the user opened under a choice key (see [Client preferences](security.md#client-preferences)), so the same tabs open on it on later pages. A tab named in the URL hash wins.

## Accordion

`Accordion::new()` with `.section(title, node)` puts sections one under the other, each opened and closed by its title: the answers of a help page, the parts of a long record. Sections start closed; `.open()` after a section opens it. `.single()` keeps one section open at a time, and then only the first section marked open starts open. An accordion holds at most 50 sections (`MAX_ACCORDION_SECTIONS`).

```rust
Accordion::new()
    .section(t!("help.password"), Paragraph::new(t!("help.password_answer")))
    .open()
    .section(t!("help.invoices"), Paragraph::new(t!("help.invoices_answer")))
    .single()
```

@live accordion

Every section is a native disclosure (`<details>` and `<summary>`), not the ARIA accordion pattern of headings and buttons, because the native element needs no script, keeps closed content out of the reading order, and lets the browser's find in page open the section that holds a match. In single mode the sections share a `name`, and the browser closes the open one when another opens. Unlike `Tabs`, the content of a closed section is in the page from the start.

| Key | What it does |
|---|---|
| `Tab` | Moves to the next section title |
| `Enter` or `Space` | Opens or closes the section; focus stays on its title |

## Split

`Split::new(start, end)` shows two nodes with a handle between them. `.ratio(percent)` sets the start size. The handle moves with the pointer or the arrow keys.

## Dock

`Dock` arranges panels around a center area: left, right, bottom and center tabs that users move, resize, close and bring back. Each user's layout is saved per screen. See [Shell and commands](shell.md).
