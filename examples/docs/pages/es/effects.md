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

## Toasts

`ui.toast(text)` muestra un toast informativo. `ui.notify(Toast::success(text))` elige el tono: `Toast::info`, `success`, `warning` o `danger`. `.action(label, effect)` añade un botón que ejecuta otro effect, como abrir el registro que se guardó:

```rust
.on_success(|order, ui| {
    ui.notify(Toast::success(t!("orders.saved")).action(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(order.id) })));
})
```

Los toasts permanecen cuatro segundos (los de peligro, ocho), y el tiempo se detiene mientras el puntero está sobre un toast o este tiene el foco, de modo que se pueda llegar a sus botones. Los toasts info, success y warning se leen con cortesía mediante una región de estado; los toasts danger son alertas y se anuncian de inmediato. Cada toast tiene un botón para descartarlo, el tono también lo indican su barra de color y su texto, y se muestran como máximo tres toasts a la vez (`MAX_TOASTS`): uno nuevo elimina el más antiguo.

@live toasts
