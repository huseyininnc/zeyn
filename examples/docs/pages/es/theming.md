# Theming

Estado: implementado en `crates/zeyn/assets/zeyn.css`.

El UI kit predeterminado lee cada color, fuente, tamaño y radio de propiedades personalizadas de CSS en `:root`. Una organización cambia la apariencia sobrescribiendo unas pocas. Nunca necesita editar `zeyn.css` ni escribir selectores.

## 1. Cómo se aplica un tema

El servidor sirve primero `zeyn.css` y después, cuando la app tiene una, una hoja de estilos adicional con las sobrescrituras de tokens. El archivo de sobrescrituras solo contiene bloques `:root`:

```css
:root {
  --z-brand: #7a2448;
  --z-font: "Source Sans 3", "Segoe UI", system-ui, sans-serif;
  --z-radius: 8px;
  --z-density: 0.75;
}
```

Reglas para el archivo de sobrescrituras:

- Debe servirse desde el propio origen de la app. La Content Security Policy es `style-src 'self'`, por lo que los bloques `<style>` en línea y las hojas de estilos de terceros se bloquean.
- Las fuentes deben estar ya instaladas en las máquinas de los usuarios o servirse desde el origen de la app. El kit en sí no carga fuentes web.
- Cárgalo después de `zeyn.css`, para que a igualdad de selectores gane el orden.
- Define los tokens base de la sección 2. Los tokens derivados de la sección 3 los siguen automáticamente. Sobrescribe un token derivado solo cuando el valor calculado no funcione para tu marca.

### Temas claro, oscuro y de la app

Toda app tiene un selector de tema en el menú, junto al selector de idioma. Ofrece System (sigue `prefers-color-scheme`), Light, Dark y cualquier tema que defina la app:

```rust
App::new("Billing")
    .theme(Theme::new().brand("#7a2448").radius(6))
    .themes([
        Theme::light(),
        Theme::dark(),
        Theme::named("sand", [("--z-bg", "#f5f1ea"), ("--z-surface", "#fffdf8")]).label(t!("theme.sand")),
    ])
    .default_theme("system")
```

- `App::theme` es la apariencia de la organización y se aplica bajo todos los temas.
- `App::themes` lista lo que los usuarios pueden elegir. Sin ella, la lista es Light y Dark. System se ofrece cuando ambos están en la lista. `Theme::named(...)` se construye sobre el tema claro; `.dark_base()` lo construye sobre el oscuro.
- `App::default_theme` es el tema de los usuarios que no han elegido ninguno. `system` si no se define.
- `App::theme_picker(false)` oculta el selector; todos obtienen el tema predeterminado.
- Un solo tema bloquea la app: `.themes([Theme::named("acme", tokens)])` o `.theme(Theme::dark())`.

Los tokens se comprueban cuando arranca la app. `App::check` (y por tanto `router` y `serve`) informa de nombres de token desconocidos, valores que podrían salirse de su declaración (`;`, llaves, `\`, `<`, `>`, `url(`), nombres de tema incorrectos, un tema predeterminado desconocido y cada par de colores de la sección 5 que un tema deje por debajo de WCAG 2.2 AA. Solo los valores `#rgb` y `#rrggbb` se pueden comprobar en cuanto al contraste; las demás sintaxis de color se dan por buenas.

Todos los temas se sirven en una sola hoja de estilos, `/_zeyn/theme.css`, construida una vez al arrancar y cargada después de las hojas de estilos del kit. La elección se guarda en la cookie `zeyn_theme` y, para un usuario con sesión iniciada en una app con un view store (`App::datagrid_views`), en ese store, de modo que lo sigue a un dispositivo nuevo. El servidor renderiza el tema en `<html>`, así que el primer pintado ya es correcto y no hace falta ningún script en línea:

| Marcado | Resultado |
|---|---|
| `<html>` | System: sigue `prefers-color-scheme` |
| `<html data-theme="light">` | Siempre claro |
| `<html data-theme="dark">` | Siempre oscuro |
| `<html data-theme="light" data-zeyn-theme="sand">` | El tema de la app `sand`, sobre el tema claro |

Cambiar el tema en el selector solo cambia estos atributos y el meta `color-scheme`, y luego llama a `POST /_zeyn/theme`. No se recarga nada.

Para dar al tema oscuro sus propios valores en un archivo de sobrescrituras escrito a mano, repite los selectores que usa el kit:

```css
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --z-bg: #0e1012; }
}
:root[data-theme="dark"] { --z-bg: #0e1012; }
```

### Densidad

Dos densidades, que se cambian con tokens. Cómoda es la predeterminada: filas de grid de 40 px, encabezados de 36 px, controles de 36 px, relleno de tarjeta de 20 px. Compacta (`<html data-density="compact">`) tiene filas, encabezados y controles de 32 px, y escala la unidad de espaciado a 0.8. Los usuarios la eligen en el menú de cuenta de la barra superior (`App::header(|h| h.account())`); la elección es la cookie `zeyn_density`, que el servidor renderiza en `<html>`, así que el primer pintado ya es correcto. Los controles y las filas del grid nunca son más pequeños que 24 px, así que la densidad compacta sigue cumpliendo el tamaño de destino de WCAG 2.2. `Theme::new().compact()` hace del espaciado compacto el predeterminado de la organización.

La altura de fila de un grid virtual la fija el servidor porque el runtime calcula a partir de ella las posiciones de desplazamiento. `Paging::virtual_scroll()` sigue la densidad (40 px en cómoda, 32 px en compacta); `Paging::virtual_rows(px)` mantiene la altura que recibe. Un cambio de densidad se aplica a los grids virtuales a partir de la página siguiente.

## 2. Tokens base

Estos son los tokens que se espera que establezca una organización.

| Token | Propósito | Claro | Oscuro |
|---|---|---|---|
| `--z-brand` | El único color de marca. Marca del logotipo, botones primarios, elemento de menú actual, anillo de foco | `#0f6674` | mismo valor; consulta `--z-accent` |
| `--z-on-brand` | Texto sobre una superficie `--z-brand` sólida | `#ffffff` | `#ffffff` |
| `--z-font` | Pila de fuentes de la interfaz | `"Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, "Noto Sans", sans-serif` | igual |
| `--z-font-mono` | Código, ejemplos de API | `ui-monospace, "Cascadia Mono", "SF Mono", "Segoe UI Mono", Menlo, Consolas, monospace` | igual |
| `--z-font-size` | Texto del cuerpo | `0.875rem` (14 px) | igual |
| `--z-font-size-sm` | Celdas del grid, etiquetas, filtros | `0.8125rem` (13 px) | igual |
| `--z-font-size-xs` | Encabezados de columna, títulos de grupo de menú, información de página | `0.75rem` (12 px) | igual |
| `--z-radius` | Radio de esquina base, de 0 a 12 px. Botones, entradas, elementos de menú; todos los demás radios derivan de él | `6px` | igual |
| `--z-density` | Multiplicador de espaciado, `1` normal, `0.75` compacto | `1` | igual |
| `--z-bg` | Fondo de página detrás de los paneles | `#f5f6f8` | `#0d0f12` |
| `--z-surface` | Paneles, grids, tarjetas, entradas | `#ffffff` | `#15181d` |
| `--z-surface-2` | Encabezado y pie del grid, pie de ventana, entradas de solo lectura | `#f8f9fa` | `#1a1e24` |
| `--z-raised` | Popovers, menús, diálogos, toasts, la paleta de comandos. En oscuro, las capas elevadas son más claras en lugar de depender de sombras | `#ffffff` | `#1f242b` |
| `--z-hover` | Fondo al pasar el cursor para filas, elementos de menú, botones | `#eff1f4` | `#232830` |
| `--z-border` | Divisores y contornos de panel | `#e2e5e9` | `#2b3139` |
| `--z-border-strong` | Contornos de entradas, bordes de casillas, líneas base de gráficos. Debe alcanzar 3:1 frente a `--z-surface` | `#858d97` | `#6f7884` |
| `--z-text` | Texto del cuerpo y valores | `#15181d` | `#e7eaee` |
| `--z-text-muted` | Etiquetas, encabezados, texto secundario. Debe alcanzar 4.5:1 frente a `--z-surface-2` | `#5a626c` | `#9ba4af` |
| `--z-danger` | Errores, marcas de obligatorio, filas de peligro | `#b42318` | `#f28b82` |
| `--z-warning` | Filas de advertencia | `#8a5300` | `#e6b45a` |
| `--z-success` | Filas de éxito | `#1a6b3c` | `#6fcf8f` |
| `--z-info` | Toasts informativos | `#275d93` | `#8ab8e6` |

## 3. Tokens derivados

Se calculan a partir de los tokens base con `color-mix()` y la sintaxis de color relativo. Se actualizan solos cuando cambia un token base.

| Token | Propósito | Claro | Oscuro |
|---|---|---|---|
| `--z-accent` | Color de marca usado como texto o línea fina sobre una superficie: enlaces, elemento de menú actual, barra de la fila con foco | `var(--z-brand)` | `oklch(from var(--z-brand) 0.8 c h)`, el matiz de la marca con un tono claro fijo. Los navegadores sin sintaxis de color relativo reciben `color-mix(in oklab, var(--z-brand) 45%, #fff)` |
| `--z-primary` | Relleno del botón primario | `var(--z-brand)` | `var(--z-accent)` |
| `--z-on-primary` | Texto del botón primario | `var(--z-on-brand)` | `#0d0f12` |
| `--z-primary-hover` | Botón primario al pasar el cursor | marca al 86% mezclada con negro | acento al 84% mezclado con blanco |
| `--z-primary-active` | Botón primario pulsado | marca al 74% mezclada con negro | acento al 70% mezclado con blanco |
| `--z-brand-subtle` | Tinte detrás del elemento de menú actual y de la fila del grid con foco | acento a `--z-tint` sobre `--z-surface` | misma fórmula |
| `--z-focus` | Anillo de foco | `var(--z-accent)` | `var(--z-accent)` |
| `--z-danger-subtle`, `--z-warning-subtle`, `--z-success-subtle`, `--z-info-subtle` | Fondos de filas y mensajes | color de estado a `--z-tint` sobre `--z-surface` | misma fórmula |
| `--z-tint` | Intensidad de los tintes sutiles | `9%` | `16%` |
| `--z-text-2xs` a `--z-text-num-lg` con `--z-leading-*` | Escala tipográfica, tamaño sobre altura de línea: `2xs` 11/16 (teclas, marcas de eje de gráficos en teléfonos), `xs` 12/16 (encabezados de grid, leyendas, insignias), `sm` 13/20 (celdas, navegación, etiquetas, botones), `md` 14/20 (cuerpo, entradas), `lg` 16/24 (títulos de sección de tarjetas, gráficos y diálogos), `xl` 20/28 (títulos de ventana, títulos de página en teléfonos), `2xl` 24/32 (título de página), `num` 30/36 (valores KPI), `num-lg` 40/44 (métrica destacada). Los pesos son 400 para el texto, 500 para etiquetas, botones y elementos actuales, y 600 solo para títulos y números | | igual |
| `--z-space` | Unidad de espaciado | `calc(4px * var(--z-density))` | igual |
| `--z-control` | Altura de botones y entradas | `max(24px, 9 * --z-space)` (36 px; 32 px en compacta) | igual |
| `--z-row`, `--z-row-head` | Altura de fila y de encabezado del grid para grids paginados e infinitos | 40 px y 36 px (32 px en compacta) | igual |
| `--z-gap` | Espacio entre tarjetas y secciones de una pantalla | `4 * --z-space` (16 px) | igual |
| `--z-s-0_5`, `--z-s-1`, `--z-s-1_5`, `--z-s-2`, `--z-s-3`, `--z-s-4`, `--z-s-5`, `--z-s-6`, `--z-s-8`, `--z-s-10`, `--z-s-12`, `--z-s-16` | Escala de espaciado: el número multiplicado por `--z-space` (2, 4, 6, 8, 12, 16, 20, 24, 32, 40, 48 y 64 px con densidad 1) | | igual |
| `--z-radius-sm` | Insignias, chips, casillas, teclas de teclado | `max(2px, --z-radius - 2px)` (4 px) | igual |
| `--z-radius-lg` | Tarjetas, grids, popovers, toasts | `--z-radius + 2px` (8 px) | igual |
| `--z-radius-xl` | Diálogos, la tarjeta de inicio de sesión, la paleta de comandos | `--z-radius + 6px` (12 px) | igual |
| `--z-radius-full` | Avatares, interruptores, puntos de estado, píldoras de recuento | `999px` | igual |
| `--z-shadow-1` | Botones predeterminados, el indicador de un control segmentado | `0 1px 2px` al 6% | ninguna |
| `--z-shadow-2` | Popovers, menús, tooltips, toasts. Las filas del grid y las tarjetas nunca llevan sombra | dos capas suaves | un anillo `--z-border` de 1 px y una sombra profunda |
| `--z-shadow-3` | Diálogos y la paleta de comandos | dos capas más profundas | un anillo de 1 px y una sombra más profunda |
| `--z-dur-1`, `--z-dur-2`, `--z-dur-3`, `--z-dur-out` | Movimiento: colores al pasar el cursor (80 ms), entrada de popovers y toasts (140 ms), entrada de diálogos, cajones y la paleta (200 ms), toda salida (100 ms). Con `prefers-reduced-motion: reduce` pasan a 0, y los fundidos duran 80 ms como mucho | | igual |
| `--z-ease-out`, `--z-ease-in`, `--z-ease-linear` | Curva de las entradas y movimientos, de las salidas y de los cambios de color | `cubic-bezier(0.2, 0, 0, 1)`, `cubic-bezier(0.4, 0, 1, 1)`, `linear` | igual |
| `--z-backdrop` | Detrás de una ventana modal | `rgb(20 24 28 / 40%)` | `rgb(0 0 0 / 60%)` |
| `--z-dir` | Sentido de lectura como signo, para desplazamientos sin forma lógica (sombras interiores que marcan el lado de inicio o de fin, transformaciones de desplazamiento): `calc(2px * var(--z-dir))` | `1`, y `-1` con `dir="rtl"` | igual |

Tokens por elemento establecidos por el marcado del servidor: `--z-width` (ancho de ventana), `--z-flex` (proporción de columna), `--z-row-height` (altura de fila del grid virtual) y, para la biblioteca de componentes, `--z-stack-gap`, `--z-split`, `--z-progress`, `--z-columns`, `--z-level` y `--z-pop-x` / `--z-pop-y` (posición de popup).

Los colores de serie de los gráficos son `--z-chart-1` a `--z-chart-8` en `components.css`: azul `#2563d9`, naranja `#d67400`, magenta `#a3369b`, verde `#14866d`, oliva `#7a6a00`, violeta `#6e56cf`, rojo `#cc3b33` y pizarra `#56636f`, con valores más claros en el tema oscuro (`#5b9bff`, `#f08a3c`, `#e27bd6`, `#38c29f`, `#ece27a`, `#9d8cff`, `#ff7b70`, `#9aa7b5`). El orden mantiene distinguibles los cinco primeros con deuteranopía, protanopía y tritanopía, y la serie 1 no es la marca, así que una marca roja o verde nunca se lee como algo bueno o malo; aun así una app puede establecer `--z-chart-1: var(--z-brand)`. La comprobación de arranque exige a cada uno 3:1 frente a `--z-surface`. Sobrescríbelos como cualquier token base.

## 4. Elegir un color de marca

- `--z-brand` lleva texto blanco en el tema claro (botones primarios, la letra del logotipo, el panel de marca del shell de inicio de sesión). Elige un color con al menos 4.5:1 frente a `--z-on-brand`. La mayoría de los colores de marca de tono medio a oscuro lo cumplen. Para un color de marca claro, como el amarillo, establece también `--z-on-brand` con un color oscuro.
- En el tema oscuro el kit aclara la marca para el texto, los anillos y los botones primarios, y mantiene la marca sólida para la letra del logotipo. Una marca distinta se ve intencionada en ambos temas sin un segundo color.
- Mantén los colores de estado distintos de la marca. Una marca roja junto a filas de error rojas dificulta detectar los errores; en ese caso desplaza `--z-danger` hacia un rojo diferente o mantén la marca fuera de los grids sobrescribiendo `--z-brand-subtle`.
- Comprueba tus colores usando como objetivo la tabla de la sección 5. Cualquier par por debajo del umbral incumple WCAG 2.2 AA.

## 5. Contraste verificado

Cada par de abajo se comprueba al arrancar la app (`App::check`), para los temas integrados y para cada tema de la app, y también en la prueba unitaria `built_in_themes_reach_wcag_aa_on_every_pair` de `src/server/theme.rs` (ejecútala con `--nocapture` para imprimir esta tabla). Los tokens derivados (`--z-accent`, los colores primarios y los tintes sutiles) se calculan igual que en `zeyn.css`, en Oklab. Umbrales: 4.5:1 para texto, 3:1 para los contornos de componentes de interfaz y los indicadores de foco.

| Par | Claro | Oscuro | Necesario |
|---|---|---|---|
| `--z-text` sobre `--z-bg` | 16.45 | 15.90 | 4.5 |
| `--z-text` sobre `--z-surface` | 17.79 | 14.74 | 4.5 |
| `--z-text` sobre `--z-surface-2` | 16.88 | 13.86 | 4.5 |
| `--z-text` sobre `--z-raised` | 17.79 | 12.93 | 4.5 |
| `--z-text` sobre `--z-hover` | 15.72 | 12.27 | 4.5 |
| `--z-text` sobre `--z-brand-subtle` | 15.56 | 11.18 | 4.5 |
| `--z-text` sobre `--z-danger-subtle` | 15.49 | 11.60 | 4.5 |
| `--z-text` sobre `--z-warning-subtle` | 15.59 | 11.20 | 4.5 |
| `--z-text` sobre `--z-success-subtle` | 15.53 | 11.23 | 4.5 |
| `--z-text` sobre `--z-info-subtle` | 15.52 | 11.40 | 4.5 |
| `--z-text-muted` sobre `--z-bg` | 5.71 | 7.61 | 4.5 |
| `--z-text-muted` sobre `--z-surface` | 6.18 | 7.05 | 4.5 |
| `--z-text-muted` sobre `--z-surface-2` | 5.86 | 6.63 | 4.5 |
| `--z-text-muted` sobre `--z-raised` | 6.18 | 6.19 | 4.5 |
| `--z-text-muted` sobre `--z-hover` | 5.46 | 5.87 | 4.5 |
| `--z-text-muted` sobre `--z-brand-subtle` | 5.40 | 5.35 | 4.5 |
| `--z-border-strong` sobre `--z-surface` | 3.36 | 3.98 | 3 |
| `--z-border-strong` sobre `--z-surface-2` | 3.19 | 3.74 | 3 |
| `--z-border-strong` sobre `--z-raised` | 3.36 | 3.49 | 3 |
| `--z-accent` sobre `--z-surface` | 6.62 | 9.81 | 4.5 |
| `--z-accent` sobre `--z-bg` | 6.12 | 10.58 | 4.5 |
| `--z-accent` sobre `--z-brand-subtle` | 5.79 | 7.44 | 4.5 |
| `--z-focus` sobre `--z-surface` | 6.62 | 9.81 | 3 |
| `--z-focus` sobre `--z-bg` | 6.12 | 10.58 | 3 |
| `--z-on-primary` sobre `--z-primary` | 6.62 | 10.58 | 4.5 |
| `--z-on-primary` sobre `--z-primary-hover` | 8.82 | 11.70 | 4.5 |
| `--z-on-primary` sobre `--z-primary-active` | 11.12 | 12.74 | 4.5 |
| `--z-danger` sobre `--z-surface` | 6.57 | 7.45 | 4.5 |
| `--z-danger` sobre `--z-danger-subtle` | 5.73 | 5.86 | 4.5 |
| `--z-warning` sobre `--z-surface` | 6.33 | 9.35 | 4.5 |
| `--z-warning` sobre `--z-warning-subtle` | 5.55 | 7.11 | 4.5 |
| `--z-success` sobre `--z-surface` | 6.54 | 9.32 | 4.5 |
| `--z-success` sobre `--z-success-subtle` | 5.70 | 7.09 | 4.5 |
| `--z-info` sobre `--z-surface` | 6.83 | 8.54 | 4.5 |
| `--z-info` sobre `--z-info-subtle` | 5.96 | 6.60 | 4.5 |
| `--z-on-brand` sobre `--z-brand` | 6.62 | 6.62 | 4.5 |
| `--z-brand` sobre `--z-surface` | 6.62 | n.a. | 4.5 |

`--z-border` es decorativo (divisores entre elementos que ya tienen texto) y no se exige que alcance 3:1.

## 6. Otros comportamientos de accesibilidad

- Todo elemento interactivo muestra un anillo `:focus-visible` de 2 px en `--z-focus`. Las filas del grid dibujan el anillo dentro de la fila para que el contenedor de desplazamiento nunca lo recorte.
- Los botones, las entradas, los encabezados de orden, los cuadros de filtro, los botones del paginador y el botón de cierre de ventana miden al menos 24 x 24 px con cualquier densidad.
- `prefers-reduced-motion: reduce` elimina la entrada de los toasts y todas las transiciones.
- En el modo `forced-colors`, los iconos usan `CanvasText`, el botón primario y la fila con foco usan `Highlight`, y el estado que depende de tintes de fondo (elemento de menú actual, estado de fila) recibe en su lugar un contorno o borde.
