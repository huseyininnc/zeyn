# Tarjetas, rails y búsqueda

Las páginas públicas, como una tienda, muestran las filas como tarjetas en lugar de una tabla. `CardGrid`, `Rail` y `SearchBox` cargan sus filas mediante el mismo tipo de acción que usa un DataGrid: recibe un `DataQuery` y devuelve un `Page`. La paginación, la ordenación y el texto de búsqueda se ejecutan en el servidor, y la lectura necesita el acceso de la acción, como en un DataGrid.

```rust
#[action(public)]
pub async fn catalog_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?.into_iter().filter(|p| p.active).collect()))
}
```

## Card grid

```rust
CardGrid::of(catalog_products)
    .title(t!("shop.products"))
    .card(|p: &Product| {
        Card::image(p.image.clone(), p.name.clone())
            .text(p.price.to_string())
            .badge(Badge::new(t!("shop.new")).tone(Tone::Success))
            .link(shop_product, ProductPage { id: p.id })
    })
    .sort_option(t!("shop.sort.name"), Product::name, false)
    .sort_option(t!("shop.sort.cheapest"), Product::price, false)
    .image_size(300, 300)
    .paging(Paging::infinite(24))
```

- **Tarjetas.** `Card::image(image, title)` crea una tarjeta a partir de una fila, con `text`, `badge`, `alt` y `link` (una pantalla y sus params; toda la tarjeta es un enlace y se abre sin recargar la página). Las imágenes son diferidas, con el ancho y el alto de `image_size` para que la página no salte. El texto alternativo de la imagen está vacío, porque el título que hay junto a ella da nombre a la tarjeta; define `alt` cuando la imagen diga más. Las direcciones de imagen que no sean una ruta, `https:` o `data:image/` se omiten.
- **Más en una tarjeta.** `subtitle` añade una segunda línea más tenue bajo el título (un autor, una categoría). `corner(badge)` coloca una insignia sobre la esquina final de la imagen, junto a `badge` en la inicial; `badges([...])` añade una fila de insignias bajo el texto, como etiquetas. `progress(fraction, label)` dibuja una barra fina a lo largo del borde inferior de la imagen para indicar cuánto está hecho: la fracción va de 0 a 1, y los lectores de pantalla oyen la etiqueta y el porcentaje (`role="progressbar"` con `aria-valuenow`). `horizontal()` dispone la tarjeta en una fila, con una imagen pequeña al principio y el texto a su lado; una cuadrícula de tarjetas o un rail con tarjetas así les da columnas más anchas. Todo ello funciona en un `CardGrid` y en un `Rail`.

```rust
Card::image(c.cover.clone(), c.title.clone())
    .subtitle(c.teacher.clone())
    .corner(Badge::new(t!("courses.new")).tone(Tone::Success))
    .badges(c.tags.iter().map(|t| Badge::new(t.clone())))
    .progress(c.done, t!("courses.done"))
    .horizontal()
```
- **Primera página en el servidor.** La primera página (24 tarjetas en una cuadrícula, 12 en un rail, o el tamaño indicado en `paging`) forma parte del HTML de la página, así que los buscadores y los visitantes sin JavaScript ven las tarjetas y sus enlaces, y las imágenes llevan su tamaño para que nada se mueva al cargarse. Las páginas siguientes llegan desde el navegador. Una página pública es el mismo HTML para todos los visitantes, así que puede almacenarse en caché. Una tarjeta enlazada a una screen con ruta enlaza a su dirección canónica (`/items/{slug}`), nunca a una redirección.
- **Paginación.** `Paging::infinite(n)` (el valor por defecto, 24) muestra un botón "Show more" que también se activa cuando entra en la vista al desplazarse; el foco pasa a la primera tarjeta nueva cuando se pulsa el botón. `Paging::pages(n)` muestra los botones anterior y siguiente y el número de página. El servidor fija el tamaño de página; el cliente solo indica dónde continuar.
- **Ordenación.** Cada `sort_option` es un orden que los usuarios pueden elegir en un select; el primero es el orden con el que empiezan las tarjetas. El servidor acepta solo estos órdenes y descarta los filtros y el texto de búsqueda que envíe el cliente.
- **Búsqueda.** `.search(text)` muestra solo las filas que coinciden con el texto libre ([`DataQuery::q`](#free-text)), como el texto de una pantalla de resultados.
- **Campos enmascarados.** Las tarjetas las construye un closure que ve la fila sin procesar. Para un modelo con campos `#[mask]`, solo los usuarios que pueden revelarlos reciben las tarjetas; todos los demás reciben `Forbidden`. Usa un modelo sin campos enmascarados para las tarjetas públicas.

### Vistas previas al pasar el cursor

`.preview(screen, params)` en una `ImageCard` o un `ListItem` que tenga enlace muestra esa pantalla en un panel junto a la tarjeta: tras aproximadamente medio segundo con el ratón encima, o de inmediato cuando la tarjeta recibe el foco del teclado. La pantalla se carga solo entonces, desde `GET /_zeyn/screen/...` con las mismas comprobaciones de acceso que una ventana, y cada vista previa se conserva mientras dure la página (24 como máximo, se vacían cuando `nav.js` abre otra pantalla). Los marcos, objetos, reproductores multimedia y scripts se eliminan antes de mostrar el panel, así que una vista previa nunca carga un iframe. El panel vive en la capa superior, de modo que los rails y las cuadrículas que recortan su contenido no lo cortan; se abre por el lado con más espacio, se voltea en los bordes de la pantalla, se desplaza dentro de sí mismo y nunca desplaza la página. Sigue WCAG 1.4.13: el puntero puede pasar sobre el panel, Escape lo cierra y permanece hasta que el puntero o el foco se van. En pantallas táctiles y con punteros imprecisos no hay vista previa y un toque abre el enlace; el script (`preview.js`, 2,5 KB gzip) nunca se carga allí, y en el resto se carga con el primer hover o foco de una tarjeta así. Con movimiento reducido, el panel aparece sin su breve animación de crecimiento.

```rust
Rail::of(search_products).card(|p: &Product| {
    Card::image(p.image.clone(), p.name.clone())
        .link(product, ProductParams { id: p.id })
        .preview(product_summary, ProductParams { id: p.id })
})
```

@live preview


## Rail

Un rail es un card grid dispuesto en una sola fila que se desplaza lateralmente, con ajuste al desplazarse y botones anterior y siguiente que se desplazan el ancho de una vista (con suavidad, salvo que el usuario haya pedido movimiento reducido). Carga el siguiente lote, de 12 en 12, cuando el usuario se desplaza cerca del final. Las tarjetas son enlaces en el orden de tabulación y se desplazan hasta quedar a la vista cuando reciben el foco.

La barra de desplazamiento está oculta; la pantalla táctil, el trackpad y Mayús+rueda siguen desplazando, y `.scrollbar(true)` deja una barra fina. Con el ratón se puede pulsar y arrastrar el rail hacia los lados. Tras un movimiento de más de 5 píxeles, el clic al soltar se cancela, así que la tarjeta bajo el puntero no se abre y el rail se detiene en la tarjeta más cercana; un movimiento menor es un clic normal. Mientras se arrastra no se selecciona texto, las imágenes no se arrastran fuera y no se abre la vista previa de la tarjeta. En pantallas táctiles se mantiene el desplazamiento propio del navegador. Los botones anterior y siguiente siempre están y se desactivan en los extremos, porque una barra oculta no sirve para desplazarse con el teclado. Sin JavaScript los botones se ocultan y el rail se desplaza de forma nativa.

```rust
Rail::of(catalog_products)
    .title(t!("shop.new_arrivals"))
    .card(|p: &Product| Card::image(p.image.clone(), p.name.clone()))
    .sort_option(t!("shop.sort.newest"), Product::id, true)
```

## Search box

Un campo de búsqueda con sugerencias, que sigue el patrón combobox de W3C APG con una ventana emergente de lista. Mientras el usuario escribe, espera `instant` milisegundos y pide sugerencias al servidor: la acción se ejecuta con el texto como `DataQuery::q`, y vuelve el texto de sugerencia de cada fila (su primer campo de texto sin enmascarar, salvo que `suggestion` indique otra cosa). Enter, o elegir una sugerencia, abre la pantalla de resultados con el texto, sin recargar la página.

```rust
#[screen(route = "/shop/search", public)]
pub fn shop_search(p: SearchParams) -> Screen {
    Screen::new(t!("shop.search.all"))
        .body(SearchBox::of(catalog_products).value(p.q.clone()).results(shop_search))
        .body(CardGrid::of(catalog_products).card(product_card).search(p.q).paging(Paging::pages(12)))
}

SearchBox::of(catalog_products)
    .label(t!("shop.search.label"))
    .instant(150)
    .suggestion(|p: &Product| p.name.clone())
    .results(shop_search)
```

Teclado: Abajo y Arriba se mueven por las sugerencias (Abajo las abre), Enter busca con la sugerencia resaltada o con el texto escrito, Escape cierra las sugerencias y después borra el campo. Una región en vivo indica a los lectores de pantalla cuántas sugerencias hay.

La pantalla de resultados recibe `SearchParams` (`q`, vacío cuando la pantalla se abre desde un menú). El servidor recorta el texto a `MAX_SEARCH_QUERY` (100) caracteres, decide cuántas sugerencias enviar (8 salvo que `limit` indique otra cosa, como máximo 20), comprueba el acceso de la pantalla, los guards de la feature y el permiso de la acción, y cuenta cada llamada de sugerencias contra el límite de escritura (`App::write_limit`).

### Sugerencias enriquecidas

`.item(|row| ListItem::new(title).subtitle(text).image(src).link(screen, params))` muestra cada sugerencia como una fila con una imagen pequeña (40 por 56 píxeles, de carga diferida, sujeta a las reglas de `img-src`, con texto alternativo vacío porque el título nombra la fila; sin marcador de posición si no hay imagen), el título y el subtítulo. El servidor responde `[{text, subtitle, image, href}]` con la dirección canónica del registro. Elegir una fila con un clic o con Enter abre su enlace mediante `nav.js`, y como cada fila es un enlace real, funcionan el clic central y «abrir en una pestaña nueva». Enter sin ninguna fila resaltada sigue abriendo la pantalla de resultados. El patrón combobox no cambia: cada fila es una opción nombrada por su título y su subtítulo, y las filas miden al menos 44 píxeles de alto. Sin `.item` las sugerencias son texto simple, como arriba.

```rust
SearchBox::of(catalog_products)
    .item(|p: &Product| {
        ListItem::new(p.name.clone())
            .subtitle(p.code.clone())
            .image(p.image.clone())
            .link(shop_product, ProductPage { id: p.id })
    })
    .results(shop_search)
```

## Texto libre

`DataQuery::q` es texto libre: filas cuyos campos de texto lo contienen, sin distinguir mayúsculas de minúsculas. La `i` turca con punto y sin punto (`İ`, `ı`) coincide con `i`, así que "istanbul" encuentra "İSTANBUL". Los campos con `#[mask]` nunca se buscan, un texto en blanco coincide con todas las filas y un texto de más de 100 caracteres se ignora. `DataQuery::apply` y las capas de datos SQL (`LOWER(TRANSLATE(column, 'İı', 'ii')) LIKE ...` con parámetros enlazados) lo leen de la misma manera.
