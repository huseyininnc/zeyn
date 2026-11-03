# Editor de líneas

Un campo `Vec<M>` de un modelo es una lista de líneas, como las líneas de una factura. `.lines(M::f, ...)` las edita como una pequeña tabla dentro del formulario: añadir, quitar y mover líneas, moverse entre celdas con el teclado y ver los totales mientras se escribe. El formulario entero, líneas incluidas, lo valida y lo guarda una sola acción.

```rust
#[model]
pub struct Line {
    #[rule(required, max_len = 80)]
    pub item: String,
    #[rule(min = 1, max = 999)]
    pub quantity: i64,
    #[rule(min = 0)]
    pub unit_price: Money,
}

#[model]
pub struct Invoice {
    #[rule(required, max_len = 80)]
    pub customer: String,
    #[rule(required)]
    pub lines: Vec<Line>,
}

Form::of(invoice)
    .field(Invoice::customer)
    .lines(Invoice::lines, |l| {
        l.column(Line::item)
            .column(Line::quantity)
            .column(Line::unit_price)
            .total(Line::quantity, Line::unit_price)
    })
    .submit(t!("docs.live.save"), save_invoice)
```

@live lines

## Columnas y totales

`.column(Line::f)` añade una columna editable con el control de su tipo. `.total(quantity, price)` muestra cantidad por precio para cada línea y la suma bajo las líneas, en la moneda del formulario.

## Validación

Cada línea se comprueba con las reglas de su modelo y los errores aparecen en la celda. `#[rule(required)]` en el campo `Vec` significa al menos una línea. El servidor limita el número de líneas (`MAX_LINES`), envíe lo que envíe el cliente.

## Guardado

La acción recibe `Valid<Invoice>`, así que cada línea ha pasado sus reglas antes de que se ejecute la acción:

```rust
#[action(permission = "sales.invoice.write", audit)]
pub async fn save_invoice(invoice: Valid<Invoice>) -> Result<Money> {
    Ok(Money::tl(invoice.lines.iter().map(|l| l.unit_price.minor * l.quantity).sum()))
}
```
