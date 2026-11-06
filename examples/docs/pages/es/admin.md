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
