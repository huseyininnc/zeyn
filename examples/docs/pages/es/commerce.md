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


### Precios en tarjetas

`ImageCard::price(Price)` pone el mismo precio bajo el texto de la tarjeta en un `CardGrid` o un `Rail`, dentro del enlace de la tarjeta, así que los lectores de pantalla oyen juntos el título y la frase del precio. No añade script; la hoja de estilos de tienda llega con la primera tarjeta que tiene precio.

```rust
CardGrid::of(catalog_products).card(|p: &Product| {
    Card::image(p.photo.clone(), p.name.clone())
        .price(Price::new(p.price).was(p.list_price))
        .link(product_page, ProductPage { id: p.id })
})
```

## Control deslizante de rango

```rust
RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
    .step(50)
    .money(Currency::Try)
    .value(p.min_price, p.max_price)
    .drives("products")
```

- **Dos tiradores nativos.** Cada tirador es un `<input type="range">` con uno de los dos nombres, así que el navegador le da el comportamiento de slider de W3C APG: una parada de tabulación cada uno, las flechas avanzan un paso, Inicio y Fin van a los extremos, y los lectores de pantalla oyen el valor con formato (`aria-valuetext`, como "₺500.00") y la etiqueta ("Price, desde", "Price, hasta"). Re Pág y Av Pág mueven una décima parte del rango.
- **Un filtro.** `.drives(topic)` pone el control en un formulario de filtro: al soltar un tirador, los dos valores pasan a los parámetros de pantalla con los mismos nombres y los oyentes del tema se refrescan en su sitio. Dentro de un panel de facetas lo hace el formulario del panel, y en cualquier otro formulario los tiradores se envían como los demás campos.
- **Valores.** `value(low, high)` toma los parámetros (`None` conserva el extremo de la pista). Los valores fuera de los límites se recortan y un valor bajo mayor que el alto se intercambia, así que una dirección editada a mano no puede romper el control. `money(currency)` muestra unidades enteras de una moneda en el formato de la página.
- **Fechas.** `RangeSlider::dates(label, names, first, last)` con `.date_value(from, to)` elige días; el formulario envía fechas ISO como `2026-03-31` mediante campos ocultos.
- **Script.** `range.js` (cerca de 1 KB con gzip) se carga solo en páginas con un control de rango. Mantiene el tirador bajo por debajo del alto, rellena la pista entre ambos, escribe los valores junto a la etiqueta y mueve el tirador más cercano cuando se pulsa la pista vacía. Lee el tamaño de la pista una vez por pulsación y nunca mientras un tirador se mueve.

## Selector de cantidad

```rust
#[action(permission = "cart.edit")]
pub async fn set_quantity(change: QuantityChange, cart: CartStore) -> Result<QuantityState> {
    let stock = cart.stock(&change.target)?;
    let kept = change.quantity.min(stock);
    cart.set(&change.target, kept)?;
    Ok(QuantityState::new(kept).max(stock))
}

QuantityStepper::of(set_quantity, line.sku.clone())
    .label(t!("cart.quantity"))
    .range(1, 20)
    .value(line.quantity)
```

- **Comprobado en el servidor.** Un cambio (pulsar menos o más, o escribir un número) se envía 400 ms después de que el usuario se detiene, mediante `POST /_zeyn/data` con la comprobación CSRF y el límite de escritura, como el usuario y con el permiso de la acción comprobado. El servidor ajusta la cantidad al rango del selector antes de que se ejecute la acción, y la acción comprueba lo que solo ella sabe, como el stock. El artículo (`target`) lo fija la pantalla y nunca se lee del navegador.
- **Manda la respuesta.** La acción responde con la cantidad que guardó y el máximo permitido (`QuantityState::new(kept).max(stock)`). Si es menor que lo pedido, el selector la muestra y dice "Solo hay 7 disponibles." en una región en vivo. Un cambio más nuevo cancela la petición que sigue en camino, así que solo se muestra la última respuesta; una petición fallida repone la última cantidad guardada y lo indica.
- **Teclado y táctil.** Los botones miden al menos 24 por 24 píxeles (WCAG 2.5.8), se llaman "Reducir" y "Aumentar" y se desactivan en los extremos del rango; el foco pasa al campo cuando el botón pulsado se desactiva. El campo acepta dígitos y las flechas.
- **Script.** `quantity.js` (cerca de 1 KB con gzip) se carga solo en páginas con un selector.
