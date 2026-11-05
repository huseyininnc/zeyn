# DataGrid

`DataGrid` muestra filas de una acción que recibe un `DataQuery` y devuelve `Page<E>`. La paginación, la ordenación, los filtros y los totales se ejecutan en el servidor, así que una tabla sobre un millón de filas cuesta al navegador lo mismo que una sobre cien.

## Una tabla

```rust
DataGrid::of(search_samples)
    .paging(Paging::pages(8))
    .column(Sample::code).width(110).filter()
    .column(Sample::name).flex(1).filter()
    .column(Sample::unit).width(120).filter().badge(|u| match u {
        Unit::Piece => Tone::Neutral,
        Unit::Kg => Tone::Info,
        Unit::Liter => Tone::Brand,
    })
    .column(Sample::price).width(150).avg()
    .column(Sample::stock).width(150).sum().progress(120)
    .column(Sample::active).width(100).check_icon()
    .row_style(|s| match s.stock {
        0 => Some(RowStyle::Danger),
        1..=9 => Some(RowStyle::Warning),
        _ => None,
    })
```

@live datagrid

Ordena haciendo clic en una cabecera, escribe en un cuadro de filtro, arrastra una cabecera para mover una columna o su borde para cambiarle el tamaño. Los totales cubren todas las filas filtradas, no solo la página.

## La acción

```rust
#[action(permission = "stock.product.read")]
pub async fn search_samples(q: DataQuery, store: SampleStore) -> Result<Page<Sample>> {
    Ok(q.apply(store.all()?))
}
```

`DataQuery::apply` filtra, ordena, totaliza y pagina un `Vec` en memoria, lo que sirve para listas pequeñas y pruebas. Un store respaldado por una base de datos lee el mismo `DataQuery` (página, orden, filtros, totales) y responde con SQL; consulta [Datos y bases de datos](database.md).

## Paginación

| Paginación | Uso |
|---|---|
| `Paging::pages(n)` | Páginas numeradas, 50 filas salvo que se indique otra cosa |
| `Paging::infinite(n)` | Carga más filas al desplazarse |
| `Paging::virtual_scroll()` | Listas muy grandes; solo las filas visibles están en la página |
| `Paging::none()` | Listas cortas que caben en una página |

El servidor limita el tamaño de página pida lo que pida el cliente.

## Columnas

| Llamada | Resultado |
|---|---|
| `.width(px)`, `.flex(n)` | Ancho fijo o una parte del ancho libre |
| `.filter()` | Un cuadro de filtro adecuado al tipo: texto, rango numérico, rango de fechas, lista de opciones |
| `.sum()`, `.avg()`, `.min()`, `.max()` | Totales en el pie. `sum` sobre una columna de texto no compila |
| `.badge(\|v\| tone)` | Muestra el valor como una insignia de color |
| `.progress(max)` | Una barra de 0 a `max` |
| `.check_icon()` | Una marca de verificación para `true` |
| `.link(\|row\| effect)` | Convierte la celda en un enlace que ejecuta un effect |
| `.pin_start()`, `.hidden()` | Fijada al inicio, u oculta hasta que el usuario la muestre |
| `.editable()` | Editable en el sitio, se guarda con `.edit_with(action)` |
| `.plain()` | Números enteros sin separadores de miles, para códigos y años |

Los números y los importes se alinean a la derecha con cifras tabulares. Los enteros se agrupan según la configuración regional del usuario (`1,234,567` en inglés, `1.234.567` en turco), en las celdas de la tabla y en valores mostrados como `DescriptionList`; los identificadores `#[key]` nunca se agrupan, y `.plain()` desactiva la agrupación en una columna.

## Filas

- `.on_open(|row| effect)`: doble clic o Enter en una fila.
- `.row_action(label, action, |row| input)`: un botón en cada fila.
- `.bulk_action(label, action, M::id)`: se ejecuta sobre las filas seleccionadas.
- `.confirm(message)` y `.tone(Tone::Danger)` justo después de `.row_action` o `.bulk_action`: pregunta antes de que esa acción se ejecute, en el mismo diálogo que `ActionButton::confirm` (el foco empieza en Cancelar), y dibuja su botón en rojo.
- `.row_menu(|row| ContextMenu)`: el menú contextual de una fila.
- `.row_style(|row| Option<RowStyle>)`: filas de peligro, advertencia o éxito.
- `.exportable()`: exportación CSV de las filas filtradas, enviada en streaming por el servidor.

La [referencia del DataGrid](grid.md) cubre vistas guardadas, maestro y detalle, agrupación, datos en árbol, edición en línea, navegación entre celdas, copiar y pegar, virtualización de columnas y accesibilidad.
