# Árbol y gráficos

## Tree

`Tree::of(action)` muestra una jerarquía que se carga bajo demanda. La acción recibe un `TreeQuery` y devuelve los hijos de `q.parent` (`None` para el nivel superior). Una rama carga sus hijos la primera vez que se abre, así que un árbol sobre una jerarquía grande cuesta una petición pequeña por cada rama abierta.

```rust
#[action(permission = "stock.product.read")]
pub async fn sample_tree(q: TreeQuery, store: SampleStore) -> Result<Vec<TreeItem>> {
    Ok(match q.parent.as_deref() {
        None => vec![TreeItem::branch("piece", "Piece"), TreeItem::branch("kg", "Kilogram")],
        Some(unit) => store.by_unit(unit)?.into_iter().map(|s| TreeItem::leaf(s.id.0.to_string(), s.name).hint(s.code)).collect(),
    })
}

Tree::of(sample_tree).label("Products by unit")
```

@live tree

El árbol sigue el patrón ARIA tree: las teclas de flecha se mueven entre elementos y abren o cierran ramas. El servidor limita el número de hijos por rama (`MAX_TREE_CHILDREN`).

## Gráficos

`Chart::bar`, `stacked_bar`, `line`, `area`, `combo`, `scatter`, `donut`, `gauge` y `funnel` dibujan SVG en el servidor. No hay biblioteca de gráficos en el navegador; un gráfico es parte del HTML de la página.

```rust
Row::new()
    .col(Chart::bar("Sales").labels(["Jan", "Feb", "Mar", "Apr", "May", "Jun"])
        .series("2025", [120.0, 98.0, 143.0, 160.0, 151.0, 189.0])
        .series("2026", [131.0, 122.0, 150.0, 171.0, 166.0, 204.0]))
    .col(Chart::donut("Units").labels(["Piece", "Kg", "Liter"]).series("Products", [40.0, 40.0, 40.0]))

Chart::stacked_bar("Channels").labels(months).series("Shop", shop).series("Web", web)
Chart::combo("Sales and margin").labels(months).series("Sales", sales).line_series("Margin %", margin).right_axis()
Chart::area("Visits").labels(months).series("Visits", visits)
Chart::scatter("Price and units").points("Tea", [(1.5, 40.0), (2.5, 22.0)])
Chart::gauge("Capacity").value(72.0).range(0.0, 100.0).target(85.0)
Chart::funnel("Checkout").labels(["Visited", "Cart", "Paid"]).series("People", [1000.0, 420.0, 150.0])
```

@live chart

Cada gráfico tiene un título, un resumen en texto para lectores de pantalla, una leyenda cuando tiene dos series o más, y sus números como tabla bajo *Show data*. Los gráficos de barras, líneas, áreas y combinados dibujan en SVG solo su geometría, que se estira con la tarjeta (`--z-chart-h`, 240 px, 200 px en teléfonos); las etiquetas de los ejes son HTML a su lado, así que se mantienen en 12 px a cualquier ancho, se abrevian a partir de diez mil (`600K`, `1.2M`), y las etiquetas de categoría se reducen para que se vean como mucho doce (la mitad en gráficos de menos de 480 px). Las barras ocupan el 60% de su banda; las líneas miden 2 px y no llevan puntos, salvo un punto redondo en el último valor y en la categoría bajo el cursor; las áreas se desvanecen. El foco del puntero o del teclado sobre una categoría dibuja una cruz y un tooltip con todas las series en esa categoría. Los colores de las series son los tokens `--z-chart-1` a `--z-chart-8`, en un orden cuyos cinco primeros siguen distinguiéndose con daltonismo; las series 7 y 8 además dibujan sus líneas discontinuas. La comprobación de arranque y una prueba (`chart_colors_reach_three_to_one_in_both_themes`) exigen a cada serie 3:1 frente a `--z-surface` (consulta [Theming](theming.md)).

Cada barra, punto, porción, etapa y valor de gauge es una marca con su etiqueta y su valor como nombre. El SVG es un grupo con el nombre del título y del resumen, y el gráfico es una sola parada de tabulación:

| Tecla | Qué hace |
|---|---|
| Tab | Entra en el gráfico, en la marca enfocada por última vez (la primera al principio) |
| Izquierda, Derecha | Marca anterior o siguiente de la misma serie (se invierten con `dir="rtl"`) |
| Arriba, Abajo | El mismo punto de la serie anterior o siguiente |
| Inicio, Fin | Primera o última marca de la serie |
| Escape | Oculta el tooltip |

El cursor, el foco y un toque muestran el mismo texto en un tooltip, que se oculta a los lectores de pantalla para que nada se lea dos veces. Sin el script las marcas siguen teniendo nombre y la tabla permanece. Las marcas pequeñas (puntos de 8 px) están por debajo del tamaño de objetivo de 24 px; la tabla bajo *Show data* es el camino equivalente (WCAG 2.5.8).

## Cifras clave, sparklines y bullets

`Kpi` muestra un valor como el texto más grande de la tarjeta (30 px, cifras tabulares, el símbolo de moneda o la unidad más pequeños y atenuados), su cambio frente al periodo anterior como una pequeña etiqueta como `+48.8%` con "vs previous period" al lado (verde cuando el cambio es bueno, rojo cuando no, gris por debajo de medio punto porcentual; `.lower_is_better()` para costes y esperas; los lectores de pantalla oyen la frase completa), un objetivo con la proporción alcanzada y una tendencia a todo el ancho de los valores recientes, del color de la etiqueta, sobre un área que se desvanece. Los valores de siete cifras o más se abrevian (`$1.23M`, `1,23 Mn` en turco), con el valor completo en el tooltip y para los lectores de pantalla.

```rust
Kpi::new("Revenue this month", 184_250.0).delta(12.4).target(200_000.0).trend([140.0, 152.0, 160.0, 184.0])
Kpi::new("Average wait", 6.5).decimals(1).unit("min").delta(-9.0).lower_is_better()
Kpi::money("Revenue", Money::tl(18_425_050)).delta(4.2).target(200_000.0)
```

`Kpi::money` muestra un importe con su moneda en la configuración regional del usuario (`₺184,250.50` en inglés, `184.250,50 ₺` en turco, con el símbolo más pequeño), y su objetivo en la misma moneda.

Los Kpi de un mismo `Row` o `AutoGrid` tienen la altura del más alto, tengan o no tendencia, y el sparkline queda al pie de cada tarjeta. Un `Row` de Kpi da a cada uno un cuarto del ancho: cuatro llenan la fila en un escritorio, y la fila pasa a dos por línea por debajo de unos 830 px; por debajo de 360 px las tarjetas quitan su tendencia (con container queries sobre la fila, así que una fila de Kpi en una columna estrecha se reparte antes). Cada tarjeta conserva al menos 132 px, con o sin tendencia. Los gráficos ocupan el ancho de su contenedor. Los de anillo, los indicadores, los embudos y los de dispersión mantienen sus proporciones; los de anillo y los indicadores miden como mucho 18rem de alto y quedan centrados.

```rust
Row::new()
    .col(Kpi::money("Revenue", revenue).delta(12.4).trend(weekly))
    .col(Kpi::new("Orders", 1_284.0).delta(3.1))
    .col(Kpi::new("New customers", 312.0).delta(-2.0))
    .col(Kpi::money("Average order", average).delta(0.0))
```

En un `DataGrid`, una columna `Vec<f64>` se dibuja como sparkline y una columna de número o dinero como bullet frente a un objetivo:

```rust
DataGrid::of(search_reps)
    .column(Rep::sales).cell(BulletCell::new(80_000.0, 120_000.0))
    .column(Rep::weeks).cell(SparklineCell::new())
```

Ambos son un SVG pequeño por celda con un nombre como "Trend of 12 values from 40 to 61, lowest 12, highest 99". Presupuesto: unos 0,65 KB de HTML por fila para las dos celdas; 10.000 filas de ambas se renderizan en unos 0,4 s en una compilación de depuración (`sparkline_and_bullet_cells_render_ten_thousand_rows_quickly`), y las tablas solo renderizan las filas visibles. `charts.js` añade 3,7 KB sin comprimir (1,4 KB con gzip) y `charts.css` 4,1 KB (1,3 KB con gzip).
