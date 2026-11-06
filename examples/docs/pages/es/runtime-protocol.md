# Protocolo del runtime

Estado: borrador, implementado por `zeyn::server` y `assets/zeyn/*.js` (servido como `/_zeyn/zeyn.js`).

Este es el contrato entre el servidor de Zeyn y el runtime del navegador. Las pantallas se renderizan en el servidor. El runtime del navegador es un script pequeño que envía las acciones del usuario al servidor y aplica lo que vuelve. El código de la aplicación nunca escribe JavaScript.

## 1. Carga de página

`GET {route}?params={json}` devuelve un documento HTML completo. `params` es el JSON de los parámetros de la pantalla codificado como URL y se omite cuando la pantalla no tiene ninguno.

Una ruta con segmentos de path, como `/orders/{id}`, lee esos campos de params desde el path (`/orders/42`) y los demás campos desde `params`. Un valor de segmento que no se puede interpretar responde 404. Los valores se codifican con porcentaje, incluidos los puntos (`%2E`), de modo que la URL de una pantalla nunca parece un archivo para el runtime. Una pantalla que responde `Screen::moved_to` recibe `301 Moved Permanently` con la nueva URL en `Location`; una que responde `Screen::redirect` recibe `303 See Other` con `Cache-Control: no-store`. `nav.js` sigue ambas sin cargar la página y muestra la dirección del destino, también cuando la carga no añade una entrada al historial (atrás y adelante).

Una página fallida responde con su estado (404, 403, 500, ...) y la pantalla de error de la aplicación (`App::not_found`, `App::server_error`) o un mensaje breve integrado, dentro del shell. Una petición con `x-zeyn-partial: 1` recibe solo esa pantalla con `x-zeyn-partial: 1` y `x-zeyn-title`, de modo que el runtime la intercambia sin cargar la página. Los detalles de los errores nunca llegan al navegador.

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="zeyn-csrf" content="TOKEN">
  <title>Products · Inventory</title>
  <link rel="stylesheet" href="/_zeyn/zeyn.css?v=BUILD">
  <script type="module" src="/_zeyn/zeyn.js?v=BUILD"></script>
</head>
<body>
  <a class="z-skip" href="#z-main">Skip to content</a>
  <div class="z-app">
    <nav class="z-nav" aria-label="Main">...</nav>
    <main class="z-main" id="z-main">SCREEN</main>
  </div>
  <div class="z-toasts" role="status" aria-live="polite"></div>
</body>
</html>
```

El menú enumera solo las pantallas que el usuario actual puede abrir. El elemento actual tiene `aria-current="page"`.

Cada script y hoja de estilos del framework en el head lleva `?v=` y el id de compilación (`meta[name=zeyn-build]`, de `ZEYN_BUILD_ID` o un hash del ejecutable); `theme.css` lleva un hash del texto del tema. Una petición con el valor actual recibe `Cache-Control: public, max-age=31536000, immutable`, así que una carga de página en caliente no pide ninguno de ellos; cualquier otra petición (una pestaña antigua tras un despliegue, sin `v`) recibe `no-cache` y el ETag. Los módulos que el runtime importa más tarde toman el mismo `v` de la URL del script que los importa.

### 1.1 Browser modules

Una página carga y ejecuta solo el código de navegador que usa su pantalla. Todas las páginas ejecutan el núcleo: `zeyn.js` (peticiones, efectos, ventanas, formularios, menús, atajos, la delegación de eventos y el cargador de módulos), `nav.js` y `enterprise.js` (ganchos de respuesta sobre `fetch` y el tiempo de inactividad de la sesión), 90 KB sin comprimir y 29 KB con gzip en total. Todo lo demás es un módulo de un componente, `/_zeyn/{name}.js` y en algunos casos `/_zeyn/{name}.css`, declarado en `MODULES` de `crates/zeyn/src/server/modules.rs` con las marcas que lleva su HTML: nombres de atributos (`data-zeyn-date`) y clases (`z-select`), comparados como nombres completos.

| Módulo | Componentes |
|---|---|
| `components.js` | la parte compartida que importa cada módulo de controles: utilidades, los núcleos de ventanas emergentes y listas, los selectores (los de la estructura de la aplicación), los diálogos de confirmación, las alertas y las migas de pan |
| `tabs.js`, `dates.js`, `choices.js`, `inputs.js`, `lines.js`, `steps.js`, `tree.js`, `overlays.js`, `menubutton.js`, `busy.js` | paneles, pestañas y paneles divididos; selectores de fecha y hora (también los filtros de fecha de los grids); listas, búsquedas y selecciones múltiples; números, áreas de texto que crecen, máscaras y contraseñas; el editor de líneas; formularios por pasos; árboles; tooltips y popovers; botones de menú y botones divididos; capas de espera |
| `grid.js` | grids, enlazado como `<link rel="modulepreload">` porque `zeyn.js` lo importa por sí mismo |
| `palette.js`, `pro.js` | la paleta de comandos y los botones de comando; barras de menú, cajas de herramientas, barras de estado, paneles acoplables e inspectores de propiedades |
| `forms.js`, `reveal.js`, `import.js` | borradores y transiciones de flujo de los formularios; mostrar valores enmascarados; importaciones |
| `catalog.js`, `media.js`, `files.js`, `charts.js` y sus hojas de estilo | cuadrículas de tarjetas, carriles y cajas de búsqueda; reproductores, contenido incrustado e imágenes; subidas; gráficos, minigráficos, tarjetas KPI y gráficos de bala |
| `share.js`, `chips.js`, `reactions.js`, `hero.js`, `state.js`, `preview.js`, `assistant.js` | botones de compartir, chips de filtro, reacciones, diapositivas hero, el almacén de preferencias, vistas previas de tarjetas (primer paso del puntero), el asistente (paneles de chat o la primera pulsación del lanzador) |
| `topics.js`, `vlist.js` | nodos que escuchan topics y los controles que los conducen (`data-zeyn-listens`, `data-zeyn-drives`); listas virtuales (`data-zeyn-vlist`) |

Una página completa enlaza en su head las hojas de estilo que usan todas las páginas (`zeyn.css`, `components.css`, `content.css`, `pro.css`, `enterprise.css`), las hojas de estilo de los módulos cuyas marcas están en su HTML y sus scripts después de los módulos que importan. `zeyn.js` se sirve con la misma tabla (`[name, script selector, style selector]`) y carga lo que falta cuando llega contenido después: `nav.js` lanza `zeyn:need` con `{ root, waits }` antes de colocar una pantalla y espera las hojas de estilo nuevas, las ventanas esperan igual antes de abrirse, y `zeyn:content`, las páginas de grid y los filtros de grid cargan los scripts de lo que contienen. El navegador guarda una instancia por URL, así que un módulo se ejecuta una vez. Un módulo prepara lo que encuentra en el documento al ejecutarse y después escucha `zeyn:content`; nunca busca ni escucha componentes que no están en la página. Los módulos importan la parte compartida con el `?v=` de su propia URL. `tests/ui_modules.rs` falla cuando una página enlaza un módulo que no usa o cuando la hoja de estilo de un módulo da estilo a una clase fuera de sus marcas.

Medido en el showcase (sin comprimir, en la red con gzip): una página solo con texto carga 132 KB de scripts (46 KB) en lugar de 269 KB (80 KB), una página con grid 223 KB (74 KB) en lugar de 357 KB (106 KB), una página con formulario 186 KB (65 KB) en lugar de 269 KB (80 KB), y todas las páginas 151 KB de estilos (35 KB) en lugar de 177 KB (44 KB). Una página que contiene todos los componentes, como el catálogo de componentes, carga más archivos que antes (68 peticiones en lugar de 41, 836 KB en lugar de 774 KB sin comprimir), porque cada módulo es un archivo propio.

## 2. Marcado de la pantalla

La raíz de cada pantalla lleva su nombre y sus parámetros:

```html
<section class="z-screen" data-zeyn-screen="product_list" data-zeyn-params="null" aria-labelledby="product_list-title">
```

Las ventanas usan `class="z-window" role="dialog" aria-modal="true"` y un estilo `--z-width`.

Cada elemento del cuerpo se envuelve en un elemento nodo. Su path es la dirección que usan los endpoints de más abajo. Los contenedores (`Row`, `Stack`, `Panel`, `Card`, `Tabs`, `Split`) envuelven a cada hijo del mismo modo, y el path del hijo añade su índice: el tercer hijo del primer hijo del elemento 1 del cuerpo es `"1.0.2"`.

```html
<div class="z-node" data-zeyn-node="1">
  <div class="z-node" data-zeyn-node="1.0">...</div>
</div>
```

Los endpoints reciben `node` como una cadena con puntos o, para los elementos del cuerpo, como un número simple (`0`). Los paths tienen como máximo 16 niveles de dígitos; cualquier otra cosa se rechaza con `422`.

## 3. Atributos que lee el runtime

| Atributo | En | Significado |
|---|---|---|
| `data-zeyn-effect` | button, `tr` | Effect en JSON que se ejecuta al hacer clic (filas: con doble clic o Enter) |
| `aria-keyshortcuts` | button | Atajo de teclado, por ejemplo `Control+N` |
| `data-zeyn-export` | button | Formato de exportación del grid de la misma pantalla (`csv`, `xlsx`) |
| `data-zeyn-grid` | raíz del grid | Marca un grid; también tiene `data-zeyn-mode`, `data-zeyn-paging`, `data-zeyn-page-size`, `data-zeyn-total`, `data-zeyn-next`, `data-zeyn-offset` |
| `data-field`, `aria-sort`, `aria-colindex` | `th` | Nombre del campo de la columna, ordenación actual, posición |
| `data-filter`, `data-options` | `th` | Tipo de entrada del filtro (`text`, `number`, `money`, `date`, `datetime`, `time`, `choice`, `bool`) y opciones de choice `[{value,label}]` |
| `data-pin` | `th` | `start` o `end` para las columnas fijadas (también las columnas de selección y de acciones de fila) |
| `data-edit`, `data-optional` | `th` | Tipo de entrada de una columna editable, para los usuarios que pueden guardar |
| `data-zeyn-texts`, `data-zeyn-columns`, `data-zeyn-view` | raíz del grid | Textos de los controles localizados, todas las columnas `[{field,label,hidden,pin,width}]`, la vista guardada |
| `data-zeyn-views` | raíz del grid | `server` cuando las vistas se guardan en el servidor |
| `data-zeyn-version-field`, `data-zeyn-exportable` | raíz del grid | Nombre del campo de versión de fila para las ediciones en línea; `csv` cuando el grid puede exportarse |
| `data-zeyn-key`, `data-zeyn-version`, `data-zeyn-menu` | `tr` | Clave de la fila, versión de la fila, elementos del menú contextual |
| `data-v` | `td` | Valor de entrada de una celda editable |
| `data-zeyn-detail`, `aria-expanded` | `tr` | Pantalla de detalle de la fila como `{screen, params}`; si está abierta |
| `data-zeyn-expand` | button en una fila | Abre o cierra el detalle, el grupo o los hijos de la fila |
| `data-zeyn-level`, `aria-level`, `data-zeyn-path` | `tr` en un `treegrid` | Profundidad (0 en lo más alto) en un grid agrupado o de árbol; el path de valores de grupo o de claves de ancestros que se envía al abrir la fila |
| `z-group` | `tr` | Una fila de encabezado de grupo; `z-branch-more` es una fila "Cargar más" de una rama |
| `data-zeyn-run` | button en una fila | Elemento de fila que se ejecuta (`a0` es la primera acción de fila) |
| `data-agg` | `tfoot td` | Totales que muestra la columna (`sum avg min max count`) |
| `data-zeyn-bulk` | div antes del grid | Barra de selección con botones `data-zeyn-bulk-run` |
| `data-zeyn-menu` | raíz de la pantalla | Menú contextual de la pantalla |
| `data-zeyn-native-menu="off"` | `html` | El menú contextual del navegador está desactivado salvo en las entradas de texto |
| `data-zeyn-rows` | `tbody` | Dónde van las filas |
| `data-zeyn-page` | botón del paginador | `prev` o `next` |
| `data-zeyn-page-info` | span | Texto como `1-50 of 230`, construido a partir de las plantillas localizadas de la raíz del grid |
| `data-zeyn-text-range`, `data-zeyn-text-range-total`, `data-zeyn-text-loaded` | raíz del grid | Plantillas localizadas: `{from}-{to}`, `{from}-{to} of {total}`, `{count} rows loaded` |
| `data-zeyn-more` | div | Centinela del scroll infinito, contiene el botón "Cargar más" |
| `data-zeyn-submit` | form | Nombre de la action de envío |
| `data-zeyn-editor` | input | Editor del lado del cliente de un tipo de campo personalizado |

Effects:

| Effect | JSON | El runtime hace |
|---|---|---|
| Abrir ventana | `{"effect":"open_window","screen":"product_form","params":{...}}` | Carga la pantalla (sección 4.1) en un `<dialog>` modal |
| Navegar | `{"effect":"navigate","screen":"...","params":{...}}` | Carga `/_zeyn/route/{screen}?params=`, que redirige a la página, en la forma con path cuando su ruta tiene segmentos |
| Toast | `{"effect":"toast","message":"Saved"}` | Muestra un toast durante 4 segundos |
| Cerrar y refrescar | `{"effect":"close_and_refresh"}` | Cierra el diálogo superior y recarga todos los grids de la pantalla que está debajo |

## 4. Endpoints

Todas las peticiones `POST` envían JSON, deben llevar la cabecera `x-zeyn-csrf` con el token de la etiqueta meta `zeyn-csrf`, y en caso contrario se rechazan con `403`.

Una pantalla pública abierta sin sesión llega con una etiqueta meta `zeyn-csrf` vacía y sin `Set-Cookie`, de modo que pueda almacenarse en caché. Antes de su primer `POST` el runtime llama a `GET /_zeyn/csrf`, que responde `{ "token": "..." }` con `Cache-Control: no-store`, establece la cookie `zeyn_csrf` cuando el navegador no tiene ninguna y rechaza las peticiones cuyo `Sec-Fetch-Site` no sea `same-origin`. El token pasa a la etiqueta meta; cuando la llamada falla, el `POST` sale sin token y se rechaza como cualquier otro. No se reintenta nada en silencio.

La navegación dentro de la aplicación (`nav.js`) pide una pantalla con `x-zeyn-partial: 1` y recibe solo su HTML, con el título de la página en `x-zeyn-title`. Cuando la pantalla define etiquetas de head (descripción, URL canónica, imagen de vista previa, robots, JSON-LD), el cuerpo empieza con un `<template data-zeyn-head>` que las contiene; `nav.js` elimina cada elemento `[data-zeyn-head]` del head del documento, mueve allí las etiquetas del template y descarta el template. Una pantalla sin etiquetas de head no envía template, así que las etiquetas antiguas solo se eliminan.

### 4.1 Fragmento de pantalla

`GET /_zeyn/screen/{screen}?params={json}` devuelve solo el HTML de la pantalla, sin el shell de la página. Se usa para ventanas y para detalles de fila: abrir una fila con `data-zeyn-detail` carga su pantalla en un nuevo `tr.z-detail` justo debajo (una celda que ocupa toda la fila, con un `role="region"`), y cerrarla elimina esa fila. El permiso de la pantalla se comprueba como en cualquier pantalla; los grids dentro del detalle hablan con el servidor usando el nombre y los parámetros propios de la pantalla de detalle.

### 4.2 Filas del grid

`POST /_zeyn/grid`

```json
{
  "screen": "product_list",
  "params": null,
  "node": 0,
  "query": {
    "page": { "type": "offset", "offset": 0, "limit": 50 },
    "sort": [{ "field": "code", "desc": false }],
    "filters": [{ "field": "name", "contains": "tea" }, { "field": "price", "from": "10", "to": "99.90" }],
    "with_total": true,
    "aggregates": [{ "field": "price", "op": "sum" }]
  },
  "view": { "columns": [{ "field": "code", "width": 140, "pin": "start" }, { "field": "name" }] },
  "head": false
}
```

Los filtros pueden establecer `contains`, `starts_with`, `equals`, `from`, `to`, `any_of` e `is` (véase `docs/grid.md`). `view.columns` es la disposición de columnas con la que se renderizan las filas; con `head: true` la respuesta también incluye `head`, la fila de encabezado.

Para el scroll infinito `page` es `{ "type": "after", "cursor": "...", "limit": 100 }` y `cursor` es `null` en el primer lote.

Respuesta `200`:

```json
{ "rows": "<tr>...</tr>", "foot": "<tr>...</tr>", "head": null, "count": 50, "total": 230, "next": "50" }
```

`foot` es la fila de totales cuando se pidieron totales, y `null` en caso contrario.

`total` y `next` pueden ser `null`. El runtime reemplaza las filas en los grids paginados y las añade al final en el scroll infinito.

Los grids agrupados y de árbol (`<table role="treegrid">`) envían `path` con `POST /_zeyn/grid` para cargar una rama: los valores de grupo (o las claves de ancestros) de la fila que se abre, tal como aparecen en su `data-zeyn-path`. La respuesta tiene la misma forma; `rows` contiene filas de grupo o filas un nivel más profundas, y `total` las cuenta. El runtime las coloca después de la fila abierta y elimina cada fila más profunda que la sigue cuando la fila se cierra. Un path más largo que los niveles de grupo del grid (o 32 en un árbol) da `404`.

### 4.2.1 Más endpoints del grid

| Endpoint | Cuerpo | Hace |
|---|---|---|
| `POST /_zeyn/grid/view` | `{screen, params, node, view}` | Guarda la vista del usuario (`null` la elimina). `204`; `404` sin un almacén de vistas |
| `POST /_zeyn/grid/run` | `{screen, params, node, key, item}` | Ejecuta un elemento de fila (`a0` acción de fila, `m2.1` path de menú). El servidor carga la fila por clave y construye la entrada. `{effects}` |
| `POST /_zeyn/grid/bulk` | `{screen, params, node, action, keys, all, query}` | Ejecuta una acción masiva con las claves elegidas, o con todas las filas que coinciden con `query.filters` cuando `all`. `{effects, count}` |
| `POST /_zeyn/grid/edit` | `{screen, params, node, key, values, view}` | Guarda una edición en línea; `values` contiene las columnas editadas y la versión de la fila tal como las envía un formulario. `{row}`; `403` para columnas que no son editables, `409`, `422` |
| `GET /_zeyn/grid/export` | `?screen&params&node&q={sort,filters,columns}` | Transmite por streaming todas las filas coincidentes como CSV |
| `POST /_zeyn/menu` | `{screen, params, item}` | Ejecuta un elemento de acción del menú contextual de la pantalla |

### 4.2.2 Scroll virtual

Los grids con `data-zeyn-paging="virtual"` muestran una única lista desplazable sobre todas las filas, sean cuantas sean. El servidor renderiza el primer bloque (`data-zeyn-page-size` filas, offset 0) con `data-zeyn-total`: las primeras 64 filas viven en la tabla y el resto en `<template data-zeyn-rows-later>` después de ella, o todas las filas en la plantilla cuando el grid muestra 40 columnas o más. La raíz del grid también tiene `data-zeyn-row-height` (píxeles) y la variable CSS `--z-row-height`.

El runtime:

- Coloca el cuerpo de la tabla en un contenedor con scroll cuyo espaciador interno mide `total * rowHeight` píxeles de alto, con un tope de 8 000 000 px. Por encima del tope, la posición del scroll se asigna al índice de fila por proporción (`row = scrollTop / (spacerHeight - viewport) * (total - visibleRows)`), de modo que 1 000 000 de filas aún llegan hasta el final.
- Mantiene en el DOM solo las filas visibles más 20 filas por encima y por debajo, posicionadas con `transform: translateY(...)`. Las filas tienen una altura fija; el texto largo se corta con puntos suspensivos.
- Carga bloques de `data-zeyn-page-size` filas con `POST /_zeyn/grid` y `page: { "type": "offset", "offset": blockStart, "limit": blockSize }`, con `with_total: false` después de la primera llamada. Mantiene hasta 20 bloques y como máximo 100.000 celdas en una caché (sale el menos usado recientemente; 7 bloques para un grid de 63 columnas, nunca menos de 3), y los bloques guardados para un `reload` cuentan en el mismo tope y salen primero, precarga el siguiente bloque en la dirección del scroll y cancela las peticiones de los bloques que salieron de la vista (`AbortController`).
- Nunca borra las filas visibles mientras se carga un bloque. Las filas ausentes muestran una fila marcadora con `aria-busy="true"` hasta que llega su bloque, de modo que la pantalla no parpadea.
- Los cambios de ordenación y de filtro descartan la caché y vuelven al inicio.
- Teclado: las teclas de flecha, Page Up, Page Down, Home y End mueven la fila activa y la desplazan hasta dejarla a la vista. La tabla tiene `aria-rowcount` = total y cada fila `aria-rowindex`.
- El texto de posición (`data-zeyn-page-info`) usa `data-zeyn-text-range-total`.

### 4.3 Envío de formulario

`POST /_zeyn/form`

```json
{ "screen": "product_form", "params": { "id": 7 }, "node": 0, "values": { "code": "P-001", "price": "1.234,56", "active": true } }
```

Los valores se envían tal como los escribió el usuario: cadenas para las entradas y los selects, booleanos para las casillas. El servidor los interpreta según el tipo del campo.

El servidor parte del modelo que cargó la pantalla y sobrescribe solo los campos que están en el formulario y no son de solo lectura. Los demás valores del cliente se ignoran. Las claves nunca provienen del cliente.

Respuestas:

- `200 { "effects": [ ... ] }`: guardado; ejecuta los effects en orden.
- `422 { "errors": [ { "field": "code", "rule": { "rule": "required" }, "message": "Code is required." } ] }`: muestra cada mensaje bajo su campo (`#{scope}-{field}-error`), establece `aria-invalid="true"` en la entrada y pone el foco en la primera entrada no válida. Los errores con `field` vacío se muestran en la parte superior del formulario.

### 4.4 Errores

Cualquier endpoint puede responder con:

| Estado | Cuerpo | Significado |
|---|---|---|
| `403` | `{ "error": "forbidden", "message": "..." }` | Permiso ausente, guard denegado o token CSRF incorrecto |
| `404` | `{ "error": "not_found", "message": "..." }` | Pantalla o nodo desconocido |
| `409` | `{ "error": "conflict", "message": "..." }` | Otra persona cambió el registro |
| `422` | véase 4.3 | Validación |
| `504` | `{ "error": "timeout", "message": "..." }` | La llamada tardó más que el límite del servidor |
| `500` | `{ "error": "internal", "message": "An unexpected error occurred." }` | Los detalles solo están en el log del servidor |

El runtime muestra el `message` como un toast de error.

### 4.5 Idioma

`POST /_zeyn/locale` con `{ "locale": "tr" }` guarda la elección en la cookie `zeyn_locale` y responde `204`. El runtime recarga la página. Sin cookie, el servidor elige a partir de `Accept-Language` y después el valor predeterminado de la aplicación. `<html lang>` siempre coincide con el idioma elegido.

El shell renderiza un selector de idioma (`select[data-zeyn-locale]`) cuando la aplicación tiene más de un idioma.

### 4.6 Tema

El shell renderiza un selector de tema (`select[data-zeyn-theme-picker]`) salvo que la aplicación lo oculte u ofrezca un único tema. Cada opción lleva `data-scheme` (`light`, `dark`, vacío para System) y los temas de la aplicación `data-custom`. Al cambiar, el runtime establece de inmediato `data-theme` y `data-zeyn-theme` en `<html>` y la meta `color-scheme`, y luego envía `POST /_zeyn/theme` con `{ "theme": "dark" }`. El servidor responde `204` con la cookie `zeyn_theme`, o `404` para un tema que la aplicación no ofrece. Las páginas se renderizan con el tema de esa cookie, así que no hay recarga ni parpadeo. Véase `docs/theming.md`.

### 4.7 Endpoints empresariales

`enterprise.js` y `enterprise.css` se cargan junto al runtime. Los detalles están en `docs/enterprise.md`.

| Endpoint | Propósito |
| --- | --- |
| `POST /_zeyn/reveal {screen, params, node, field, key?}` | Valor sin procesar de un campo enmascarado, `{value}`; requiere el permiso de desenmascarar, siempre auditado |
| `POST /_zeyn/reason {screen, reason}` | Motivo para una pantalla marcada con `reason`; `204` con una cookie `HttpOnly` |
| `POST /_zeyn/import {screen, params, node, csv, commit}` | Vista previa del CSV (`commit: false`) o importación |
| `POST /_zeyn/keepalive` | "Seguir conectado" tras el aviso de inactividad; `204` |
| `GET /_zeyn/health`, `GET /_zeyn/ready` | Liveness y readiness; health no tiene límite de frecuencia, readiness solo desde `probe_ips` |
| `GET /_zeyn/jobs` | Estado de los jobs para los usuarios con el permiso de jobs |

Cada respuesta lleva `x-zeyn-build`. Rechazos a los que el runtime reacciona: `428 reason_required` (pide un
motivo y repite la petición) y `401 reauth_required` (pide al usuario que inicie sesión de nuevo y repite
la petición).

## 5. Seguridad

- Cada respuesta establece `Content-Security-Policy: default-src 'self'; script-src 'self'; style-src 'self'; style-src-attr 'unsafe-inline'; img-src 'self' data: blob:; object-src 'none'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'`, `X-Content-Type-Options: nosniff` y `Referrer-Policy: same-origin`. `App::csp` añade hosts `https` con nombre a `img-src`, `media-src` (y `blob:`), `frame-src`, `connect-src` y `font-src`; las demás directivas nunca cambian. El texto de la cabecera se construye una sola vez al iniciar.
- Sin scripts en línea ni manejadores de eventos en línea. Los atributos `style` en línea se usan solo para propiedades personalizadas de CSS (`--z-width`, `--z-flex`) y anchos de columna fijos, por lo que la política añade `style-src-attr 'unsafe-inline'`. Los scripts siguen siendo estrictos.
- El token CSRF se guarda en una cookie `HttpOnly`, `SameSite=Strict` y se compara en tiempo constante con la cabecera `x-zeyn-csrf`.
- Las páginas, el formulario `totp` y `GET /_zeyn/csrf` nunca llevan el token tal como está en la cookie: cada respuesta recibe un relleno aleatorio nuevo seguido del token XOR el relleno (128 dígitos hexadecimales), para que gzip no pueda filtrarlo (BREACH). El servidor lo desenmascara antes de comparar y sigue aceptando el token simple de 64 dígitos.
- Las respuestas dinámicas de 1 KB o más (páginas, pantallas parciales, bloques de grid, exportaciones) se comprimen con gzip en el nivel 2 cuando el navegador lo acepta, con `Vary: Accept-Encoding`. Nunca se comprimen: las respuestas que ya tienen un `Content-Encoding` (los recursos del framework, comprimidos una vez y guardados en caché), los rangos (`Content-Range` y cualquier petición con `Range`), vídeo, audio, imágenes distintas de SVG, `application/octet-stream`, archivos comprimidos y PDF, `text/event-stream`, `application/x-ndjson` (el flujo del asistente) y las actualizaciones a WebSocket.
- Cada llamada comprueba, por orden: la autenticación, el acceso de la pantalla, los guards de la feature, el acceso de la action del nodo, y luego ejecuta los interceptores y la action.

## 6. Comportamiento del runtime

- Sin JavaScript, las páginas siguen renderizándose con la primera página de cada grid. Los formularios y la paginación necesitan el runtime.
- Teclado: las teclas de flecha se mueven entre las filas del grid, Enter abre una fila, Escape cierra el diálogo superior, y los atajos de `aria-keyshortcuts` funcionan en todas partes salvo dentro de las entradas de texto. En los grids: Space selecciona la fila, F2 la edita, Ctrl+Z deshace la última edición, Shift+F10 o la tecla de menú contextual abre el menú de la fila o del encabezado, Shift+clic ordena por varias columnas, Derecha e Izquierda (invertidas en páginas de derecha a izquierda) abren y cierran el detalle, el grupo o los hijos de una fila, e Izquierda en una fila hija va a su padre. Derecha en una fila entra en sus celdas: las flechas, Home, End, Ctrl+Home, Ctrl+End, Tab y Shift+Tab mueven el cursor de celda, Shift amplía un rango (`aria-selected` en las celdas), Ctrl+C copia el rango como texto separado por tabuladores y Ctrl+V pega en las celdas editables, un `POST /_zeyn/grid/edit` por fila modificada. Los grids con 40 o más columnas mantienen en la página solo las columnas a la vista (un `colgroup` con anchos fijos, espaciadores `td.z-col-gap` con `colspan`, `aria-colindex` en las celdas).
- El clic derecho abre el menú de Zeyn donde haya uno definido (encabezados, filas con `data-zeyn-menu`, pantallas con `data-zeyn-menu`); en cualquier otro lugar se mantiene el menú propio del navegador.
- El foco se mueve a un diálogo cuando se abre y vuelve a quien lo abrió cuando se cierra.
- Estado pendiente en el mismo fotograma del clic: `nav.js` marca el enlace de destino como actual y pone `html[data-zeyn-loading]` mientras carga una pantalla, las ventanas y los envíos de formularios también lo ponen, y los grids atenúan sus filas (`aria-busy`) en cuanto se pulsa una ordenación, una página o una tecla de filtro (los filtros esperan 150 ms por más teclas y una petición nueva cancela la anterior). Una barra fina en la parte superior de la ventana aparece solo cuando la respuesta tarda más de 150 ms; es fija, así que nada se mueve, y no se anima con `prefers-reduced-motion`.
- Las comprobaciones del lado del cliente (`required`, `minlength`, `maxlength`, `min`, `max`, `type="email"`) se ejecutan antes del envío con la API de validación de restricciones del navegador. El servidor siempre vuelve a validar.

## 7. Biblioteca de componentes

Los componentes de `zeyn::ui` se sirven con los módulos de la sección 1.1 y la hoja de estilo `/_zeyn/components.css`. Como el runtime, no necesitan inicio de sesión.

### 7.1 Endpoints

Todos son `POST` con JSON y la cabecera CSRF, y comprueban primero el acceso de la pantalla y los guards de la feature.

| Endpoint | Cuerpo | Respuesta |
|---|---|---|
| `/_zeyn/node` | `{ screen, params, node }` | HTML de un nodo con la primera página de sus grids. Lo usan las pestañas diferidas. |
| `/_zeyn/nodes` | `{ screen, params, nodes: ["1.0", "2"] }` | `{ nodes: { "1.0": "<div class=\"z-node\" ...>" } }`: varios nodos renderizados en una sola ejecución de la pantalla, para los listeners de un topic refrescado o conducido. Como máximo 32 rutas distintas; una ruta que no está en la pantalla responde `404`. El acceso, los guards y los interceptors de la pantalla se ejecutan una vez, y los params se leen como los params tipados de la pantalla. |
| `/_zeyn/data` | `{ screen, params, node, key, input }` | JSON de la fuente de datos `key` del nodo: opciones de lookup (`key` es el nombre del campo), hijos de un árbol (`key` está vacío), comprobaciones de paso (`key` es `_step`). Se comprueba el permiso de la action de la fuente y se ejecutan los interceptores, como en los grids. La entrada que no encaja con la action se rechaza con `404`. |
| `/_zeyn/data` con `screen` = `_site.{feature}` | como arriba | Lo mismo para un componente en una ranura de la cabecera del sitio: `node` es la ranura (0 start, 1 center, 2 end, 3 bottom), el índice del nodo en ella y después los índices de los hijos. Se comprueban los guards de la feature y el permiso de la action. |
| `/_zeyn/parse` | `{ kind, text, label }` | Cómo lee el servidor una fecha u hora escrita, según el idioma, la zona y el reloj del usuario: `{ ok, text, iso, utc, zone }` o `{ ok: false, message }`. `kind` es `date`, `time`, `datetime`, `local` o `presets` (hoy y rangos rápidos). |

### 7.2 Valores que envían los formularios

- Los lookups envían los ids como texto: `"7"`, o `"3,7,9"` para varios.
- Las líneas envían `"{field}": "rows"` y un valor por celda como `"{field}.{line}.{column}"`, más `"{field}.{line}._i"`, el índice de la línea cargada de la que provino la fila (vacío para las líneas nuevas). El servidor reconstruye cada línea a partir de su línea cargada, de modo que solo cambian las columnas mostradas y editables.
- Los errores de validación de una línea usan `{ "field": "lines", "rule": { "rule": "line", "value": { "row": 0, "field": "quantity" } } }`.

### 7.3 Eventos

| Evento | En | Detalle |
|---|---|---|
| `zeyn:content` | `document` | Elemento con contenido nuevo (navegación, paneles diferidos). El runtime carga los módulos que necesita y cada módulo cargado prepara lo que contiene. |
| `zeyn:errors` | el formulario | Los errores `422`, después de que el runtime los mostró. Lo usan el editor de líneas y los formularios por pasos. |
| `zeyn:effect` | `document` | Un effect que el runtime no conoce: `{ effect, source, run }`. `run(next)` ejecuta otro effect. Lo usa `confirm`. |
| `zeyn:topics` | `document` | `{ topics }`: topics que refrescar, desde otro script (`live.js` al recibir una notificación). |
| `zeyn:reload` | un elemento | Recarga en su sitio los grids que contiene: la página actual, o la primera con `{ reset: true }`. |

El effect `{"effect":"confirm","message":"...","title":"...","ok":"...","cancel":"...","then":{...}}` pregunta en un cuadro de diálogo modal de alerta y ejecuta `then` solo después de que el usuario confirma.

El effect `{"effect":"refresh","topics":["sales"]}` refresca los listeners de los topics. `[data-zeyn-listens="a,b"]` marca un listener (el nodo de su padre `data-zeyn-node`); `[data-zeyn-drives="a"]` marca un control que conduce topics: un formulario de filtro (`data-zeyn-kinds` dice qué params son números o booleanos), un search box (su campo se llama `q`) o chips de filtro (cada chip lleva sus params en `data-zeyn-params`). Conducir fusiona los valores del control en el `data-zeyn-params` de la raíz de la pantalla, reemplaza la dirección con ellos cuando la pantalla es la principal y refresca los listeners. `topics.js` procesa a la vez los topics nombrados en un frame de animación: un listener que contiene un grid recibe `zeyn:reload`, los demás de una pantalla van en un solo `POST /_zeyn/nodes`, y uno más nuevo para la misma pantalla aborta el anterior y se queda con sus nodos. El foco y los `<details>` abiertos se conservan por id, test id, fila `data-zeyn-key` y posición.

Una lista virtual es `section[data-zeyn-vlist]` con `data-zeyn-row-height`, `data-zeyn-block` y `data-zeyn-total`; `vlist.js` pide bloques a `POST /_zeyn/grid` con `{ page: { type: "offset", offset }, with_total: true }` y coloca cada `<li>` en `(aria-posinset - 1) * height`. Los lookups que dependen de otro campo llevan `data-depends-on="field"` y envían el valor de ese campo como `parent` en su entrada de `/_zeyn/data`.

Las preferencias del cliente (`zeyn::ClientPref`) viven en el módulo del navegador `/_zeyn/state.js`, que las páginas importan solo cuando contienen un elemento con `data-zeyn-remember` o `data-zeyn-prefs-clear`. `GET /_zeyn/prefs` responde `{ preferences }`, que indica si el hook de consentimiento de la aplicación permite conservar valores de preferencias más allá de la sesión, con `Cache-Control: no-store`. El servidor no lee ningún valor guardado.

### 7.4 Medios

`Video` y `Audio` se sirven con `/_zeyn/media.js` y `/_zeyn/media.css`, enlazados después de la biblioteca de componentes. Véase `docs/media.md`.

| Endpoint | Hace |
|---|---|
| `GET /_zeyn/media/{source}/{id}` | Un archivo de una fuente de medios registrada con peticiones de rango: `200`, `206`, `304`, `416`. |
| `GET /_zeyn/media/position?source=&id=` | `{ at, kept }`, donde se detuvo el usuario con sesión iniciada; `kept` es `false` cuando la fuente no guarda posiciones (el reproductor entonces usa el almacenamiento local). |
| `POST /_zeyn/media/position` | `{ source, id, at }` con la cabecera CSRF la guarda: `204`, `422` para una posición incorrecta. |

### 7.5 Islas

Con la feature `islands`, una pantalla que contiene `div[data-zeyn-island]` se sirve con
`<meta name="zeyn-islands">` (nombre de isla a URL `{ js, wasm }`), `/_zeyn/islands.js` y una política
con `'wasm-unsafe-eval'`; las demás páginas no cambian. Una respuesta parcial que contiene una isla lleva
`x-zeyn-islands: 1`, y `nav.js` la carga completa cuando el documento no tiene la política de islas.

| Atributo o endpoint | Significado |
|---|---|
| `data-zeyn-island`, `data-zeyn-load`, `data-zeyn-props` | Nombre de la isla, `visible`, `interaction` o `idle`, JSON de props |
| `data-zeyn-island-fallback` | Contenido alternativo, oculto cuando la isla se monta |
| `data-zeyn-island-state` | Lo establece el cargador: `loading`, `ready`, `failed` |
| `GET /_zeyn/islands/{file}` | Un archivo compilado que figura en el manifiesto, en caché durante un año |
| `POST /_zeyn/data` con `key` = nombre remoto | Una llamada tipada desde la isla, con todas las comprobaciones de 7.1 |

Véase `docs/islands.md`.

### 7.6 Tiempo real

Con la feature `realtime`, `div[data-zeyn-live="topic"]` marca los nodos que se refrescan cuando el servidor
notifica el topic. Las páginas que contienen uno enlazan `/_zeyn/live.js`; una respuesta parcial que contiene uno lleva
`x-zeyn-live: 1` y `nav.js` importa el script. `zeyn:reload` sobre un elemento recarga en su sitio los grids
que contiene.

| Endpoint | Hace |
|---|---|
| `GET /_zeyn/live/topics?t=a,b&csrf=` | WebSocket; tramas de texto `{ "topic": "a" }` |
| `GET /_zeyn/live/doc/{kind}/{doc}?csrf=` | WebSocket; tramas binarias `0x01` + actualización Y (v1) en ambos sentidos |

Los handshakes comprueban el origen, el token CSRF frente a la cookie, el usuario con sesión iniciada y los
permisos antes de actualizar la conexión. Véase `docs/realtime.md`.

El reproductor despacha el evento cancelable `zeyn:media-adaptive` (`{ media, kind, url, hls, dash }`) en su raíz antes de recurrir a los archivos progresivos; una biblioteca de streaming llama a `preventDefault()` para tomar el control.

## 8. Shell de estilo escritorio

`/_zeyn/pro.js` y `/_zeyn/pro.css` se cargan después de la biblioteca de componentes. Igual que el runtime, no requieren inicio de sesión.

### 8.1 Comandos y la paleta

Los comandos se declaran en Rust (`Command::open`, `Command::navigate`, `Command::effect`, `Command::action`) y se registran con `Feature::command`. El shell renderiza un botón oculto por cada comando que el usuario puede ejecutar (los guards de la feature pasan y el acceso del comando lo permite) dentro de `div[data-zeyn-commands]`:

| Atributo | Significado |
|---|---|
| `data-zeyn-command` | Id del comando |
| `data-zeyn-effect` | Effect de un comando de effect, que el runtime ejecuta al hacer clic |
| `data-zeyn-command-run` | Id de un comando de action, que se ejecuta con `POST /_zeyn/command` |
| `aria-keyshortcuts` | Atajo normalizado (`Control+Alt+N`); el manejo de atajos del runtime hace clic en el botón |
| `data-hint`, `data-icon` | Atajo tal como se muestra a los usuarios, nombre del icono |
| `data-text-*` en el contenedor | Textos localizados de la paleta |

`App::check` rechaza los atajos que no se pueden leer, los atajos de un solo carácter (WCAG 2.1.4), el atajo propio de la paleta, los atajos o ids usados por dos comandos, los atajos que el navegador nunca pasa a la página (`BROWSER_RESERVED`: `Ctrl+N`, `Ctrl+T`, `Ctrl+W`, `Ctrl+Tab`, `Alt+F4` y sus formas en macOS) y, salvo que el comando llame a `.override_browser()`, los atajos que usa el navegador (`BROWSER_DEFAULTS`: imprimir, guardar, buscar, barra de direcciones, recargar, zoom, herramientas de desarrollo, copiar y pegar, de `F1` a `F12`). Los botones de la barra de herramientas siguen las mismas reglas en tiempo de ejecución: un atajo así no se enlaza y el motivo se registra en el log. Las combinaciones libres son `Alt+Shift+<letra>` y `Ctrl+Alt+<letra>`.

`Alt+Shift+P` (Option+Shift+P en macOS) abre la paleta por defecto; `App::palette_shortcut("Ctrl+Shift+P")` lo cambia, también a una tecla que usa el navegador, a propósito. El atajo está en `data-palette-keys`, y las letras se emparejan por tecla física, de modo que Option y AltGr no lo rompen. La paleta también se abre desde `button[data-zeyn-palette-open]` en el menú. Es un `<dialog>` modal con un combobox y un listbox de ARIA. Enumera los comandos de la aplicación, los botones de la barra de herramientas, los comandos de la barra de menús y los elementos del menú contextual de la pantalla actual (la ventana superior cuando hay una abierta) y las pantallas del menú. La coincidencia es aproximada e ignora mayúsculas y acentos; los ocho comandos ejecutados más recientemente van primero (clave de `localStorage` `zeyn.palette.recent`). Escape la cierra y el foco vuelve a donde estaba.

| Endpoint | Cuerpo | Respuesta |
|---|---|---|
| `POST /_zeyn/command` | `{ command }` | Ejecuta un comando de action con la entrada fijada en el servidor: CSRF, guards de la feature, el permiso de la action y los interceptores. `{ effects }`; `404` para ids desconocidos y comandos de effect |
| `POST /_zeyn/search` | `{ query }` | Ejecuta cada `Feature::search` que el usuario puede ejecutar (CSRF, guards de la feature, el permiso de la action, interceptores) con la consulta cortada a 100 caracteres. `{ hits: [{ label, group, url }] }`, como máximo 20, solo paths de la aplicación |

Cuando el contenedor tiene `data-search`, la paleta envía la consulta 150 ms después de la última tecla (dos caracteres o más) y enumera los resultados bajo los comandos. Solo se dibuja la respuesta a la última consulta. Elegir un resultado envía `zeyn:navigate` con el base path antepuesto a `url`; un `#anchor` en ella desplaza hasta ese elemento después de intercambiar la pantalla.

El runtime escucha `zeyn:run` en `document` para que otros scripts ejecuten trabajo a través de él: `{ effects, source }` ejecuta effects, `{ screen, item }` ejecuta un elemento del menú contextual de una pantalla, `{ error }` muestra un toast de error.

### 8.2 Barra de menús, caja de herramientas, inspector de propiedades, barra de estado

| Marcado | Comportamiento |
|---|---|
| `ul[role=menubar][data-zeyn-menubar]` | Barra de menús según W3C APG. Los elementos superiores son `button[role=menuitem][aria-haspopup=menu]` con una parada de tabulación itinerante (roving); cada uno controla un `ul[role=menu]` que está `hidden` mientras está cerrado. Los elementos llevan los atributos de comando de 8.1. Izquierda y Derecha se mueven entre menús, Abajo, Enter y Space abren, Arriba abre en el último elemento, Home y End saltan, una letra pasa al siguiente elemento que empieza por ella, Escape cierra y vuelve a la barra, Tab y un clic fuera cierran. Los menús y elementos que el usuario no puede ejecutar no se renderizan. |
| `div[role=toolbar][aria-orientation=vertical][data-zeyn-toolbox]` | Grupo de conmutadores de `button.z-tool[aria-pressed]`. Las flechas, Home y End mueven la parada de tabulación itinerante; un clic, Enter o Space activa una herramienta, envía `zeyn:tool` (`detail.tool`) y escribe `data-text-selected` en la barra de estado. |
| `section.z-inspector` | Un formulario con grupos `Form::section` (`details.z-form-section` con cuerpos `role=group`) guardado mediante `POST /_zeyn/form` en el path del nodo del inspector. Las secciones que contienen campos con errores se abren antes de las comprobaciones del navegador y después de un `422`. |
| `div[data-zeyn-statusbar]` | Elementos al inicio y al final y `[data-zeyn-status-live]` (`role=status`, `aria-live=polite`). `zeyn:status` (`detail.text`) desde cualquier elemento de la pantalla reemplaza el texto en vivo. |

### 8.3 Paneles acoplables

`div.z-dock[data-zeyn-dock]` es un grid de cuatro `section.z-dock-area[data-area]` (`left`, `center`, `bottom`, `right`) y tres divisores `div.z-dock-bar[role=separator][data-edge]`. Los tamaños son las propiedades personalizadas `--z-dock-left`, `--z-dock-right` (porcentaje del ancho) y `--z-dock-bottom` (porcentaje del alto), de 10 a 60. Cada área tiene un `role=tablist` de `button.z-dock-tab[data-panel]` arrastrables, un botón de opciones y paneles `role=tabpanel` que contienen los nodos hijos (paths como en cualquier contenedor; nunca cambian cuando se mueve un panel). Las áreas laterales sin paneles abiertos se ocultan.

Teclado: Izquierda y Derecha (invertidas en páginas de derecha a izquierda), Home y End se mueven entre las pestañas abiertas de un área; Delete cierra el panel enfocado; Shift+F10 o la tecla de menú contextual abre las opciones del área (mover a otra área, cerrar, mostrar los paneles cerrados); las flechas sobre un divisor redimensionan un 5 por ciento, Home y End van a los límites. Una entrada de la barra de menús creada con `MenuList::panels` enumera cada panel como un `menuitemcheckbox`. Los cambios se anuncian en `[data-zeyn-dock-live]`.

| Endpoint | Cuerpo | Respuesta |
|---|---|---|
| `POST /_zeyn/dock` | `{ screen, params, node, save: false }` | `{ layout }`: la disposición guardada del usuario con sesión iniciada o `null` |
| `POST /_zeyn/dock` | `{ screen, params, node, save: true, layout }` | `204`; `layout: null` la elimina |

`layout` es `{ sizes: { left, right, bottom }, panels: [{ id, area, open, active }] }` en el orden de las pestañas. El servidor comprueba CSRF, el acceso de la pantalla y los guards de la feature, conserva solo los paneles propios del dock (cada uno una vez) y acota los tamaños, y luego lo almacena en el almacén de vistas de la aplicación (`App::datagrid_views`) bajo el tenant, la pantalla y el nodo. Sin un almacén el endpoint responde `404` y sin un usuario `403`; el navegador entonces conserva la disposición en `localStorage`.

## 9. Agentes de IA: MCP y WebMCP

Los agentes de IA externos llegan a la aplicación por dos puertas, ambas desactivadas por defecto y activadas con
`App::mcp`:

| Ajuste | Endpoint MCP | Herramientas WebMCP | Escrituras |
|---|---|---|---|
| `Mcp::off()` (predeterminado) | `404` | `404` | no |
| `Mcp::read_only()` | activo | activo | no |
| `Mcp::on()` | activo | activo | guardados de formularios y actions de API que escriben |

`.server(false)` y `.webmcp(false)` desactivan una puerta. Los proyectos nuevos de `zeyn new` empiezan con
`Mcp::read_only()`.

Por qué desactivado: el endpoint permite que cualquier titular de una cabecera `Authorization` válida maneje la aplicación a través de un
modelo, y un modelo puede ser manipulado por los datos que lee. El framework deja cada aplicación lista para
los agentes (las herramientas se generan a partir del código, no hay nada que escribir), pero que una aplicación abra esa puerta,
y que los agentes puedan escribir, es una decisión de las personas que la operan. Solo lectura está a una línea de distancia.

### 9.1 Herramientas

Las herramientas se construyen a partir de la aplicación para el usuario con sesión iniciada:

- una herramienta de búsqueda por grid (`{screen}_{node}_search`, entrada: un `DataQuery`) y, con escrituras, una
  herramienta de guardado por formulario (`{screen}_{node}_save`, entrada: los valores del formulario tal como se escriben) de cada pantalla
  enrutada que se abre sin parámetros;
- una herramienta por cada action de API publicada (`api_{feature}_{action}`). Una action cuya entrada es un
  `DataQuery` lee; cualquier otra action de API cuenta como una escritura.

La descripción se construye a partir del título de la pantalla, el primer párrafo del comentario de documentación `///` de la pantalla y de la
action, y su guía `ai = "..."`. Los grids y formularios que el usuario no puede usar, y las pantallas y actions marcadas con `ai = false`, nunca se enumeran y se rechazan cuando se llaman.

`#[action(confirm)]` marca una action que un modelo solo puede ejecutar después de que el usuario la confirme en la aplicación.
`/_zeyn/mcp` no enumera ni ejecuta esas herramientas (nadie en ese canal puede confirmar). Las herramientas WebMCP
preguntan en un diálogo modal (`zeyn:confirm`) y luego envían `confirmed: true` con el formulario; el
asistente integrado pregunta en su chat (véase `docs/assistant.md`).

### 9.2 Endpoints

| Endpoint | Hace |
|---|---|
| `GET /_zeyn/tools?screen&params` | `{ tools: [{ name, description, inputSchema, kind, screen, node, writes, confirm }], texts }` para la pantalla a la vista. `zeyn.js` las registra con `document.modelContext` |
| `POST /_zeyn/mcp` | JSON-RPC: `initialize`, `tools/list`, `tools/call`. Requiere una cabecera `Authorization` y un usuario con sesión iniciada |
| `POST /_zeyn/form` con `via_agent: true` | Un guardado de formulario por WebMCP; requiere `Mcp::on()` con WebMCP. `confirmed: true` solo después de que el usuario confirmó |

Cada llamada de agente se ejecuta como el usuario: el acceso de la pantalla, los guards, el permiso de la action,
los interceptores, los límites de frecuencia y el límite de tiempo. Las llamadas de agente siempre se auditan (`via_agent: true`)
y los campos enmascarados siempre se enmascaran para ellas, incluso cuando el usuario puede revelarlos. OpenAPI marca
cada operación con `x-zeyn-ai`: `false` para `ai = false`, y en caso contrario `{ hint, confirm, writes }`.
