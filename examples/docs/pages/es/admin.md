# Vistas de administración

Componentes para la trastienda: qué cambió en un registro, código y datos de solo lectura, códigos de barras, notificaciones, tableros y calendarios. Cada uno se dibuja en el servidor y carga su hoja de estilos y su script solo en las páginas que lo usan.

## DiffView

`DiffView` muestra los campos que cambiaron entre dos valores de un registro: el valor antiguo tachado y el nuevo marcado como añadido. Se construye a partir de un registro de auditoría con `DiffView::changes(entry.changes)`, o a partir de dos valores de un modelo con `DiffView::of(&before, &after)`, que los compara como lo hace la traza de auditoría (sin las versiones de fila).

```rust
DiffView::of(&before, &after).title(t!("orders.history"))

DiffView::changes(entry.changes.clone())
    .model::<Order>()
    .layout(DiffLayout::Inline)
```

`.model::<M>()` da a los campos sus etiquetas en el idioma del usuario y a las opciones sus etiquetas. La disposición lado a lado es una tabla con una columna para los valores antiguos y otra para los nuevos; `DiffLayout::Inline` pone el valor nuevo después del antiguo, para sitios estrechos como una ventana. Se muestran como mucho `MAX_DIFF_FIELDS` (200) campos, con una nota para el resto.

Los campos enmascarados siguen enmascarados. Los valores son el texto enmascarado que guarda la traza de auditoría, y un campo que el modelo marca con `#[mask]` se enmascara de nuevo antes de mostrarse, así que ni siquiera una lista de cambios hecha a mano pone un valor sin enmascarar en la página. El componente no necesita script; `diff.css` (1,9 KB) se carga solo con él.

## CodeView y JsonView

`CodeView` muestra texto fuente de solo lectura: resaltado en el servidor, con números de línea, plegado y un botón de copiar. `JsonView` muestra un valor JSON sangrado de la misma forma.

```rust
CodeView::new(CodeLang::Sql, query_text).title("report.sql")

JsonView::new(&payload).title(t!("webhooks.last_payload"))

JsonView::of(&customer, &ctx)
```

El resaltador (`zeyn::ui::highlight`) está escrito a mano para Rust, JSON, TOML, SQL, JavaScript y shell; `CodeLang::Plain` y cualquier otro nombre muestran texto escapado. Envuelve palabras clave, cadenas, números, comentarios, tipos y claves en spans `z-hl-*` cuyos colores son tokens del tema, así que funcionan los dos temas y el navegador no hace ningún trabajo. Los bloques entre llaves o corchetes (`{}` y `[]`) que ocupan varias líneas se pliegan con un elemento nativo `<details>`, que no necesita script: la primera línea es el botón (Enter o Space) y un bloque plegado termina con puntos suspensivos.

`JsonView::of(value, &ctx)` serializa cualquier valor como lo envían los endpoints al navegador, así que los campos enmascarados siguen enmascarados. Se muestran como mucho `MAX_CODE_LINES` (10.000) líneas y `MAX_CODE_BYTES` (1 MB); un texto más largo se corta al final de una línea con una nota que dice cuántas líneas se muestran. El botón de copiar (`share.js`, que también sirve a `Share`) copia las líneas mostradas y lo confirma con un aviso; permanece oculto hasta que se ejecuta su script. `code.css` (2,7 KB) se carga solo con el componente.

| Tecla | Qué hace |
|---|---|
| Tab | Pasa al texto (una parada, se desplaza con las flechas) y después por los botones de plegado y el de copiar |
| Enter, Space | En un botón de plegado: pliega o despliega el bloque |

## Barcode y QrCode

`Barcode::code128(value)` y `QrCode::new(value)` dibujan un símbolo como SVG en el servidor. Los dos codificadores están escritos a mano siguiendo los estándares públicos (Code 128 y QR Code), así que no se añade ningún crate ni script.

```rust
Row::new()
    .col(Barcode::code128(order.number.clone()).height(56))
    .col(QrCode::new(order_url).level(QrLevel::Quartile).label(t!("orders.scan")))
```

`Barcode` codifica de 1 a 80 caracteres ASCII imprimibles (`MAX_BARCODE_CHARS`) y empaqueta las series de dígitos de dos en dos por símbolo (conjunto C), con el símbolo de control y una zona de silencio de 10 módulos a cada lado. `QrCode` codifica el texto como bytes UTF-8 en la versión más pequeña (de 1 a 40) que lo contiene en el nivel elegido (`QrLevel::Low`, `Medium` por defecto, `Quartile`, `High`), con corrección de errores Reed Solomon, la máscara de menor penalización y 4 módulos de zona de silencio; como mucho `MAX_QR_BYTES` (2.953) bytes. Un valor que no se puede codificar muestra una nota breve en lugar de un símbolo roto.

Cada símbolo es una figura que los lectores de pantalla oyen como una sola imagen con el nombre del texto que lleva debajo: el valor, o `.label(text)` cuando se indica. Las barras siguen negras sobre una placa blanca en los dos temas, porque los lectores de códigos necesitan barras oscuras sobre fondo claro. `barcode.css` (1 KB) se carga solo con los componentes; `.height(px)` y `.size(px)` fijan el tamaño dibujado, y el símbolo se encoge en un contenedor estrecho.

## Inbox y NotificationCenter

`Inbox` muestra las notificaciones del usuario, las más nuevas primero, agrupadas por día en la zona horaria del usuario (hoy, ayer y luego fechas), con las no leídas marcadas. Las filas vienen del mismo tipo de acción que usa un DataGrid (entra `DataQuery`, sale `Page`); la bandeja ordena por el campo de tiempo y pide como mucho `.limit(n)` filas (50, como máximo `MAX_INBOX`, 200). `NotificationCenter` es la campana de la barra superior: un enlace a la pantalla de la bandeja con el número de no leídas.

```rust
#[action(permission = "notes.write")]
pub async fn mark_notes_read(s: InboxSelection, store: Notes, ctx: Ctx) -> Result<()> {
    store.mark_read(ctx.identity(), &s.ids)
}

#[action(permission = "notes.read")]
pub async fn count_unread(q: UnreadCount, store: Notes, ctx: Ctx) -> Result<u32> {
    store.count_unread(ctx.identity(), q.limit)
}

Inbox::of(my_notifications)
    .when(Note::created_at)
    .read(Note::read)
    .item(|n: &Note| ListItem::new(n.title.clone()).subtitle(n.body.clone()))
    .mark_read(mark_notes_read)
    .archive(archive_notes)

App::new("Admin").header(|h| h.end(NotificationCenter::of(count_unread, inbox, ())))
```

Con `.mark_read(action)` y `.archive(action)` cada fila recibe una casilla y la bandeja una barra con "Seleccionar todo". Los botones envían las claves elegidas como `InboxSelection` a través de `POST /_zeyn/data`, que comprueba el CSRF, cuenta para el límite de escritura y comprueba el acceso a la pantalla y el permiso de la acción; se aceptan como mucho 200 claves de hasta 200 caracteres. Las claves vienen del navegador, así que la acción solo cambia notificaciones de `ctx.identity()`. Los botones que el usuario no puede ejecutar no se muestran, tampoco las casillas.

Después de una acción masiva la bandeja nombra su tema (`"notifications"`, `.topic(..)` para cambiarlo): la bandeja y todos los nodos que lo `.listens` se vuelven a dibujar en su sitio, y la campana vuelve a cargar su número. La campana cuenta con una acción que recibe `UnreadCount { limit }` y se detiene en el límite (100), una sola consulta acotada; por encima de 99 muestra `99+`, y el nombre del enlace dice el número a los lectores de pantalla. Carga el número con una petición cuando se muestra la página y otra vez cuando su tema lo nombra una bandeja, `ui.refresh("notifications")` o, con `.live()` y la característica `realtime`, `socket.notify(&ctx, "notifications")` en cualquier acción (declara el tema con `ZeynSocket::topic`). `inbox.js` (6,1 KB, 2,3 KB con gzip) e `inbox.css` (3,1 KB) se cargan solo con los componentes.

## Kanban

`Kanban::of(flow, cards, move_card)` muestra los registros de una acción como tarjetas en columnas, una por cada estado del enum `#[choice]` del flujo de trabajo, en su orden. Cada columna es una lista virtual sobre las tarjetas de su estado: la acción recibe un `DataQuery` limitado al estado, así que filtra en la base de datos, y las tarjetas se cargan en bloques de 50 (`KANBAN_BLOCK`) a medida que la columna se desplaza. La cabecera de la columna muestra el número de tarjetas y el límite WIP; una columna por encima de su límite se marca.

```rust
#[action(permission = "tasks.work")]
pub async fn move_task(r: TransitionRequest<TaskId>, store: Tasks, ctx: Ctx) -> Result<TaskId> {
    let mut task = store.get(r.id)?;
    task.state = task_flow().apply(&ctx, task.state, &r.transition, r.comment.as_deref())?;
    store.save(task)
}

Kanban::of(task_flow(), search_tasks, move_task)
    .state(Task::state)
    .card(|t: &Task| Stack::new().add(Paragraph::new(t.title.clone())).add(Badge::new(t.owner.clone())))
    .wip(TaskState::Doing, 5)
    .confirm("reopen", t!("tasks.reopen_confirm"))
```

Mover una tarjeta de una columna a otra ejecuta la transición entre los dos estados con un `TransitionRequest`, como hace `Transitions`. El tablero solo ofrece los movimientos que el usuario puede hacer; el servidor encuentra la transición, comprueba su permiso y el de la acción de mover (CSRF y límite de escritura a través de `POST /_zeyn/data`), y la acción la aplica con `Workflow::apply`, que rechaza una tarjeta que ya está en otro estado. `.confirm(name, message)` pregunta antes en el diálogo compartido. La tarjeta se mueve al instante; si el servidor se niega, vuelve a su columna con el motivo, y si acepta, las dos columnas se vuelven a dibujar con una sola petición.

Un ratón o un lápiz arrastran la tarjeta después de moverse 6 px, y en pantallas táctiles tras mantenerla pulsada 250 ms, así que deslizar sigue desplazando la columna (el núcleo de arrastre compartido, `drag.js`). Las columnas a las que puede ir se resaltan, las demás se atenúan, y el tablero se desplaza de lado mientras la tarjeta se mantiene cerca de su borde. El botón de cada tarjeta abre el mismo menú que el teclado, la forma de mover con un solo puntero (WCAG 2.5.7). `kanban.js` (19,7 KB, 6 KB con gzip), `drag.js` y `kanban.css` (4,5 KB) se cargan solo con el tablero, `vlist.js` con sus columnas.

`.sortable(Task::rank, reorder_tasks)` mantiene las tarjetas de cada columna en el orden de un campo `Rank`, con la acción de reordenar de una lista ordenable (`ReorderRequest`). Cada tarjeta recibe un asa que la reordena dentro de su columna (arrastre, teclas o su menú de mover). Una tarjeta soltada sobre otra queda antes o después de ella, y una línea muestra dónde: dentro de su propia columna eso envía el reordenamiento; hacia otra columna primero se ejecuta la transición y luego el reordenamiento con los nuevos vecinos de la tarjeta. Ambas peticiones pasan los controles del servidor: el nodo de la columna, CSRF, el límite de escritura y el permiso de cada acción. `sortable.js` se carga solo con un tablero ordenable.

| Tecla | Qué hace |
|---|---|
| Tab | Entra en el tablero, en la última tarjeta con foco (una parada para todo el tablero) |
| Up, Down | Tarjeta anterior o siguiente de la columna |
| Left, Right | La tarjeta en el mismo lugar de la columna anterior o siguiente (se invierten con `dir="rtl"`) |
| Home, End | Primera o última tarjeta visible de la columna |
| Enter, Space | Abre el menú de mover: Up y Down eligen columna, Enter mueve, Escape cierra |
| Shift + Left, Shift + Right | Mueve la tarjeta a la columna anterior o siguiente, cuando el flujo lo permite |
| Alt + Arriba, Alt + Abajo | En un tablero ordenable, mueve la tarjeta arriba o abajo en su columna |

## Calendar

`Calendar::of(action)` muestra eventos en un mes, una semana o un día, en la zona horaria del usuario. La acción es del tipo que usa un DataGrid y recibe solo el rango visible: un filtro conserva los eventos que empiezan hasta el último día visible y otro los que terminan desde el primero (los límites de fecha son días completos en la zona del usuario, así que el trabajo lo hace la base de datos), ordenados por inicio, como mucho `MAX_CALENDAR_EVENTS` (500). El servidor coloca cada evento y dibuja la vista entera; las horas se muestran en la zona de `Ctx` (`TimePolicy`), así que un evento a las 23:30 UTC cae al día siguiente en Berlín.

```rust
#[action(permission = "events.write")]
pub async fn new_event(slot: CalendarSlot) -> Result<Option<Effect>> {
    Ok(Some(Window::open(event_form, EventFormParams { start: slot.start, end: slot.end })))
}

#[screen(route = "/planner", permission = "events.read")]
pub fn planner(p: CalendarParams) -> Screen {
    Screen::new(t!("planner.title")).body(
        Calendar::of(search_events)
            .start(Event::starts)
            .end(Event::ends)
            .label(|e: &Event| e.title.clone())
            .open(|e: &Event| Window::open(event, EventParams { id: e.id }))
            .create(new_event)
            .at(p),
    )
}
```

La vista y la fecha son parámetros de la pantalla (`CalendarParams`, o campos llamados `view` y `date`). Los enlaces de la barra (anterior, hoy, siguiente, mes, semana, día) los cambian en su sitio a través del tema del calendario (`"calendar"`, `.topic(..)`): un solo `POST /_zeyn/nodes` vuelve a dibujar el calendario y la dirección lo sigue, así que una vista se puede compartir y funciona sin script. `.open(|e| effect)` convierte cada evento en un botón que ejecuta el efecto, como una ventana cuya pantalla comprueba su propio acceso. Con `.create(action)` el usuario elige un intervalo: un clic en un día, arrastrar sobre las horas o Shift con las flechas, y después Enter o N. El intervalo viaja como hora local a través de `POST /_zeyn/data` (CSRF, límite de escritura, permiso de la acción); el servidor lo lee en la zona del usuario, rechaza una hora que el cambio de reloj se salta y los intervalos de más de 31 días, y entrega un `CalendarSlot` a la acción, cuyo efecto se ejecuta antes de que el calendario se vuelva a dibujar. Los usuarios sin el permiso no pueden elegir intervalos. `.hours(from, to)` fija las filas de las vistas de semana y día (de 7 a 21).

Medido en el catálogo de componentes (servidor debug): cambiar entre mes, semana y día muestra la nueva vista de 21 a 34 ms después del clic a plena CPU y de 60 a 163 ms con la CPU 4 veces más lenta. `calendar.js` (9,6 KB, 3,3 KB con gzip) y `calendar.css` (4,4 KB) se cargan solo con un calendario.

| Tecla | Qué hace |
|---|---|
| Tab | Entra en la cuadrícula, en el día elegido (una parada para la cuadrícula) |
| Flechas | El día o la hora siguiente; Left y Right se invierten con `dir="rtl"` |
| Home, End | Primera o última celda de la fila |
| Page Up, Page Down | Mes, semana o día anterior o siguiente |
| Enter | Entra en los eventos de la celda (Up y Down entre ellos, Escape para volver); en una celda vacía crea |
| Shift + flechas | Amplía el intervalo elegido, cuando el calendario puede crear |
| N | Crea un evento en el intervalo elegido, cuando el calendario puede crear |
