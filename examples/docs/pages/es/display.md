# Visualización

Componentes que muestran estado: mensajes, insignias, progreso, pares clave valor, historial y marcadores de posición. Se renderizan en el servidor y no necesitan peticiones propias.

```rust
Stack::new()
    .add(Alert::info("Imports run in the background.").dismissible())
    .add(Alert::warning("Three products are low on stock.").title("Stock"))
    .add(Row::new().compact().col(Badge::new("Paid").tone(Tone::Success)).col(Badge::new("Overdue").tone(Tone::Danger)).col(Badge::new("Draft")))
    .add(Progress::new("Import", 64, 100))
    .add(Timeline::new()
        .event(ctx.now().add_ms(-26 * 3_600_000), "Created", None, Tone::Info)
        .event(ctx.now().add_ms(-3_600_000), "Approved", Some(Text::from("by the manager")), Tone::Success))
    .add(Skeleton::lines(2))
```

@live display

## Alert

`Alert::info`, `success`, `warning` y `danger`, o `Alert::new(tone, text)`. `.title(text)` añade un encabezado y `.dismissible()` un botón de cierre. El tono también se pronuncia antes del mensaje, así que nunca se da solo por el color.

## Badge

`Badge::new(text).tone(Tone::...)`. Tonos: `Neutral`, `Info`, `Success`, `Warning`, `Danger` y `Brand`. Una insignia con tono muestra el texto del tono sobre su tinte sutil; `Neutral` es una etiqueta atenuada con un contorno, para valores simples como un plan. `.dot()` dibuja un punto de 8 px en el tono antes del texto normal, la forma más discreta de mostrar un estado en listas; `.count()` dibuja una píldora redonda para un número. Las tablas muestran valores como insignias con `.badge(|v| tone)` y como puntos con `.status(|v| tone)`.

## Progress

`Progress::new(label, value, max)` con un `.tone(...)` opcional. Es un `progressbar` real con su etiqueta y su valor.

## ProgressRing

`ProgressRing::new(label, value, max)` muestra el mismo progreso como un anillo con el porcentaje en el centro y la etiqueta debajo, para una parte de un todo en poco espacio: una cuota usada, un perfil completado. `.tone(...)` da color al anillo y `.size(px)` fija su diámetro, 64 salvo que se indique y mantenido entre 32 y 160. El anillo es un SVG en línea dibujado por el servidor, así que no necesita script y se ve nítido en cualquier densidad. Es un `progressbar` real nombrado por su etiqueta; un valor por encima del máximo cuenta como el máximo.

```rust
ProgressRing::new(t!("account.storage_used"), used, quota).tone(Tone::Warning).size(96)
```

@live rings

## Meter

`Meter::new(label, value, min, max)` muestra una medida dentro de un rango conocido, como el espacio de disco usado o un nivel de existencias. A diferencia de `Progress`, no es una tarea que avanza hacia su final: el valor puede subir y bajar, y `.low(n)` y `.high(n)` marcan dónde pasa a ser bajo o alto. La barra toma su color de ese estado: sin dirección, el rango entre los umbrales es bueno y los dos extremos son una advertencia; `.better(Better::Higher)` hace bueno un valor alto y malo uno bajo, y `Better::Lower` lo contrario. El estado nunca se da solo con el color: "bajo" o "alto" se escribe junto al valor en el idioma del usuario y forma parte de lo que los lectores de pantalla oyen del `meter` de ARIA. Los valores son números enteros, agrupados como los agrupa el idioma del usuario, y `.unit(text)` añade una unidad.

```rust
Meter::new(t!("server.disk_used"), used_gb, 0, total_gb)
    .high(total_gb * 8 / 10)
    .better(Better::Lower)
    .unit("GB")
```

## Indicator

`Indicator::on(node)` pone un contador o un punto en la esquina de otro componente: mensajes sin leer sobre un avatar, algo nuevo en un enlace. `.count(n)` muestra el número, "99+" por encima de 99 y nada en cero; `.dot()` muestra un punto; `.tone(...)` fija el color, `Danger` salvo que se indique. La marca en sí está oculta para los lectores de pantalla, porque un "3" suelto después de un nombre dice poco: `.label(text)` da las palabras que oyen en su lugar, como "3 mensajes sin leer". Dé una siempre que la marca tenga un significado que el componente de debajo no exprese ya; sin etiqueta, un contador se lee como su número y un punto no se lee.

```rust
Indicator::on(Avatar::new(user.name.clone()))
    .count(unread)
    .label(t!("inbox.unread_count").arg("n", unread.to_string()))
```

@live meters

## DescriptionList

Pares clave valor, a partir de los campos de un registro o a mano:

```rust
DescriptionList::of(&invoice)
    .field(Invoice::customer)
    .field(Invoice::issued)
    .item(t!("ui.status"), "INV-2026-0042")
```

Los valores se formatean como las celdas de una tabla: dinero, fechas y choices en el idioma del usuario.

## Timeline

`Timeline::new().event(moment, title, detail, tone)` lista eventos. Los instantes se muestran en la zona horaria y el idioma del usuario.

## EmptyState y Skeleton

`EmptyState::new(title).text(text)` llena un área vacía con un siguiente paso: `.link(label, screen, params)` va a otra página mediante un `LinkButton`, y `.opens(label, screen, params)` abre una pantalla de la aplicación como ventana. `Skeleton::lines(n)` es un marcador de posición mientras algo se carga.

## LinkButton

Ir a un sitio es un enlace; hacer algo es un botón. `LinkButton::new(label, screen, params)` es un `<a href>` real a la ruta canónica de la pantalla (como `/products/42`) con aspecto de botón, de modo que el clic central, «abrir en una pestaña nueva» y los buscadores lo tratan como el enlace que es. Se abre mediante `nav.js` sin recargar la página, y Enter lo sigue. El aspecto es secundario salvo que `.primary()` pida el aspecto relleno de la acción principal. La pantalla debe tener una ruta, o el código no compila. `EmptyState::link` y `Hero::link` lo usan.

```rust
LinkButton::new(t!("cart.browse"), products, ()).primary()
```

## ClearPrefs

`ClearPrefs::new(label)` es un botón que elimina todas las preferencias del cliente que este navegador guarda para la aplicación, por ejemplo en una página de privacidad junto a la lista de `App::client_prefs_table()`. Un toast lo confirma. Las cookies que establece el servidor (tema, idioma, sesión) no se tocan. Los scripts pueden hacer lo mismo con `zeyn.prefs.clear()`.

## Breadcrumb

`Breadcrumb::new().link(label, screen, params).current(label)`: los enlaces son referencias tipadas a pantallas, así que una pantalla renombrada no deja un enlace roto.

## ActionButton

`ActionButton::new(label, action, input)` ejecuta una acción. `.confirm(text)` pregunta primero, `.tone(Tone::Danger)` marca una destructiva y `.on_success(|result, ui| ...)` ejecuta effects. La entrada se fija en el servidor y nunca viene del cliente; el permiso de la acción se comprueba en el servidor como en cualquier formulario. `.tone(Tone::Brand)` lo dibuja como la acción principal de su zona, por ejemplo una tarjeta.

```rust
ActionButton::new(t!("docs.live.count"), count_samples, ())
    .on_success(|n, ui| {
        if let Ok(n) = n {
            ui.toast(t!("docs.live.counted", n = n));
        }
    })
```

@live action

## Markdown

`Markdown::new(text)` muestra texto con formato, como un comentario, la descripción de un producto o una ayuda. El texto es una cadena o una clave de locale. Necesita la feature `markdown` (`zeyn = { features = ["markdown"] }`), que no añade dependencias.

```rust
Markdown::new(product.description.clone())
Markdown::new(t!("help.import")).trusted()
```

- Encabezados, párrafos, listas, tablas, bloques de código, citas, reglas, código en línea, enlaces, imágenes, negrita y cursiva.
- El HTML sin procesar nunca se deja pasar: `<script>` aparece como texto, y los bloques de código también se escapan.
- Los enlaces solo pueden apuntar a `http`, `https`, `mailto`, un ancla o una ruta relativa. Los demás, como `javascript:` o `data:`, conservan solo su etiqueta. Los enlaces de usuario reciben `rel="noopener noreferrer nofollow ugc"`.
- Las imágenes se cargan solo desde la propia aplicación (una ruta relativa); las demás imágenes muestran su texto alternativo.
- `.trusted()` es para texto que escribió la aplicación: los encabezados reciben ids para enlazar a ellos y los enlaces pierden `nofollow ugc`. El escape y las reglas de enlaces siguen igual.
- El texto de más de 64 KB (`MAX_MARKDOWN`) se recorta y el anidamiento está limitado, de modo que una entrada hostil sigue siendo barata de renderizar.

## Embed

`Embed` muestra una página de otro sitio, como un reproductor de vídeo o un mapa, en un marco bloqueado. Se requiere un título (los lectores de pantalla anuncian los marcos por él); sin `.title(...)` el embed no se puede colocar y el compilador lo dice.

```rust
Embed::url("https://www.youtube-nocookie.com/embed/abc")
    .title(t!("tour.title"))
    .allow([EmbedPermission::Fullscreen, EmbedPermission::PictureInPicture])
    .aspect(16, 9)
    .click_to_load("/static/tour.jpg")
```

- El host debe estar en el `frame-src` de la aplicación: `App::csp(|c| c.frame_src(["https://www.youtube-nocookie.com"]))` (consulta [Seguridad](security.md#csrf-csp-and-headers)). Cuando la dirección no es `https` o su host no está en la lista, no se renderiza ningún marco; una advertencia dice que el contenido no se puede mostrar.
- Cada marco recibe `sandbox="allow-scripts allow-same-origin allow-presentation"`, `referrerpolicy="no-referrer"` y `loading="lazy"`. Las ventanas emergentes, la navegación del nivel superior, los formularios y las descargas nunca se pueden permitir. `.allow(...)` añade funciones del navegador: `Fullscreen`, `PictureInPicture`, `Autoplay` y `EncryptedMedia`.
- `.click_to_load(poster)` muestra primero un botón, con la imagen de póster (una ruta de la aplicación o un host de `img-src`; `""` para ninguna) y el nombre del otro sitio. No se contacta con el otro sitio hasta que se pulsa el botón; entonces el marco ocupa su lugar y recibe el foco. El script forma parte de `media.js`, así que no cuesta ninguna petición extra.
- `.notice(t!(...))` pone un texto de la aplicación en ese botón, como una advertencia de contenido. Se escapa, se traduce como cualquier texto y forma parte del nombre accesible del botón, de modo que los lectores de pantalla lo leen antes de que el usuario decida. Sin `.click_to_load` activa el botón sin póster.
- `.remember(EXTERNAL_VIDEO)` pregunta una vez durante la vida útil de la clave en lugar de en cada página. La clave es una clave de consentimiento declarada con `ClientPref::consent` y registrada con `App::client_prefs` (consulta [Preferencias del cliente](security.md#client-preferences)); los embeds vinculados a la misma clave comparten la respuesta. Las páginas posteriores, las navegaciones dentro de la aplicación y otras pestañas cargan por sí solas esos marcos, sin mover el foco (WCAG 2.2 3.2.1 On Focus, 2.4.3 Focus Order); el foco pasa al marco solo tras una pulsación real. El botón vuelve cuando el registro ha terminado o no supera sus comprobaciones, cuando la aplicación no registró la clave, cuando el almacenamiento está bloqueado y la página es nueva, y sin JavaScript. Una cadena simple o una clave de elección no compila.
- Un campo que contiene una dirección para incrustar lleva `#[rule(embed)]`. Los formularios aceptan una URL o código de embed pegado, conservan solo el `src`, exigen `https` y un host de la lista, descartan los parámetros de seguimiento (`utm_*`, `fbclid`, `gclid` y otros) y guardan la dirección canónica, nunca el HTML. Las entradas de la API deben enviar la dirección canónica.

```rust
#[entity(table = "videos")]
pub struct Video {
    #[key]
    pub id: VideoId,
    #[rule(required, embed)]
    pub url: String,
}
```

## Image

`Image` necesita su texto alternativo y su tamaño; sin `.alt(...)` y `.size(width, height)` no se puede colocar y el compilador lo dice. El tamaño mantiene el espacio mientras se carga el archivo, así que la página no se mueve.

```rust
Image::new("https://images.example.com/logo.png")
    .alt(t!("brand.logo"))
    .size(240, 80)
    .lazy()
    .srcset([("https://images.example.com/logo@2x.png", "2x")])
```

- `src` es una ruta de la aplicación (`/static/logo.png`) o una dirección `https` en un host del `img-src` de la aplicación (`App::csp(|c| c.img_src([...]))`). Cualquier otra imagen no se renderiza y en su lugar se muestra su texto alternativo, de modo que el navegador nunca registra una petición bloqueada.
- Cada imagen recibe `decoding="async"`; `.lazy()` añade `loading="lazy"` para las imágenes que quedan fuera de la primera vista. `srcset` admite descriptores de densidad (`2x`) o de ancho (`800w`); las entradas en otros hosts o con otros descriptores se omiten. Los descriptores de ancho añaden `sizes` a partir del ancho de la imagen.
- Una imagen puramente decorativa recibe un texto alternativo vacío: `.alt(Text::from(""))`.

### Listas largas de fotos

En un catálogo con miles de fotos de productos, una imagen puede mostrar algo mientras se carga y recuperarse cuando falla. Todo usa funciones nativas del navegador; un solo listener del módulo de medios atiende a todas las imágenes de la página.

```rust
Image::new(p.photo_800.clone())
    .alt(p.name.clone())
    .size(400, 500)
    .lazy()
    .srcset([(p.photo_400.as_str(), "400w"), (p.photo_800.as_str(), "800w")])
    .sizes("(min-width: 800px) 25vw, 50vw")
    .placeholder(Placeholder::color("#d9dde3"))
    .low(p.photo_40.clone())
    .fallback("/static/no-photo.webp")
    .on_error(t!("shop.no_photo"))
    .fit(Fit::Cover)
```

- `.placeholder(...)` rellena la caja mientras se carga el archivo, sin script: `Placeholder::color("#d9dde3")` o un token del tema (`"--z-surface-2"`), `Placeholder::blur(data_uri)` para una imagen diminuta creada por la aplicación (una URI base64 `data:image/webp`, `avif`, `jpeg` o `png` de 600 bytes como máximo, nunca SVG), o `Placeholder::image(src)` para un único archivo compartido por todas las imágenes. Un placeholder que incumple una regla se omite.
- `.low(src)` muestra al instante un archivo pequeño de baja resolución bajo el completo. El archivo completo se carga a la vez (de forma diferida cuando la imagen es diferida) y aparece con un fundido cuando está listo; con movimiento reducido aparece sin fundido.
- `.fallback(src)` prueba otro archivo una vez cuando la imagen falla. Después se muestra `.on_error(text or component)`; sin él, el texto alternativo con un icono sobre el placeholder. No se vuelve a intentar nada.
- `.priority()` marca la primera imagen que ve un visitante (el pintado más grande): `loading="eager"` y `fetchpriority="high"`, nunca diferida. Úsalo una vez por página.
- `.sizes(...)` indica el ancho de la imagen en el diseño, para que el navegador elija el ancho correcto de `srcset`. Sin él, una imagen diferida recibe `sizes="auto"` y el navegador mide el hueco.
- `.fit(Fit::Cover)` rellena la caja y recorta lo que sobra; `Fit::Contain` muestra el archivo entero.
- `low`, `fallback` y `Placeholder::image` siguen las mismas reglas de dirección que `src`; los demás se omiten. La caja conserva siempre el tamaño de la imagen, así que nada se mueve mientras los archivos se cargan o fallan.

## ExternalLink

`ExternalLink::new(url, text)` enlaza con otro sitio con `rel="noopener noreferrer"`, de modo que el otro sitio no recibe ni la dirección de la página ni un control sobre la ventana. Los lectores de pantalla oyen "external site" tras el texto; una flecha lo marca visualmente.

```rust
ExternalLink::new("https://example.com/help", t!("help.center")).new_tab()
ExternalLink::new(customer.website.clone(), t!("customer.website")).user_content()
```

`.user_content()` añade `nofollow ugc` para las direcciones que escribieron los usuarios. `.new_tab()` abre una pestaña nueva y se lo dice a los lectores de pantalla; los enlaces se quedan en la misma pestaña salvo que se indique. Solo las direcciones `https` y `http` se convierten en enlaces; cualquier otra cosa, como `javascript:`, se muestra como texto plano.

## Tooltip, popover y superposición de ocupado

Un tooltip es una descripción corta de un control: `ActionButton::tip(text)`, `ToolButton::tip(text)` en una barra de herramientas de pantalla y `Form::tip(field, text)`, que añade un botón de información tras la etiqueta del campo. El texto es el `aria-describedby` del control, así que los lectores de pantalla lo oyen con el control. Se muestra tras un breve instante con el cursor encima y de inmediato con el foco, permanece mientras el puntero está sobre él (WCAG 1.4.13), se oculta con Escape sin mover el foco y se muestra en pantallas táctiles tras una pulsación larga (un toque en el botón de información). Un tooltip nunca lleva información que el usuario deba tener: ponla en la etiqueta, en las reglas del campo o en la página.

`Popover::new(label).title(title).add(node)` es un botón que abre un diálogo no modal junto a él, para pequeños detalles. El foco pasa a su interior; Escape, el botón de cierre o un clic fuera lo cierran y el foco vuelve al botón. En pantallas estrechas se abre como una hoja inferior. Sus hijos son nodos normales, así que un formulario dentro de él se envía a su propia ruta de nodo con todas las comprobaciones del servidor.

`Busy::new().add(node)` cubre su contenido mientras se ejecuta una petición dentro de él: un formulario guardando, un botón de acción, una tabla o un árbol cargando. El contenedor recibe `aria-busy` de inmediato; el spinner y el texto "Working" (un mensaje de estado) se muestran solo tras 400 ms, o `.delay_ms(n)`, de modo que las respuestas rápidas no parpadean, y permanecen al menos 300 ms una vez mostrados. La superposición bloquea el puntero, no el teclado, y el foco se queda donde estaba.

```rust
Stack::new()
    .add(ActionButton::new(t!("docs.live.count"), count_samples, ()).tip(t!("docs.live.count_tip")))
    .add(Popover::new(t!("docs.live.rules")).title(t!("docs.live.rules_title")).add(rules_list))
    .add(Busy::new().add(ActionButton::new(t!("docs.live.slow"), slow_count, ())))
```

@live overlay

| Tecla | Dónde | Qué hace |
|---|---|---|
| Tab | un control con tooltip | El foco muestra el tooltip; al salir se oculta |
| Escape | en cualquier sitio mientras se muestra un tooltip | Oculta el tooltip; el foco se queda |
| Enter, Espacio | botón del popover | Abre el popover; el foco pasa a su primer control |
| Escape | dentro del popover | Lo cierra; el foco vuelve al botón |
| Tab | dentro del popover | Recorre sus controles; al salir se cierra |

Los tres necesitan unos 2,2 KB de script con gzip y ninguna petición propia; un tooltip es 1 nodo, un popover 6 más su contenido y la superposición de ocupado 3.

## Botones desplegables y divididos

`MenuButton::new(label)` abre un menú de comandos; `MenuButton::split(label, action, input)` ejecuta una acción desde su parte principal y abre el menú desde su flecha. Los comandos son `.action(label, action, input)` (o `.action_then(...)` con effects tras el éxito), `.item(label, effect)` para effects como abrir una ventana y `.separator()`. `.confirm(text)` pregunta antes de que se ejecute el último comando de acción.

```rust
MenuButton::split(t!("orders.save"), save_draft, id)
    .action(t!("orders.save_and_send"), save_and_send, id)
    .separator()
    .action(t!("orders.delete"), delete_order, id)
    .confirm(t!("orders.delete_confirm"))
```

Los comandos de acción son nodos con su propia ruta y se envían al endpoint de formularios como un `ActionButton`: el acceso de la pantalla, los guards de la feature, el permiso de la acción, los interceptores, CSRF y el límite de frecuencia de escritura se comprueban en cada llamada, y la entrada se queda en el servidor. Los comandos que el usuario no puede ejecutar se omiten del menú (en un botón dividido, en cambio, se deshabilita la acción principal), y el servidor los rechaza igualmente si se llaman.

@live menus

| Tecla | Dónde | Qué hace |
|---|---|---|
| Enter, Espacio, Flecha abajo | el botón | Abre el menú en el primer comando |
| Flecha arriba | el botón | Abre el menú en el último comando |
| Flecha abajo, Flecha arriba | el menú | Comando siguiente y anterior, de forma circular |
| Inicio, Fin | el menú | Primer y último comando |
| Letras | el menú | El siguiente comando que empieza por ellas |
| Enter, Espacio | un comando | Lo ejecuta y cierra el menú |
| Escape | el menú | Lo cierra; el foco vuelve al botón |
| Tab | el menú | Lo cierra y avanza |

El menú necesita unos 0,8 KB de script con gzip y ninguna petición hasta que se ejecuta un comando; son 4 nodos más 2 por comando.

## DocumentViewer

`DocumentViewer::of(page_action, page_count)` muestra un documento paginado, como la vista previa de un informe, cuyas páginas se dibujan en el servidor. La action de página recibe un `DocPage` (`document` de `.document(id)`, nunca del navegador, y `page`) y devuelve una página como fragmento SVG o HTML. Cada página tiene el tamaño de `.page_size(..)` (`PageSize::A4`, `PageSize::LETTER`, `PageSize::pt(w, h)` o `PageSize::px(w, h)`), así que el área de desplazamiento mide desde el principio lo mismo que todo el documento.

```rust
DocumentViewer::of(report_page, report.pages)
    .document(report.id.to_string())
    .page_size(PageSize::A4)
    .outline(vec![OutlineItem::new("Summary", 1).child(OutlineItem::new("Totals", 2))])
    .search(search_report)
    .download("PDF", format!("/reports/{}/pdf", report.id))
```

- En la página solo están las páginas visibles y una a cada lado. Se piden por `POST /_zeyn/data` cuando el desplazamiento se detiene (las páginas que se pasan rápido nunca se piden), la más cercana primero, como mucho 3 a la vez, y se cancela la petición de una página que ha salido de la vista. El navegador guarda como mucho 12 páginas dibujadas.
- El zoom escala las páginas sin volver a pedirlas: ajustar al ancho, ajustar a la página y del 50 al 400 por ciento. El cuadro de página lleva a una página, y la página actual se anuncia ("Página 3 de 500") cuando se detiene el desplazamiento.
- `.outline(items)` añade un panel de índice, `.search(action)` un cuadro de búsqueda cuya action recibe un `DocSearch` y devuelve `PageHit` (se muestran como mucho 50), y `.download(label, href)` un enlace de descarga.
- El servidor comprueba en cada página el acceso a la pantalla, el permiso de la action de página, CSRF y el límite de escritura, rechaza números de página fuera de 1 al número de páginas y páginas de más de 2 MB (`MAX_PAGE_BYTES`), y limpia cada página con una lista de permitidos (`sanitize_markup`): se quitan scripts, manejadores de eventos, `foreignObject`, marcos, enlaces e imágenes externos y `url()` en estilos, y los id reciben un prefijo por página. `.trusted()` omite la limpieza para páginas que la app dibuja con datos que controla; el texto de un usuario que llega sin escapar a una página así se ejecuta como script para todos los que la abren.
- `document.js` (13 KB, 4,7 KB con gzip) y `document.css` (3,6 KB) se cargan solo en páginas con un visor.

| Tecla | Dónde | Qué hace |
|---|---|---|
| Av Pág, Re Pág | las páginas | Página siguiente y anterior |
| Inicio, Fin | las páginas | Primera y última página |
| `+`, `-` | las páginas | Acerca y aleja |
| Enter | el cuadro de página | Va a la página escrita |
