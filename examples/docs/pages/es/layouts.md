# Layouts

Los componentes de layout colocan otros componentes. No contienen datos ni añaden peticiones; una pantalla construida con ellos es una sola respuesta HTML.

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

Nodos uno debajo de otro. `.gap(units)` fija el espacio entre ellos en unidades de espaciado (4 px con densidad normal), de 0 a 12.

## Row

Columnas una al lado de otra. Cada `.col(node)` toma una parte del ancho; `.weight(n)` después de una columna la hace más ancha. En pantallas estrechas las columnas se apilan. `.compact()` agrupa elementos pequeños como las insignias.

```rust
Row::new().col(customer_form).weight(2).col(order_totals)
```

## Card y Panel

`Card` es una superficie simple con un título opcional. `Panel` es una sección con título y una barra de herramientas opcional; `.collapsible()` permite al usuario plegarla y `.collapsed()` la inicia plegada.

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`Tabs::new(label)` con `.tab(title, node)`. Las teclas de flecha se mueven entre pestañas, como describe el patrón ARIA de tabs. El contenido de una pestaña cerrada no se renderiza hasta que se abre por primera vez, así que una pantalla con muchas pestañas se mantiene ligera.

## Split

`Split::new(start, end)` muestra dos nodos con un tirador entre ellos. `.ratio(percent)` fija el tamaño inicial. El tirador se mueve con el puntero o con las teclas de flecha.

## Dock

`Dock` organiza paneles alrededor de un área central: pestañas a la izquierda, a la derecha, abajo y en el centro que los usuarios mueven, redimensionan, cierran y recuperan. El diseño de cada usuario se guarda por pantalla. Consulta [Shell y comandos](shell.md).
