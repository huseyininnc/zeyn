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

`Card` es una superficie simple con un título opcional: relleno de 20 px, un borde y sin sombra. `.action(node)` pone una acción al final de la fila del título (un botón de enlace o un menú), `.footer(node)` añade un pie bajo un divisor, como un enlace "View all". Un grid o una lista que es el contenido de una tarjeta llega de borde a borde, así que hay un solo borde y no dos; una tarjeta dentro de otra pierde su propia caja; las tarjetas de un `Row` se estiran hasta la más alta. `Panel` es una sección con título y una barra de herramientas opcional; `.collapsible()` permite al usuario plegarla y `.collapsed()` la inicia plegada.

```rust
Panel::new(t!("ui.panel_products")).collapsible().toolbar(|t| t.export(Export::Csv)).add(products())
```

## Tabs

`Tabs::new(label)` con `.tab(title, node)`. Las teclas de flecha se mueven entre pestañas, como describe el patrón ARIA de tabs. El contenido de una pestaña cerrada no se renderiza hasta que se abre por primera vez, así que una pantalla con muchas pestañas se mantiene ligera.

`.remember(KEY)` guarda la pestaña que abrió el usuario bajo una clave de elección (consulta [Preferencias del cliente](security.md#client-preferences)), de modo que las mismas pestañas se abren en ella en páginas posteriores. Una pestaña indicada en el hash de la URL tiene prioridad.

## Accordion

`Accordion::new()` con `.section(title, node)` pone secciones una debajo de otra, cada una abierta y cerrada por su título: las respuestas de una página de ayuda, las partes de un registro largo. Las secciones empiezan cerradas; `.open()` después de una sección la abre. `.single()` mantiene abierta una sección a la vez, y entonces solo la primera sección marcada como abierta empieza abierta. Un accordion contiene 50 secciones como máximo (`MAX_ACCORDION_SECTIONS`).

```rust
Accordion::new()
    .section(t!("help.password"), Paragraph::new(t!("help.password_answer")))
    .open()
    .section(t!("help.invoices"), Paragraph::new(t!("help.invoices_answer")))
    .single()
```

@live accordion

Cada sección es un elemento nativo de divulgación (`<details>` y `<summary>`), no el patrón ARIA de acordeón con encabezados y botones, porque el elemento nativo no necesita script, mantiene el contenido cerrado fuera del orden de lectura y permite que la búsqueda en la página del navegador abra la sección que contiene una coincidencia. En modo único las secciones comparten un `name`, y el navegador cierra la abierta cuando se abre otra. A diferencia de `Tabs`, el contenido de una sección cerrada está en la página desde el principio.

| Tecla | Qué hace |
|---|---|
| `Tab` | Pasa al título de la sección siguiente |
| `Enter` o `Space` | Abre o cierra la sección; el foco se queda en su título |

## Split

`Split::new(start, end)` muestra dos nodos con un tirador entre ellos. `.ratio(percent)` fija el tamaño inicial. El tirador se mueve con el puntero o con las teclas de flecha.

## Dock

`Dock` organiza paneles alrededor de un área central: pestañas a la izquierda, a la derecha, abajo y en el centro que los usuarios mueven, redimensionan, cierran y recuperan. El diseño de cada usuario se guarda por pantalla. Consulta [Shell y comandos](shell.md).
