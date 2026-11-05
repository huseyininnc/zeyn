# Pantallas y params

Una pantalla es una página con una ruta, o una ventana abierta sobre otra pantalla. `#[screen]` declara su acceso igual que una acción.

## Una página

```rust
#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products"))
        .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).shortcut("Alt+N").opens(product_editor, ProductParams { id: None }))
        .body(DataGrid::of(search_products).column(Product::code).column(Product::name).flex(1))
}
```

- `Screen::new(title)` es una página; `Screen::window(title)` es una ventana.
- Varias llamadas a `.body(...)` colocan varios componentes uno debajo de otro.
- Una pantalla devuelve `Screen` o `Result<Screen>`.

## Params

El primer argumento de una pantalla son sus params: un struct `#[params]`, o `_: ()` cuando no tiene. Los demás argumentos se inyectan, como en las acciones.

```rust
#[params]
pub struct ProductParams {
    pub id: Option<ProductId>,
}

#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
        None => Product::default(),
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(Form::of(product).field(Product::code).submit(t!("stock.save"), save_product)))
}
```

Los params están tipados en todos los sitios donde se usan: `Window::open(product_editor, 7)` no compila, porque el editor recibe `ProductParams`.

Cuando todos los campos de una estructura `#[params]` son `Option`, la pantalla también se abre sin parámetros, desde un enlace del menú o una dirección simple, con todos los campos en `None`. Un campo que no es opcional sigue necesitando su valor: sin él, la pantalla responde 404.

Una pantalla sin `route` no tiene URL. Se abre como ventana desde un enlace, un botón de la barra de herramientas, un menú o un comando.

## Params in the path

Una ruta puede nombrar campos de los params como segmentos de ruta. Cada `{name}` enlaza el campo del mismo nombre y se lee con el tipo de ese campo:

```rust
#[params]
pub struct OrderParams {
    pub id: OrderId,
    #[serde(default)]
    pub tab: Option<String>,
}

#[screen(route = "/orders/{id}", permission = "sales.order.read")]
pub fn order(p: OrderParams, store: OrderStore) -> Result<Screen> {
    let order = store.get(p.id).ok_or(Error::NotFound)?;
    Ok(Screen::new(t!("sales.order")).body(DescriptionList::new().item(t!("sales.customer"), order.customer)))
}
```

- `/orders/42` abre el pedido 42. Los campos que no están en la ruta viajan como antes en la consulta JSON: `/orders/42?params={"tab":"lines"}`.
- Los segmentos admiten enteros, claves de entidad (`OrderId`), `String` y `Slug`. Un valor solo se interpreta en la forma en que lo escribe el framework (`7`, no `007` ni `+7`); cualquier otra cosa responde 404.
- Un segmento que no nombra ningún campo de los params, una ruta con segmentos en una pantalla sin params, o un tipo de campo que no puede ser un segmento no compila, y el error dice qué cambiar.
- Varios segmentos funcionan de la misma manera: `#[screen(route = "/catalog/{slug}/{n}", public)]` con `slug: Slug` y `n: u32`.
- Los enlaces que escribe el framework usan la forma con ruta: `Effect::navigate`, las migas de pan, los comandos y `return_to` tras iniciar sesión. Las pantallas sin segmentos conservan `?params={json}`.
- Una pantalla con segmentos no puede ser un elemento de menú ni la pantalla de inicio, porque nada podría rellenarlos.
- El acceso se comprueba en cada petición, como en cualquier pantalla.

### Slugs

`Slug::from_title("Çay Bardağı")` es `cay-bardagi`: ASCII en minúsculas, palabras unidas por `-`, letras turcas convertidas (`ı` e `İ` a `i`, `ş` a `s`, `ğ` a `g`, `ç` a `c`, `ö` a `o`, `ü` a `u`), acentos de otras letras latinas eliminados y cualquier otra cosa omitida. Un slug tiene como máximo 80 bytes. En una ruta solo se interpreta la forma canónica, así que `/catalog/Cay-Bardagi/1` responde 404.

### No encontrado y movido

Devuelve `Err(Error::NotFound)` cuando el registro no está; el usuario recibe la página 404. Cuando la dirección cambió, por ejemplo después de renombrar un producto, responde con la nueva. El navegador recibe una redirección permanente (301) y la navegación dentro de la aplicación la sigue sin cargar la página:

```rust
#[screen(route = "/catalog/{slug}/{n}", public)]
pub fn catalog_page(p: CatalogParams, store: Catalog) -> Result<Screen> {
    let item = store.find(p.n).ok_or(Error::NotFound)?;
    if item.slug != p.slug {
        return Ok(Screen::moved_to(catalog_page, CatalogParams { slug: item.slug, n: p.n }));
    }
    Ok(Screen::new(item.name))
}
```

`Screen::redirect(screen, params)` envía una petición a otro screen, como una elección aleatoria o el elemento más reciente, mientras la dirección en sí sigue siendo válida. El navegador recibe `303 See Other` con el path canónico del destino y `Cache-Control: no-store`; la navegación dentro de la app lo sigue sin cargar la página, y la barra de direcciones muestra el destino. El destino es siempre un screen tipado con una ruta, de modo que una redirección nunca puede llevar a una dirección fuera de la app, y un destino que la app no sirve responde 404.

```rust
#[screen(route = "/random", public)]
pub async fn random_item(store: Catalog) -> Result<Screen> {
    let item = store.random().await?.ok_or(Error::NotFound)?;
    Ok(Screen::redirect(catalog_page, CatalogParams { slug: item.slug, n: item.n }))
}
```

@live redirect

## Acceso

| Opción | Significado |
|---|---|
| `permission = "..."` | La identidad necesita este permiso |
| `public` | Cualquiera. Los visitantes sin sesión reciben la página sin cookies (consulta [Páginas públicas](public-pages.md)) |
| `cache = 300` | Solo pantallas públicas: los navegadores y las cachés compartidas pueden guardar la página de un visitante sin sesión durante este número de segundos (como máximo 86400) |
| `sensitive` | Abrirla se registra como una lectura de datos personales |
| `reason` | El usuario debe dar un motivo antes de que se abra (consulta [Auditoría, enmascarado y jobs](enterprise.md)) |

Una página también informa sobre sí misma a los buscadores y a las vistas previas de enlaces con `.description(..)`, `.canonical(..)`, `.og_image(..)`, `.json_ld(..)` y `.robots(..)` en el `Screen`; consulta [Páginas públicas](public-pages.md#search-engines-and-link-previews).

Los elementos de menú, los botones de la barra de herramientas y los comandos que abren una pantalla que el usuario no puede abrir se omiten del HTML.

Una pantalla para visitantes que no han iniciado sesión declara `public`. Sus acciones necesitan su propio acceso: una pantalla pública que lista datos llama a una acción `public`, o la lista queda vacía para los visitantes.

```rust
#[action(public)]
pub async fn latest_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

#[screen(route = "/catalog", public)]
pub fn catalog() -> Screen {
    Screen::new(t!("catalog.title")).body(DataGrid::of(latest_products).column(Product::code).column(Product::price))
}
```

Omitir tanto `permission` como `public` es un error de compilación, así que una pantalla nunca queda abierta por accidente.

## Barra de herramientas y menú contextual

```rust
Screen::new(t!("stock.products"))
    .toolbar(|t| t.button(t!("stock.new_product")).icon(Icon::Plus).opens(product_editor, ProductParams { id: None }).export(Export::Csv))
    .context_menu(ContextMenu::new().item(t!("stock.new_product"), Window::open(product_editor, ProductParams { id: None })).action(t!("stock.restock_low"), restock_low, ()))
```

Los atajos como `Alt+N` funcionan en la pantalla y se muestran en la paleta. Los atajos que usa el navegador (`Ctrl+P`, `Ctrl+S`, `F5`) no se enlazan salvo que el botón llame a `.override_browser()`, y los que el navegador nunca deja pasar (`Ctrl+N`, `Ctrl+T`, `Ctrl+W`) nunca se enlazan; las combinaciones libres son `Alt+Shift+<letra>` y `Ctrl+Alt+<letra>`.

Los iconos de botones, grupos de menú y herramientas vienen de `Icon`: `Plus`, `Edit`, `Delete`, `Save`, `Search`, `Box`, `Cart`, `Table`, `Home`, `Chart`, `Users`, `Settings`, `Bell`, `Shield`, `Clock`, `Tag`, `Mail`, `SignOut`, `ArrowUp`, `ArrowDown`. Son máscaras en `zeyn.css` dibujadas en una cuadrícula de 24 px con un trazo redondeado de 2 px, toman el color del texto y se ocultan a los lectores de pantalla, así que la etiqueta siempre lleva el significado.

## Cabecera

`.subtitle(text)` pone una línea breve bajo el título, y `.primary(|t| ...)` coloca las acciones principales de la pantalla al final de la fila del título, como botones primarios. Juntos forman la cabecera de página que tienen casi todas las pantallas de administración.

```rust
Screen::new(t!("orders.title"))
    .subtitle(t!("orders.intro"))
    .primary(|t| t.button(t!("orders.new")).icon(Icon::Plus).shortcut("Alt+N").opens(order_editor, OrderParams { id: None }))
    .toolbar(|t| t.export(Export::Csv))
```

`.primary` acepta los mismos botones que `.toolbar`, con los mismos atajos y entradas de la paleta; la barra de herramientas sigue bajo la cabecera para las demás herramientas. El subtítulo es la descripción de la pantalla para los lectores de pantalla (`aria-describedby`). Si la fila es demasiado estrecha, las acciones pasan bajo el título, y en el teléfono se alinean al inicio. Sin subtítulo ni acciones principales, la pantalla se muestra como antes, con un título sencillo.

## Features y el menú

Una pantalla pertenece a una feature. El prefijo de ruta de la feature va antes de la ruta, sus guards se ejecutan antes de que se abra la pantalla y su menú lista las pantallas:

```rust
Feature::new("stock")
    .route_prefix("/stock")
    .guard(SignedIn)
    .screen(product_list)
    .screen(product_editor)
    .menu(|m| m.group(t!("menu.stock"), Icon::Box).item(t!("stock.products"), product_list))
```

`App::home(screen)` hace que `/` abra una pantalla.

## Páginas de error

`App::not_found(screen)` y `App::server_error(screen)` dan a la aplicación sus propias páginas para el 404 y para los errores de servidor:

```rust
#[screen(public)]
pub fn not_found_page() -> Screen {
    Screen::new(t!("errors.not_found")).body(EmptyState::new(t!("errors.not_found_text")))
}

App::new(t!("app.name")).not_found(not_found_page).server_error(server_error_page)
```

- La pantalla 404 responde a las rutas desconocidas y a toda pantalla que devuelve `Err(Error::NotFound)`, incluidos los valores de ruta que no se pueden interpretar.
- La pantalla de error de servidor responde a `Error::Internal`, a los tiempos de espera agotados (504) y a los panics en los manejadores. Los detalles del error van solo al registro del servidor; la pantalla no recibe params y no sabe nada de la petición.
- Ambas se muestran dentro del shell con el código de estado correcto. La navegación dentro de la aplicación recibe solo la pantalla y la sustituye sin cargar la página. Los endpoints del framework y la API siguen respondiendo JSON.
- Las pantallas de error no reciben params, no necesitan ruta y no forman parte de una feature. Hazlas `public`: un usuario que no puede abrir una, o una pantalla de error que falla ella misma, recibe el mensaje breve incorporado.

## Cómo llega una pantalla al navegador

La primera petición recibe la página completa: menú, pantalla y recursos. Los enlaces dentro de la aplicación cargan después solo la pantalla y la sustituyen en el sitio, de modo que el menú, el estado abierto y la posición de desplazamiento se mantienen. El botón de retroceso, las pestañas nuevas y los marcadores funcionan como con enlaces normales.
