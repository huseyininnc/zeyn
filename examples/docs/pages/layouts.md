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

`Card` is a plain surface with an optional title. `Panel` is a titled section with an optional toolbar; `.collapsible()` lets the user fold it and `.collapsed()` starts it folded.

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`Tabs::new(label)` with `.tab(title, node)`. Arrow keys move between tabs, as the ARIA tabs pattern describes. The content of a closed tab is not rendered until the tab is first opened, so a screen with many tabs stays light.

## Split

`Split::new(start, end)` shows two nodes with a handle between them. `.ratio(percent)` sets the start size. The handle moves with the pointer or the arrow keys.

## Dock

`Dock` arranges panels around a center area: left, right, bottom and center tabs that users move, resize, close and bring back. Each user's layout is saved per screen. See [Shell and commands](shell.md).
