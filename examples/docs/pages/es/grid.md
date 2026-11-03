# Data Grid

Estado: implementado por `zeyn::DataGrid`, los endpoints del grid en `zeyn::server` y `assets/zeyn/*.js` (servido como `/_zeyn/zeyn.js`).

El grid se renderiza en el servidor. Sus filas provienen de una action que recibe un `DataQuery` y
devuelve `Page<E>`. El runtime del navegador solicita páginas, envía lo que hace el usuario (orden, filtro,
disposición, ediciones, actions) y coloca en su lugar el HTML que recibe. Cada llamada comprueba los permisos en
el servidor, y nada de lo que envía el cliente se da por confiable.

```rust
DataGrid::of(search_products)
    .mode(DataMode::Server)
    .paging(Paging::pages(25))
    .column(Product::code).width(120).pin_start().filter()
        .link(|p| Window::open(product_editor, ProductParams { id: Some(p.id) }))
    .column(Product::name).flex(1).filter().editable()
    .column(Product::unit).filter().editable().badge(|u| match u {
        Unit::Kg => Tone::Info,
        _ => Tone::Neutral,
    })
    .column(Product::price).filter().editable().avg().max()
    .column(Product::stock).filter().sum().progress(120)
    .column(Product::active).filter().check_icon().editable()
    .edit_with(save_product)
    .row_action(t!("stock.restock"), restock_product, |p| p.id)
    .bulk_action(t!("stock.deactivate"), deactivate_products, Product::id)
    .row_menu(|p| ContextMenu::new()
        .item(t!("stock.edit"), Window::open(product_editor, ProductParams { id: Some(p.id) }))
        .action(t!("stock.restock"), restock_product, p.id))
    .exportable()
```

## Totales de columna

`sum()` y `avg()` funcionan con números enteros y dinero, `min()` y `max()` también con fechas, horas y
momentos, y `count()` con todas las columnas. Usarlos con otro tipo no compila. Los totales abarcan
todas las filas que cumplen los filtros, no solo la página.

`DataQuery::apply` los calcula para datos en memoria. Una capa de datos que pagina en SQL lee
`query.aggregates` y añade `Page::with_total(field, op, value)` a partir de una consulta de agregados. El grid
conserva solo los totales que declaran sus columnas, de modo que un cliente no puede pedir otros. Los totales se formatean
en el servidor en el idioma del usuario. Los promedios de números enteros tienen dos decimales
(`Value::Decimal`), y los promedios de dinero se redondean (half away from zero, es decir, el .5 se aleja de cero) a la unidad más pequeña.

## Filtros y orden

`filter()` añade un filtro a la fila de filtros, con la entrada elegida según el tipo de la columna:

| Columna | Filtro | Consulta |
|---|---|---|
| Texto | contiene, empieza por, es igual a | `contains`, `starts_with`, `equals` |
| Número entero, dinero | desde, hasta | `from`, `to` como `1234.56` |
| Fecha | desde, hasta | `from`, `to` como `2026-03-31` |
| Momento (`DateTime`) | desde, hasta (fechas) | un día completo en la zona horaria del usuario |
| Hora | desde, hasta | `14:30` |
| Opción | elegir cualquiera de las opciones | `any_of` |
| Sí/no | todos, sí, no | `is` |

Los valores viajan en una forma fija legible por máquina, sea cual sea el idioma del usuario. La antigua
forma `{ "field", "contains" }` sigue funcionando. El servidor descarta los filtros sobre columnas sin
`filter()` y los órdenes sobre columnas que el grid no tiene, y limita los filtros (32), el texto de filtro (200
caracteres), las opciones (100) y las claves de orden (4). Un valor que no se puede leer se ignora. La zona horaria
de los filtros de fecha proviene de los ajustes del usuario en el servidor.

Shift+clic en un encabezado añade esa columna a las claves de orden. Un número en el encabezado muestra el orden.

## Columnas: fijar, redimensionar, ocultar, mover

`pin_start()` y `pin_end()` mantienen una columna en un borde mientras el grid se desplaza lateralmente (como máximo
tres por lado). `hidden()` deja una columna fuera hasta que el usuario la muestre. Cada encabezado tiene un
menú de columna (el botón, un clic derecho, Shift+F10 o la tecla de menú contextual) con orden, fijar, mover
a la izquierda y a la derecha, ocultar, filtro, un selector de columnas y restablecer. El borde de un encabezado redimensiona la columna,
arrastrando o con las teclas de flecha (Shift para pasos mayores). Arrastrar un encabezado sobre otro encabezado
del mismo grupo fijado mueve la columna; una pulsación corta sigue ordenando. Todo funciona de derecha a izquierda.

## Vistas guardadas

La disposición del usuario (columnas mostradas en orden, anchos, columnas fijadas), el orden y los filtros forman un `DataGridView`.
Con `App::datagrid_views(store)` se guarda por usuario en el servidor, para cada tenant, pantalla y
grid, y la página vuelve con ella, con la primera página ya ordenada y filtrada. Implementa
`DataGridViewStore` para una base de datos; `MemoryViewStore` es para desarrollo y pruebas. Sin un store,
el navegador conserva la vista en `localStorage`. El servidor depura cada vista que recibe con las
columnas y los límites del propio grid.

## Celdas

| Método | Columnas | Muestra |
|---|---|---|
| `badge(\|v\| Tone)` | enums `#[choice]` | una insignia de color por opción |
| `progress(max)` | números, dinero | una barra y el número |
| `link(\|row\| Effect)` | cualquiera | el valor como un enlace que ejecuta un effect |
| `check_icon()` | `bool` | una marca de verificación o un guion, con palabras para lectores de pantalla |
| `relative_time()` | `DateTime` | "hace 3 minutos", hora exacta como tooltip |
| `cell(renderer)` | cualquiera | tu propio HTML (`CellRenderer<T>`) |

Un `CellRenderer` debe escapar los datos del usuario. Las exportaciones y las copias usan el valor simple.

## Acciones de fila, menús y acciones masivas

`row_action(label, action, |row| input)` añade un botón a cada fila. `row_menu(|row| ContextMenu)`
añade un menú contextual. `Screen::context_menu(...)` añade uno al espacio vacío de una pantalla. Los elementos
que ejecutan una action se muestran solo a los usuarios con el permiso de esa action.

Cuando se ejecuta un elemento de fila, el cliente envía solo la clave de la fila y el elemento. El servidor carga la fila
de nuevo mediante la action del grid, construye el elemento a partir de esa fila, comprueba el permiso de la action,
ejecuta los interceptores y después la action. Se rechaza un elemento que ya no existe para la fila (por
ejemplo, uno que quedó deshabilitado). La entrada nunca proviene del cliente.

`bulk_action(label, action, Model::key)` añade un botón a la barra de selección. La action recibe
`Vec<K>`, el tipo del campo clave; cualquier otra cosa no compila. Elegir "todas las filas coincidentes"
permite que el servidor reúna las claves por sí mismo con los filtros del usuario, hasta 10.000. Las claves elegidas
una a una provienen del cliente, por lo que la action debe comprobar cada registro como lo haría una action de API.

El navegador conserva su propio menú contextual en todos los demás lugares. `App::browser_context_menu(false)` lo
desactiva excepto en las entradas de texto y el texto editable. Eso no protege nada: los usuarios aún pueden leer el
código fuente de la página, abrir las herramientas de desarrollo o desactivar los scripts. Además elimina el corrector ortográfico, copiar
enlace, la traducción y las herramientas de accesibilidad. Mantén el valor predeterminado.

## Edición en línea

`edit_with(save)` con una action que recibe `Valid<E>` (la misma regla que los formularios) y `editable()` en
las columnas. Las claves, los campos de solo lectura y las versiones de fila nunca pasan a ser editables. F2 o un doble clic en
una celda editable inicia la edición de la fila; Enter guarda, Escape cancela, Ctrl+Z o el botón del toast
deshace el último guardado.

El servidor carga la fila de nuevo por clave y aplica solo las columnas editables y la versión de la fila
mediante `Form::read`, de modo que los valores se leen y validan como lo haría un formulario. Un valor para cualquier otra
columna se rechaza con `403`. Una versión obsoleta devuelve `409`, y los errores de validación devuelven
`422` y se muestran en sus celdas.

## Master detail

`detail(|row| (screen, params))` permite que cada fila se expanda en una pantalla construida a partir de la fila, cargada solo
cuando el usuario la abre:

```rust
DataGrid::of(search_today)
    .column(Order::id)
    .detail(|o| (order_lines, OrderParams { id: o.id }))

#[screen(permission = "sales.order.read")]
pub fn order_lines(params: OrderParams) -> Screen {
    Screen::window(t!("sales.order_lines", id = params.id.0.to_string())).body(
        DataGrid::of(search_lines)
            .scope(OrderLine::order_id, params.id)
            .column(OrderLine::product)
            .column(OrderLine::amount).sum()
            .detail(|l| (product_editor, ProductParams { id: Some(l.product_id) })),
    )
}
```

Los params están tipados por la pantalla (`ScreenDef::Params`), de modo que unos params incorrectos no compilan. Un
detalle es una pantalla completa: un formulario, un grid anidado o un grid con sus propios detalles, a cualquier profundidad. La
columna del expansor se muestra solo a los usuarios que pueden abrir la pantalla, y el servidor vuelve a comprobar el permiso
de la pantalla cuando se carga el detalle. Los params viajan por el navegador igual que los de una ventana,
por lo que la pantalla de detalle debe comprobar que el usuario puede ver el registro que nombran.

`scope(field, value)` limita un grid a las filas cuyo `field` es igual a `value` en cada solicitud. El
filtro va primero en `DataQuery::filters` y el cliente no puede quitarlo, de modo que un grid anidado se mantiene
en su fila padre sea lo que sea lo que envíe el navegador. Una capa de datos que construye SQL a partir de los filtros lo lee
como cualquier otro (`Filter::range` con límites iguales para números y fechas, `equals` para texto,
`any_of` para opciones, `is` para sí/no, `blank` para valores vacíos).

El botón del expansor, Enter sobre él, o Derecha e Izquierda sobre la fila abren y cierran el detalle.
Al cerrarlo se elimina, de modo que se carga de nuevo la próxima vez.

Los grids virtuales mantienen todas las filas a una altura fija para poder llegar a cualquier posición de inmediato; no
muestran expansores. Usa páginas o paginación infinita para grids con detalles.

## Agrupación

`group_by(field)` agrupa las filas en el servidor; vuelve a llamarlo para agrupar dentro de cada grupo:

```rust
DataGrid::of(search_today)
    .paging(Paging::pages(20))
    .group_by(Order::city)
    .group_by(Order::status)
    .column(Order::amount).sum().avg()
```

El grid muestra una fila de encabezado por grupo con la etiqueta de la columna, el valor, el número de filas y cada
total de columna sobre ese grupo, bajo su columna. Abrir un grupo (su botón, o Derecha e Izquierda)
carga sus subgrupos o filas, una página cada vez, con una fila "Cargar más" cuando quedan más; cerrarlo
las elimina. El nivel superior pagina sobre grupos, por lo que el paginador cuenta grupos. El pie conserva los
totales sobre todas las filas coincidentes. Las filas dentro de los grupos conservan todo lo demás: edición, acciones de fila,
menús, selección y detalles.

La capa de datos responde a `DataQuery::group`: cuando está definido, devuelve `Page::groups` (valor, recuento
y los totales que pide la consulta, por grupo) ordenados por el valor en la dirección de la clave de orden
de ese campo, paginados como las filas, con `total` contando grupos. `DataQuery::apply` hace esto para datos
en memoria; SQL usa `GROUP BY`. `group` lo define el grid y nunca se lee del cliente. El
cliente envía solo la ruta de valores de grupo que abre; el servidor comprueba su longitud y convierte cada
valor en un filtro sobre la columna de ese grupo, delante de los filtros del usuario (`blank` para el grupo
vacío).

## Datos en árbol

`tree(Model::parent)` muestra las filas como un árbol mediante una clave padre opcional: las filas sin padre en
la parte superior, y los hijos de una fila se cargan cuando esta se abre, una página cada vez, hasta un máximo de 32 niveles.

```rust
DataGrid::of(search_employees)
    .tree(Employee::manager_id)
    .column(Employee::name)
    .column(Employee::salary).sum()
```

La primera columna lleva la sangría y el botón. Para saber qué filas tienen hijos, el grid
consulta su action una vez más por página: las filas cuyo padre es una de las claves de la página, agrupadas por el
campo padre (`Filter::any_of` sobre claves y `DataQuery::group`). Los filtros y el orden se aplican en todos los
niveles; los totales del pie abarcan todas las filas coincidentes. Los grids en árbol no agrupan y no muestran
detalles de fila. Tras una edición en línea, la fila conserva su lugar en el árbol.

Los grids virtuales ni agrupan ni muestran árboles, ya que allí todas las filas tienen una altura fija.

## Celdas: navegación, copiar y pegar

Derecha sobre una fila (o un clic en una celda) coloca en ella un cursor de celda, como en una hoja de cálculo. Las teclas de flecha lo
mueven, Shift con una flecha (o Shift+clic) amplía un rango, Inicio y Fin van a los extremos de la fila y
Ctrl+Inicio y Ctrl+Fin a la primera y la última celda del grid. Tab y Shift+Tab se mueven a lo largo de la
fila y pasan a la siguiente, y salen del grid en sus extremos. Izquierda en la primera celda vuelve a la
fila. Enter o F2 edita una celda editable, Enter pulsa el único botón o enlace de una celda, Escape
descarta el rango. Las celdas del rango llevan `aria-selected`.

Ctrl+C en una celda copia el rango como texto separado por tabulaciones, con los valores tal como los ve el usuario. Ctrl+C
en una fila sigue copiando las filas seleccionadas.

Ctrl+V en una celda pega texto separado por tabulaciones de una hoja de cálculo desde el cursor; un solo valor rellena
todo el rango. Solo cambian las celdas editables (pegar en celdas de solo lectura lo indica), las opciones pueden
pegarse por etiqueta, y sí y no con las palabras del usuario. Cada fila modificada se guarda por la vía de
edición en línea (`/_zeyn/grid/edit` con la versión de la fila), una fila tras otra, de modo que cada fila se lee,
valida y comprueba en el servidor como una edición en línea; una fila que falla muestra sus errores en sus
celdas y las demás se guardan. Más de 10 filas piden confirmación antes, más de 500 se rechazan, y Ctrl+Z
deshace todo el pegado. Las ediciones del grid, las acciones de fila, las acciones masivas y las acciones de menú cuentan para
`App::write_limit`, por lo que un pegado grande puede chocar con el límite de escritura de la app.

## Virtualización de columnas

Un grid con 40 columnas o más mantiene en la página solo las columnas fijadas y las columnas a la vista (más hasta seis
a cada lado). Cada columna recibe un ancho fijo, y una celda espaciadora representa las
columnas omitidas a cada lado, de modo que los anchos, las columnas fijadas y el desplazamiento se mantienen como estaban. Las filas conservan su lista
completa de celdas, por lo que copiar, pegar y editar ven todas las columnas, y las celdas llevan `aria-colindex`.
La ventana se mueve solo cuando una columna fuera de ella entra en la vista. El servidor sigue enviando cada
celda de una fila. Un grid virtual así de ancho (`VIRTUAL_WIDE_COLUMNS`, contando las columnas de selección, detalle
y acciones de fila) envía todo su primer bloque dentro de `<template data-zeyn-rows-later>` en lugar de 64 filas
vivas, de modo que el navegador nunca maqueta una fila con todas las columnas: en el grid ancho de la galería la tarea
más larga del arranque pasó de 259 a 112 ms y DOMContentLoaded de 402 a 228 ms (medianas de 8 cargas
intercaladas, servidor release, máquina ocupada).

Medido en el grid ancho de la galería (62 columnas, 100.000 filas, desplazamiento virtual, servidor de depuración,
Chromium, ventana de 1400 por 900, pantalla de 144 Hz), con y sin virtualización de columnas:

| | Celdas en la página | Lateral, 60 px por fotograma (p50 / p95) | Hacia abajo, 60 px por fotograma (p50 / p95) | Hacia abajo, 2.000 px por fotograma (p50 / p95) |
|---|---|---|---|---|
| Sin | 2.976 | 6,9 / 7,5 ms | 42,7 / 64,8 ms | 78,5 / 141 ms, 178 tareas largas |
| Con | 1.056 | 7,0 / 28,8 ms | 24,6 / 41,7 ms | 28,9 / 48,9 ms, 7 tareas largas |

El desplazamiento hacia abajo es unas dos veces más rápido y la página contiene un tercio de las celdas. El desplazamiento lateral
ya era fluido; los fotogramas en los que se mueve la ventana cuestan hasta 29 ms. El resto del coste es
el análisis de bloques de 200 filas con todas las columnas, algo que la virtualización de columnas no cambia.

## Exportación

`exportable()` (y opcionalmente `export_permission(...)`, `export_limit(n)`) permite que el botón `Export::Csv`
de la barra de herramientas descargue todas las filas coincidentes. El servidor pagina a través de la action del grid y
transmite cada página de inmediato, con la disposición y el idioma del usuario. Las fórmulas se neutralizan. Hasta 100.000
filas de forma predeterminada.

Con la feature de Cargo `xlsx`, el botón `Export::Xlsx` de la barra de herramientas descarga las mismas filas como un
libro de Excel (`GET /_zeyn/grid/export?format=xlsx`). Las celdas conservan sus tipos: números, dinero como
número con su formato de moneda, fechas y fechas con hora como fechas reales de Excel en la zona horaria del usuario,
y booleanos. Las opciones se escriben como sus etiquetas, los campos enmascarados como su texto enmascarado, y el texto es
siempre texto, de modo que nada se convierte en una fórmula. La fila de encabezado contiene las etiquetas en el idioma del usuario,
va en negrita e inmovilizada, y las columnas conservan el orden y los anchos del usuario.

El libro se escribe con `rust_xlsxwriter` en modo de memoria constante: cada página va a un
archivo temporal a medida que llega (como máximo dos páginas esperan en memoria), y el libro comprimido se
transmite a la respuesta al final. Exportar 100.000 filas de seis columnas mantuvo el heap en unos
134 KiB mientras se escribían las filas (lo mismo tras 10.000 que tras 100.000 filas) y 460 KiB durante
la compresión; el archivo pesaba 3,4 MB.

Con la feature de Cargo `pdf`, `Export::Pdf` imprime las mismas filas como un informe de lista (como máximo 2.000
filas); consulta [Documentos PDF](operations.md#pdf-documents).

## Selección

`selectable()` (implícito en `bulk_action`) añade una columna de casillas. Shift selecciona un rango, Espacio
selecciona la fila con el foco, y la casilla del encabezado selecciona la página. La barra de selección ofrece todas las
filas coincidentes.

## Accesibilidad

La tabla tiene `role="grid"`, `aria-rowcount` (grids virtuales), `aria-colcount`, `aria-colindex`
y `aria-sort`. Las filas usan una parada de tabulación móvil (roving tab stop). Los botones dentro de la fila activa son accesibles con Tab. Los grids agrupados y en árbol
son `role="treegrid"` con `aria-level` y `aria-expanded` en las filas.
Los menús usan `role="menu"` con `menuitem`, `menuitemcheckbox` y `menuitemradio`, y el foco vuelve
a donde estaba. Los estados vacío y de error se anuncian (`role="status"`).

## Rendimiento

Medido en el archivo del showcase (1.000.000 de filas, desplazamiento virtual, servidor de depuración, Chromium,
pantalla de 144 Hz) con la Performance API:

| Desplazamiento | Fotograma p50 | Fotograma p95 | Máx | Tareas largas |
|---|---|---|---|---|
| Velocidad de rueda (60 px por fotograma) | 13,9 ms | 20,9 ms | 34,6 ms | 0 |
| Rápido (2.000 px por fotograma) | 14,1 ms | 27,8 ms | 54,9 ms | 0 |
| Barridos por toda la lista | 13,9 ms | 27,8 ms | 48,6 ms | 0 |

El DOM contiene 62 filas (unos 660 elementos), y el heap de JS se mantiene cerca de 4 MB. Exportar 100.000 filas
del mismo archivo transmitió 7 MB en 3,4 segundos.

### Mantén barato el constructor de la pantalla

Las peticiones no guardan estado, así que cada petición de un grid reconstruye la pantalla que lo contiene: cada bloque que pide un grid virtual (cada 200 filas desplazadas), cada página, ordenación, filtro y exportación, cada edición en línea y cada acción de fila ejecutan de nuevo la función `#[screen]` antes de ejecutar la action del grid. El servidor necesita la pantalla para encontrar el grid por su ruta de nodo y comprobar el acceso, así que este es el precio de no guardar estado por usuario.

Un constructor que solo coloca componentes cuesta microsegundos. Un constructor que consulta la base de datos paga esa consulta en cada una de esas peticiones. Por eso:

- Carga las filas en la action del grid (`DataGrid::of(search)`). De todos modos se ejecuta una vez por petición y recibe el `DataQuery` con la página, la ordenación y los filtros.
- Las opciones y las búsquedas pertenecen al tipo del campo (`#[choice]`), a una búsqueda de formulario (`Form::lookup`, que consulta su action cuando el usuario escribe) o a un servicio que las guarda en una caché con límite, no a una consulta que ejecute el constructor.
- Cuando una pantalla debe cargar un registro (`Form::of(record)`), mantén fuera de ella los grids grandes o virtuales: ponlos en su propia pantalla, en una pantalla de detalle (`DataGrid::detail`) o en una ventana, para que sus peticiones de bloque reconstruyan una pantalla que no carga nada.
- Los parámetros de la pantalla son la entrada que necesita el constructor; limítalos a claves y deja que la action del grid filtre por ellos (`DataGrid::scope`).
