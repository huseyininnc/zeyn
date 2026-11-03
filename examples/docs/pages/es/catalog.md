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
- **Paginación.** `Paging::infinite(n)` (el valor por defecto, 24) muestra un botón "Show more" que también se activa cuando entra en la vista al desplazarse; el foco pasa a la primera tarjeta nueva cuando se pulsa el botón. `Paging::pages(n)` muestra los botones anterior y siguiente y el número de página. El servidor fija el tamaño de página; el cliente solo indica dónde continuar.
- **Ordenación.** Cada `sort_option` es un orden que los usuarios pueden elegir en un select; el primero es el orden con el que empiezan las tarjetas. El servidor acepta solo estos órdenes y descarta los filtros y el texto de búsqueda que envíe el cliente.
- **Búsqueda.** `.search(text)` muestra solo las filas que coinciden con el texto libre ([`DataQuery::q`](#free-text)), como el texto de una pantalla de resultados.
- **Campos enmascarados.** Las tarjetas las construye un closure que ve la fila sin procesar. Para un modelo con campos `#[mask]`, solo los usuarios que pueden revelarlos reciben las tarjetas; todos los demás reciben `Forbidden`. Usa un modelo sin campos enmascarados para las tarjetas públicas.

## Rail

Un rail es un card grid dispuesto en una sola fila que se desplaza lateralmente, con ajuste al desplazarse y botones anterior y siguiente que se desplazan el ancho de una vista (con suavidad, salvo que el usuario haya pedido movimiento reducido). Carga el siguiente lote, de 12 en 12, cuando el usuario se desplaza cerca del final. Las tarjetas son enlaces en el orden de tabulación y se desplazan hasta quedar a la vista cuando reciben el foco.

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

## Texto libre

`DataQuery::q` es texto libre: filas cuyos campos de texto lo contienen, sin distinguir mayúsculas de minúsculas. La `i` turca con punto y sin punto (`İ`, `ı`) coincide con `i`, así que "istanbul" encuentra "İSTANBUL". Los campos con `#[mask]` nunca se buscan, un texto en blanco coincide con todas las filas y un texto de más de 100 caracteres se ignora. `DataQuery::apply` y las capas de datos SQL (`LOWER(TRANSLATE(column, 'İı', 'ii')) LIKE ...` con parámetros enlazados) lo leen de la misma manera.
