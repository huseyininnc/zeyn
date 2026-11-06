# Componentes de tienda

Los listados y las páginas de producto necesitan algunos componentes propios: precios con descuento, filtros con recuentos, un control deslizante de rango, selectores de variante, una galería, selectores de cantidad, resúmenes de reseñas y cuentas atrás. Se renderizan en el servidor como cualquier otro componente. Los que necesitan código en el navegador cargan un módulo pequeño solo en las páginas que los contienen, y sus estilos (`commerce.css`) también se enlazan solo allí.

## Precio

```rust
Price::new(p.price)
    .was(p.list_price)
    .unit(p.price_per_kg, PriceUnit::Kg)
    .large()
```

- **Descuento.** `was(old)` tacha el precio anterior junto al actual y muestra el porcentaje de descuento redondeado a un número entero. Un precio anterior que no es mayor que el actual, o que está en otra moneda, se omite, así que un descuento caducado nunca muestra un porcentaje negativo.
- **Los lectores de pantalla** oyen una sola frase: "Antes ₺1,599.00, ahora ₺1,299.00, 19% de descuento". El precio tachado y la etiqueta se les ocultan, porque la mayoría de los lectores de pantalla no dicen que un texto está tachado.
- **Precio por unidad.** `unit(price, PriceUnit::Kg)` añade una línea más tenue como "₺259.80 por kg". Las unidades son `Item`, `Kg`, `Grams100`, `Liter`, `Ml100` y `Meter`.
- **Formato.** Los importes son `Money` en el formato numérico de la página: `₺1,299.00` en inglés, `1.299,00 ₺` en turco, y el signo de porcentaje va delante donde el idioma lo pone delante (`%19`). `large()` agranda el precio actual para una página de producto.
- **Sin script.** El precio es HTML simple.
