# Effects y ventanas

Los effects son lo que hace el navegador tras un clic o una acción correcta: abrir una ventana, ir a una pantalla, mostrar un toast, pedir confirmación, o cerrar una ventana y refrescar la pantalla que hay debajo. Se declaran en Rust y los ejecuta el runtime, así que no se escribe JavaScript.

## Los effects

| Effect | Se construye con | Resultado |
|---|---|---|
| Abrir una ventana | `Window::open(screen, params)` | Abre la pantalla en una ventana modal |
| Navegar | `Effect::navigate(screen, params)` | Va a la pantalla en el sitio, como un enlace |
| Toast | `Effect::Toast { message }` | Muestra un mensaje corto |
| Toast con tono | `Toast::success(message).action(label, effect).into()` | Un toast con un tono (info, success, warning, danger) y un botón opcional que ejecuta otro effect |
| Confirmar | `Effect::confirm(message, then)` | Ejecuta `then` después de que el usuario confirme |
| Cerrar y refrescar | `Effect::CloseAndRefresh` | Cierra la ventana y recarga la pantalla que la abrió |
| Refrescar topics | `Effect::refresh("orders")` o `ui.refresh(["orders", "stock"])` | Refresca en su sitio los nodos de la página que escuchan un topic (consulta [Topics](#topics)) |

Los params están tipados: un effect para una pantalla con params incorrectos no compila.

`Effect::navigate` a una pantalla cuya ruta tiene segmentos de ruta (consulta [Params in the path](screens.md)) llega a la forma con ruta, como `/orders/42`; los params que no están en la ruta van en la consulta `params`.

## Después de una acción

Los formularios y los botones de acción reciben un manejador `on_success`. Recibe el resultado de la acción y un `UiCtx` que recoge effects:

```rust
Form::of(product)
    .field(Product::name)
    .submit(t!("stock.save"), save_product)
    .on_success(|saved, ui| {
        if let Ok(p) = saved {
            ui.toast(t!("stock.saved", code = p.code));
        }
        ui.close_and_refresh();
    })
```

`UiCtx` tiene `toast`, `notify`, `close_and_refresh`, `navigate` y `effect` para cualquier otra cosa, como una confirmación:

```rust
ActionButton::new(t!("ui.check_stock"), check_stock, ()).on_success(|_, ui| {
    ui.effect(Effect::confirm(t!("ui.low_stock_confirm"), Effect::navigate(product_list, ())));
})
```

## Ventanas

Una ventana es una pantalla sin ruta, construida con `Screen::window(title)`. Las ventanas se apilan: una ventana puede abrir otra. Escape o el botón de cierre cierran la ventana superior y el foco vuelve al elemento que la abrió. `.width(px)` fija su ancho.

```rust
#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::name).submit(t!("stock.save"), save_product)))
}
```

## De dónde vienen los effects

- Enlaces de tabla y aperturas de fila: `.link(|p| Window::open(...))`, `.on_open(|p| ...)`.
- Botones de la barra de herramientas: `.opens(screen, params)`.
- Menús contextuales: `.item(label, effect)`.
- Comandos: `Command::open`, `Command::navigate`, `Command::effect`.
- Manejadores `on_success` de formularios y botones de acción.

Los effects nunca recargan la página. Una navegación carga solo la pantalla nueva; un refresco recarga solo la pantalla que hay debajo de la ventana.

## Topics

Cuando cambia una parte de una pantalla, otras partes suelen mostrar los mismos datos: un pedido guardado cambia el total, un filtro cambia el grid y el gráfico. Un topic las conecta sin que los datos pasen por el navegador. Marca los nodos que muestran los datos con `.listens(topic)`; se refrescan en su sitio cuando se nombra el topic.

```rust
Screen::new(t!("sales.title"))
    .body(Kpi::new(t!("sales.total"), total).listens("sales"))
    .body(Chart::bar(t!("sales.by_month")).labels(months).series(t!("sales.amount"), amounts).listens("sales"))
    .body(DataGrid::of(search_sales).column(Sale::amount).listens("sales"))
```

Un topic se nombra de tres maneras:

- `ui.refresh("sales")` (o `Effect::refresh`) en un manejador `on_success`, después de que una action cambió los datos.
- Un control de filtro que `.drives("sales")`: chips de filtro, un search box o un formulario sin botón de envío (selects, rangos de fechas, texto). Consulta [Formularios de filtro](forms.md#filter-forms).
- `hub.notify(&ctx, "sales")` desde el canal realtime (feature `realtime`), cuando otra persona cambió los datos. Consulta `docs/realtime.md`.

`.listens` funciona en cualquier componente: llámalo al final, después de los métodos builder del propio componente. Un listener que contiene un grid recarga su grid por el endpoint del grid y conserva la página, el orden y los filtros (un cambio desde un filtro empieza en la primera página). Cualquier otro listener se vuelve a renderizar en el servidor con un solo `POST /_zeyn/nodes` para todos, así que un KPI y un gráfico cuestan una petición. Solo viajan los nombres de los topics; el servidor abre la pantalla con el acceso y los params del usuario como en cualquier petición, así que un refresco nunca muestra lo que el usuario no puede ver.

Los topics nombrados dentro de un mismo frame de animación se procesan una vez, un refresco más nuevo cancela el que sigue en curso en la misma pantalla y solo se refrescan los nodos que están en la página. El foco y los `<details>` abiertos se quedan donde estaban cuando el contenido nuevo tiene el mismo elemento (por id, test id o la clave de fila de una lista).

Los nombres de topic tienen de 1 a 64 letras, dígitos, `-`, `_` o `.`. `App::check_page` informa de un topic que se conduce en una página donde nada lo escucha, y `zeyn check` avisa de un topic que se refresca o se conduce pero nunca se escucha, o que se escucha pero nunca se refresca, conduce ni notifica.

@live topics

## Toasts

`ui.toast(text)` muestra un toast informativo. `ui.notify(Toast::success(text))` elige el tono: `Toast::info`, `success`, `warning` o `danger`. `.action(label, effect)` añade un botón que ejecuta otro effect, como abrir el registro que se guardó:

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

Los toasts permanecen cuatro segundos (los de peligro, ocho), y el tiempo se detiene mientras el puntero está sobre un toast o este tiene el foco, de modo que se pueda llegar a sus botones. Los toasts info, success y warning se leen con cortesía mediante una región de estado; los toasts danger son alertas y se anuncian de inmediato. Cada toast tiene un botón para descartarlo, el tono también lo indican su barra de color y su texto, y se muestran como máximo tres toasts a la vez (`MAX_TOASTS`): uno nuevo elimina el más antiguo.

@live toasts
