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
