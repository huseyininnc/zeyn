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

## Selector de variantes

```rust
VariantPicker::new()
    .group(
        VariantGroup::swatches("color", t!("shop.color"))
            .swatch("sand", t!("shop.sand"), "#d8c3a5")
            .swatch("navy", t!("shop.navy"), "#1f3a68")
            .current(p.color.as_deref()),
    )
    .group(
        VariantGroup::chips("size", t!("shop.size"))
            .option("m", "M")
            .option("xl", "XL")
            .unavailable("xl", t!("shop.not_in_color"))
            .current(p.size.as_deref()),
    )
    .drives("variant")
```

- **Grupos de radio.** Cada grupo es un fieldset de botones de radio nativos, el patrón radio group de W3C APG sin script: Tab llega a la opción elegida, las flechas se mueven y eligen, y la leyenda dice el grupo y la opción elegida ("Color Azul marino"). Las muestras aceptan un color hexadecimal o un token del tema; las fichas muestran su texto.
- **Las opciones no disponibles** se desactivan con un motivo (`unavailable(value, reason)`), que los lectores de pantalla leen con la opción y que muestra la descripción emergente; las muestras reciben una línea diagonal y las fichas un tachado.
- **Precio, foto y stock acompañan.** El selector impulsa sus temas y también los escucha. Elegir una opción pone su valor en el parámetro de pantalla con el nombre del grupo, mantiene la dirección al día, de modo que se puede compartir el enlace de la variante elegida, y refresca en una sola petición cada nodo que escucha el tema: pon `.listens("variant")` en el precio, la galería y la línea de stock, y la pantalla vuelve a desactivar opciones para la nueva elección. No necesita más script que el módulo de temas.
- **Comprobaciones.** `App::check_page` avisa de un selector que no impulsa nada y de una muestra sin un color válido.

## Galería de producto

```rust
p.photos.iter().fold(ProductGallery::new(p.name.clone()), |g, photo| {
    g.image(
        Image::new(photo.large.clone())
            .alt(photo.alt.clone())
            .size(800, 800)
            .low(photo.tiny.clone())
            .placeholder(Placeholder::color("--z-surface-2")),
    )
    .thumb(photo.thumb.clone())
})
```

- **Una imagen cada vez.** Las diapositivas están en una pista con ajuste de desplazamiento, así que en el móvil se pasa de una a otra deslizando, sin script. Las miniaturas bajo la imagen son botones ("Mostrar foto 2 de 5", la actual con `aria-current`); las flechas sobre la imagen con foco pasan de diapositiva en el sentido de lectura de la página. La galería es una región de carrusel con nombre cuyas diapositivas dicen "2 of 5" (patrón carousel de W3C APG). Sin miniaturas (`thumb`) muestra puntos numerados.
- **Solo carga lo que se ve.** La primera imagen se carga enseguida con `fetchpriority="high"`, porque suele ser lo más grande de una página de producto; las demás esperan en plantillas inertes y `gallery.js` pone la imagen en pantalla y sus dos vecinas, vigiladas por un único IntersectionObserver. Una galería de 12 fotos carga tres al principio. Las imágenes son `Image`, así que conservan su marcador de posición, su archivo de baja resolución y su alternativa.
- **Zoom y pantalla completa.** El ratón amplía la imagen bajo el puntero (el tamaño de la diapositiva se lee una vez al entrar el puntero, nunca mientras se mueve). Pulsar la imagen, o el botón "Pantalla completa", abre un diálogo modal con la imagen, botones de anterior y siguiente, un conmutador de zoom y un botón de cerrar; mantiene el foco dentro, se cierra con Escape y devuelve el foco a lo que lo abrió. Con movimiento reducido nada se desliza ni crece.
- **Script.** `gallery.js` (cerca de 1,5 KB con gzip) se carga solo en páginas con una galería.

## Panel de facetas

```rust
#[params]
#[derive(Default)]
pub struct Listing {
    #[serde(default)]
    pub color: Vec<String>,
    pub min_price: Option<i64>,
    pub max_price: Option<i64>,
}

#[screen(route = "/shop", public)]
pub async fn shop(p: Listing, store: ProductStore) -> Result<Screen> {
    let colors = store.count_colors(&p).await?;
    let panel = FacetPanel::of(shop, &p)
        .facet(colors.iter().fold(Facet::new("color", t!("shop.color")), |f, c| {
            f.option(c.value.clone(), c.name.clone(), c.count)
        })
        .chosen(p.color.clone()))
        .range(RangeSlider::new(t!("shop.price"), ["min_price", "max_price"], 0, 5_000)
            .money(Currency::Try)
            .value(p.min_price, p.max_price))
        .drives("products");
    let cards = CardGrid::of(search_products)
        .card(|p: &Product| Card::image(p.photo.clone(), p.name.clone()).price(Price::new(p.price)))
        .scope_any(Product::color, p.color.clone())
        .scope_range(Product::price, p.min_price.map(|m| Money::tl(m * 100)), p.max_price.map(|m| Money::tl(m * 100)))
        .listens("products");
    Ok(Screen::new(t!("shop.title")).body(Row::new().col(panel).col(cards).weight(3)))
}
```

- **Recuentos del servidor.** Cada faceta es un grupo de casillas cuyas opciones muestran cuántas filas tienen con las demás facetas aplicadas ("Rojo (124)"; los lectores de pantalla oyen "Rojo, 124 resultados"). La pantalla las cuenta con una consulta acotada por faceta, como `SELECT color, count(*) ... GROUP BY color LIMIT 50` con los demás filtros en el `WHERE`; una faceta admite como mucho `MAX_FACET_OPTIONS` (50) opciones. Una opción sin filas se desactiva salvo que esté elegida.
- **Una petición por cambio.** El panel es un formulario de filtro que impulsa sus temas y también los escucha. Un cambio pone los valores en los parámetros de pantalla (las facetas como listas, el rango como dos números), mantiene la dirección al día y refresca a todos los oyentes con un solo `POST /_zeyn/nodes`: la cuadrícula de tarjetas, una línea con el total y el propio panel con sus nuevos recuentos. Un cambio más nuevo cancela la petición que sigue en camino. Medido con 2.000 productos: 26 ms desde el clic hasta las nuevas tarjetas y recuentos.
- **Los filtros se quedan en el servidor.** La cuadrícula toma los mismos parámetros con `scope_any(field, values)` y `scope_range(field, from, to)` (en `CardGrid` y `DataGrid`), así que el navegador nunca envía filtros propios; sin valores o con extremos abiertos se conservan todas las filas.
- **Los filtros elegidos** aparecen como fichas sobre los grupos, con "Quitar todos". Cada ficha es un enlace a la pantalla sin ese filtro, así que también funciona sin script, y se llama "Quitar el filtro Rojo". Los grupos son `<details>` plegables, y los grupos abiertos siguen abiertos tras un refresco.
- **Comprobaciones.** `App::check_page` avisa de un panel que no impulsa nada y de una faceta cuyo nombre no es un campo de los parámetros de la pantalla.

## Reseñas y valoraciones

```rust
ReviewSummary::new()
    .count(5, stats.five)
    .count(4, stats.four)
    .count(3, stats.three)
    .count(2, stats.two)
    .count(1, stats.one)
```

- **Resumen.** `ReviewSummary` muestra la media como número y como estrellas, el número de reseñas y una barra por estrella de 5 a 1 con su proporción. La media y las proporciones salen de los recuentos, así que siempre concuerdan. Los lectores de pantalla oyen "4.6 de 5 estrellas" y "5 estrellas, 68%" en cada barra; las barras se les ocultan. Es HTML sin script.
- **Entrada de valoración.** `Control::Rating` en un campo de formulario es un grupo de radio dibujado como estrellas (W3C APG, sin script). Un campo entero admite estrellas enteras; un campo `Decimal` con `#[rule(min = 1, max = 5, scale = 1)]` admite medias estrellas, cada media un botón de radio llamado "4.5 of 5". El servidor lee el valor en el formato numérico de la página y rechaza un valor entre dos medias, como 4,3.

```rust
#[model]
pub struct Review {
    #[rule(required, min = 1, max = 5, scale = 1)]
    pub stars: Option<Decimal>,
    pub text: String,
}

Form::of(Review::default())
    .field(Review::stars)
    .control(Review::stars, Control::Rating)
    .field(Review::text)
    .submit(t!("reviews.send"), save_review)
```

## Cuenta atrás

```rust
Countdown::until(sale.ends_at)
    .label(t!("shop.sale_ends"))
    .ended(Badge::new(t!("shop.sale_over")))
```

- **Hora del servidor.** El servidor escribe el tiempo restante con su propio reloj ("2d 03:14:09"), y la página lleva ese reloj, así que un reloj equivocado en el dispositivo no mueve el final. Cuando el momento ha pasado, el servidor muestra el estado final dado a `ended` en lugar del reloj, sin script.
- **Un solo temporizador.** `countdown.js` (cerca de 1 KB con gzip, solo en páginas con una cuenta atrás en marcha) mantiene un temporizador para todas las cuentas atrás de la página, avanza en el segundo exacto y se detiene mientras la pestaña está oculta. Al llegar a cero el estado final sustituye al reloj; los oyentes de un tema pueden refrescar lo demás que cambia con el final.
- **Lectores de pantalla.** Los dígitos se les ocultan. Una región en vivo educada dice el tiempo restante con palabras ("Quedan 2 d 3 h 14 min") y solo cambia cuando cambia el minuto, así que nunca habla cada segundo.
