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

`--z-density` escala la unidad de espaciado. `1` es normal, `0.75` es compacto. `<html data-density="compact">` establece `0.75` sin hoja de estilos. Los controles y las filas del grid nunca son más pequeños que 24 px, así que el modo compacto sigue cumpliendo el tamaño de destino de WCAG 2.2.

La altura de fila de un grid virtual la fija el servidor (`Paging::virtual_rows(px)`, 32 px de forma predeterminada) porque el runtime calcula a partir de ella las posiciones de desplazamiento. La densidad no la cambia.

## 2. Tokens base

Estos son los tokens que se espera que establezca una organización.

| Token | Propósito | Claro | Oscuro |
|---|---|---|---|
| `--z-brand` | El único color de marca. Botones primarios, la banda del nombre de la app, el elemento de menú actual, la fila con foco, el anillo de foco | `#0f6674` | mismo valor; consulta `--z-accent` |
| `--z-on-brand` | Texto sobre una superficie `--z-brand` sólida | `#ffffff` | `#ffffff` |
| `--z-font` | Pila de fuentes de la interfaz | `"Segoe UI Variable Text", "Segoe UI", system-ui, -apple-system, BlinkMacSystemFont, "Helvetica Neue", Arial, "Noto Sans", sans-serif` | igual |
| `--z-font-mono` | Código, ejemplos de API | `ui-monospace, "Cascadia Mono", "SF Mono", "Segoe UI Mono", Menlo, Consolas, monospace` | igual |
| `--z-font-size` | Texto del cuerpo | `0.875rem` (14 px) | igual |
| `--z-font-size-sm` | Celdas del grid, etiquetas, filtros | `0.8125rem` (13 px) | igual |
| `--z-font-size-xs` | Encabezados de columna, títulos de grupo de menú, información de página | `0.75rem` (12 px) | igual |
| `--z-radius` | Radio de esquina base, de 0 a 12 px. Todos los componentes escalan a partir de él | `4px` | igual |
| `--z-density` | Multiplicador de espaciado, `1` normal, `0.75` compacto | `1` | igual |
| `--z-bg` | Fondo de página detrás de los paneles | `#f4f5f6` | `#121416` |
| `--z-surface` | Paneles, grids, ventanas, entradas | `#ffffff` | `#1a1d20` |
| `--z-surface-2` | Encabezado del grid, filas alternas, pie de ventana, entradas de solo lectura | `#f7f8f9` | `#202326` |
| `--z-hover` | Fondo al pasar el cursor para filas, elementos de menú, botones | `#eceef1` | `#2a2e33` |
| `--z-border` | Divisores y contornos de panel | `#dcdfe3` | `#33383e` |
| `--z-border-strong` | Contornos de entradas y botones, línea del encabezado. Debe alcanzar 3:1 frente a `--z-surface` | `#7d848d` | `#7a818a` |
| `--z-text` | Texto del cuerpo y valores | `#1a1d21` | `#e4e6e9` |
| `--z-text-muted` | Etiquetas, encabezados, texto secundario. Debe alcanzar 4.5:1 frente a `--z-surface-2` | `#5b626b` | `#a2a9b1` |
| `--z-danger` | Errores, marcas de obligatorio, filas de peligro | `#b3261e` | `#f28b82` |
| `--z-warning` | Filas de advertencia | `#8a5300` | `#e6b45a` |
| `--z-success` | Filas de éxito | `#1e6b3a` | `#6fcf8f` |
| `--z-info` | Toasts informativos | `#2b5d8a` | `#8ab8e6` |

## 3. Tokens derivados

Se calculan a partir de los tokens base con `color-mix()` y la sintaxis de color relativo. Se actualizan solos cuando cambia un token base.

| Token | Propósito | Claro | Oscuro |
|---|---|---|---|
| `--z-accent` | Color de marca usado como texto o línea fina sobre una superficie: enlaces, elemento de menú actual, barra de la fila con foco | `var(--z-brand)` | `oklch(from var(--z-brand) 0.8 c h)`, el matiz de la marca con un tono claro fijo. Los navegadores sin sintaxis de color relativo reciben `color-mix(in oklab, var(--z-brand) 45%, #fff)` |
| `--z-primary` | Relleno del botón primario | `var(--z-brand)` | `var(--z-accent)` |
| `--z-on-primary` | Texto del botón primario | `var(--z-on-brand)` | `#121416` |
| `--z-primary-hover` | Botón primario al pasar el cursor | marca al 86% mezclada con negro | acento al 84% mezclado con blanco |
| `--z-primary-active` | Botón primario pulsado | marca al 74% mezclada con negro | acento al 70% mezclado con blanco |
| `--z-brand-subtle` | Tinte detrás del elemento de menú actual y de la fila del grid con foco | acento a `--z-tint` sobre `--z-surface` | misma fórmula |
| `--z-focus` | Anillo de foco | `var(--z-accent)` | `var(--z-accent)` |
| `--z-danger-subtle`, `--z-warning-subtle`, `--z-success-subtle`, `--z-info-subtle` | Fondos de filas y mensajes | color de estado a `--z-tint` sobre `--z-surface` | misma fórmula |
| `--z-tint` | Intensidad de los tintes sutiles | `9%` | `20%` |
| `--z-space` | Unidad de espaciado | `calc(4px * var(--z-density))` | igual |
| `--z-control` | Altura de botones y entradas | `max(24px, 8 * --z-space)` (32 px) | igual |
| `--z-row` | Altura de fila del grid para grids paginados e infinitos | `max(24px, 8 * --z-space)` (32 px) | igual |
| `--z-gap` | Espacio entre secciones de una pantalla | `3 * --z-space` (12 px) | igual |
| `--z-radius-sm`, `--z-radius-lg` | Partes pequeñas (filtros, código), ventanas | `0.75x` y `1.5x` de `--z-radius` | igual |
| `--z-shadow-2` | Solo ventanas y toasts. Las filas del grid nunca llevan sombra | sombra suave de dos capas | más oscura |
| `--z-backdrop` | Detrás de una ventana modal | `rgb(20 24 28 / 40%)` | `rgb(0 0 0 / 60%)` |

Tokens por elemento establecidos por el marcado del servidor: `--z-width` (ancho de ventana), `--z-flex` (proporción de columna), `--z-row-height` (altura de fila del grid virtual) y, para la biblioteca de componentes, `--z-stack-gap`, `--z-split`, `--z-progress`, `--z-columns`, `--z-level` y `--z-pop-x` / `--z-pop-y` (posición de popup).

Los colores de serie de los gráficos son `--z-chart-1` a `--z-chart-6` en `components.css`, con valores más claros en el tema oscuro. Cada uno alcanza 3:1 frente a `--z-surface`. Sobrescríbelos como cualquier token base.

## 4. Elegir un color de marca

- `--z-brand` lleva texto blanco en el tema claro (botones, banda del nombre de la app). Elige un color con al menos 4.5:1 frente a `--z-on-brand`. La mayoría de los colores de marca de tono medio a oscuro lo cumplen. Para un color de marca claro, como el amarillo, establece también `--z-on-brand` con un color oscuro.
- En el tema oscuro el kit aclara la marca para el texto, los anillos y los botones primarios, y mantiene la marca sólida para la banda del nombre de la app. Una marca distinta se ve intencionada en ambos temas sin un segundo color.
- Mantén los colores de estado distintos de la marca. Una marca roja junto a filas de error rojas dificulta detectar los errores; en ese caso desplaza `--z-danger` hacia un rojo diferente o mantén la marca fuera de los grids sobrescribiendo `--z-brand-subtle`.
- Comprueba tus colores usando como objetivo la tabla de la sección 5. Cualquier par por debajo del umbral incumple WCAG 2.2 AA.

## 5. Contraste verificado

Medido en Chromium sobre la app de ejemplo en ejecución: cada token se resolvió a su valor sRGB renderizado (incluidos `color-mix()` y los colores relativos), y después se calculó la razón de contraste de WCAG 2.x. Umbrales: 4.5:1 para texto, 3:1 para los contornos de componentes de interfaz y los indicadores de foco.

| Par | Claro | Oscuro | Necesario |
|---|---|---|---|
| `--z-text` sobre `--z-bg` | 15.49 | 14.77 | 4.5 |
| `--z-text` sobre `--z-surface` | 16.91 | 13.54 | 4.5 |
| `--z-text` sobre `--z-surface-2` | 15.91 | 12.63 | 4.5 |
| `--z-text` sobre `--z-hover` | 14.55 | 10.93 | 4.5 |
| `--z-text` sobre `--z-brand-subtle` (fila con foco) | 14.79 | 9.27 | 4.5 |
| `--z-text` sobre `--z-danger-subtle` | 14.73 | 9.76 | 4.5 |
| `--z-text` sobre `--z-warning-subtle` | 14.82 | 9.31 | 4.5 |
| `--z-text` sobre `--z-success-subtle` | 14.76 | 9.33 | 4.5 |
| `--z-text-muted` sobre `--z-bg` | 5.65 | 7.78 | 4.5 |
| `--z-text-muted` sobre `--z-surface` | 6.17 | 7.13 | 4.5 |
| `--z-text-muted` sobre `--z-surface-2` (encabezados) | 5.80 | 6.65 | 4.5 |
| `--z-text-muted` sobre `--z-hover` | 5.31 | 5.76 | 4.5 |
| `--z-text-muted` sobre `--z-danger-subtle` (fila atenuada) | 5.37 | 5.14 | 4.5 |
| `--z-text-muted` sobre `--z-warning-subtle` | 5.41 | 4.90 | 4.5 |
| `--z-accent` sobre `--z-surface` (enlaces) | 6.62 | 9.33 | 4.5 |
| `--z-accent` sobre `--z-bg` | 6.06 | 10.18 | 4.5 |
| `--z-accent` sobre `--z-brand-subtle` (elemento de menú actual) | 5.79 | 6.39 | 4.5 |
| `--z-on-primary` sobre `--z-primary` | 6.62 | 10.18 | 4.5 |
| `--z-on-primary` sobre `--z-primary-hover` | 8.82 | 11.26 | 4.5 |
| `--z-on-primary` sobre `--z-primary-active` | 11.12 | 12.26 | 4.5 |
| `--z-on-brand` sobre `--z-brand` (banda del nombre de la app) | 6.62 | 6.62 | 4.5 |
| `--z-danger` sobre `--z-surface` (errores de campo) | 6.54 | 7.09 | 4.5 |
| `--z-danger` sobre `--z-danger-subtle` | 5.69 | 5.11 | 4.5 |
| `--z-warning` sobre `--z-surface` | 6.33 | 8.90 | 4.5 |
| `--z-warning` sobre `--z-warning-subtle` | 5.55 | 6.12 | 4.5 |
| `--z-success` sobre `--z-surface` | 6.52 | 8.87 | 4.5 |
| `--z-info` sobre `--z-surface` | 6.92 | 8.13 | 4.5 |
| `--z-focus` sobre `--z-surface` | 6.62 | 9.33 | 3 |
| `--z-focus` sobre `--z-bg` | 6.06 | 10.18 | 3 |
| `--z-focus` sobre `--z-brand-subtle` | 5.79 | 6.39 | 3 |
| `--z-border-strong` sobre `--z-surface` (contorno de entrada) | 3.78 | 4.30 | 3 |
| `--z-border-strong` sobre `--z-surface-2` | 3.55 | 4.01 | 3 |

`--z-border` es decorativo (divisores entre elementos que ya tienen texto) y no se exige que alcance 3:1.

## 6. Otros comportamientos de accesibilidad

- Todo elemento interactivo muestra un anillo `:focus-visible` de 2 px en `--z-focus`. Las filas del grid dibujan el anillo dentro de la fila para que el contenedor de desplazamiento nunca lo recorte.
- Los botones, las entradas, los encabezados de orden, los cuadros de filtro, los botones del paginador y el botón de cierre de ventana miden al menos 24 x 24 px con cualquier densidad.
- `prefers-reduced-motion: reduce` elimina la entrada de los toasts y todas las transiciones.
- En el modo `forced-colors`, los iconos usan `CanvasText`, el botón primario y la fila con foco usan `Highlight`, y el estado que depende de tintes de fondo (elemento de menú actual, estado de fila) recibe en su lugar un contorno o borde.
