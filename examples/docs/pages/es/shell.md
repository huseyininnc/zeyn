# Shell y comandos

El shell es el marco que rodea cada pantalla: el menú, los selectores de idioma, tema y zona horaria, los toasts y la paleta de comandos. Las aplicaciones que funcionan como herramientas de escritorio añaden una barra de menús, una caja de herramientas, un inspector de propiedades, una barra de estado y paneles acoplables.

## El menú

El menú sale de las llamadas `.menu(...)` de las features. Los elementos que el usuario no puede abrir se omiten del HTML, y una feature cuyos guards deniegan al usuario no muestra nada. El enlace de la pantalla actual lleva `aria-current="page"`.

## El shell del sitio

Las pantallas de trabajo se abren en el shell de la aplicación con su menú lateral. Las páginas públicas, como una tienda o un catálogo de productos, pueden abrirse en cambio en el shell del sitio: el nombre de la aplicación como enlace de marca, un menú horizontal, un botón de menú en pantallas estrechas, acciones opcionales y un pie de página. Una feature lo elige; las pantallas de otras features conservan el shell de la aplicación, así que ambos conviven en una misma aplicación.

```rust
Feature::new("shop")
    .screen(shop_home)
    .screen(shop_delivery)
    .screen(shop_returns)
    .menu(|m| {
        m.group(t!("shop.menu.home"), Icon::Home)
            .item(t!("shop.menu.home"), shop_home)
            .group(t!("shop.menu.help"), Icon::Box)
            .item(t!("shop.delivery.title"), shop_delivery)
            .item(t!("shop.returns.title"), shop_returns)
    })
    .shell(Shell::site().header(|h| h.sign_in().language().theme()).footer(|f| {
        f.column(t!("shop.menu.help"), |c| {
            c.link(t!("shop.delivery.title"), shop_delivery).url(t!("shop.footer.source"), "https://example.com")
        })
        .text(t!("shop.footer.note"))
    })
    .bottom_nav([
        BottomNavItem::new(t!("shop.menu.home"), shop_home).icon(Icon::Home),
        BottomNavItem::new(t!("shop.menu.help"), shop_delivery).icon(Icon::Box),
    ]))
```

- **Menú.** El menú superior lista los menús de todas las features en el shell del sitio. Un grupo de un solo elemento es un enlace simple; un grupo mayor es un botón que abre sus enlaces (el patrón APG de W3C de navegación con disclosure). Enter, Espacio o un clic alternan un grupo, Abajo lo abre y pasa a su primer enlace, Arriba, Abajo, Inicio y Fin se mueven entre sus enlaces, Escape lo cierra y vuelve al botón, y salir del grupo con Tab o hacer clic fuera lo cierra. Izquierda y Derecha se mueven entre los elementos superiores.
- **Pantallas estrechas.** Por debajo de 800 px el menú se pliega tras un botón de menú con `aria-expanded`; Escape lo cierra. Sin el script el menú permanece abierto, de modo que nada queda inalcanzable.
- **Acciones.** `sign_in()` muestra un enlace de inicio de sesión a los visitantes y un enlace de cierre de sesión a los usuarios con sesión iniciada cuando la aplicación tiene una página de inicio de sesión; `language()` y `theme()` muestran los mismos selectores que el shell de la aplicación.
- **Barra inferior.** `bottom_nav([...])` añade hasta cinco enlaces fijos en la parte inferior de la pantalla en teléfonos (800 px de ancho o menos), para los lugares a los que más se va; más de cinco no compila. Cada elemento es un `BottomNavItem` con una etiqueta, una pantalla con ruta y un icono opcional. El enlace de la página mostrada lleva `aria-current="page"` (lo mantiene al día `nav.js`), los enlaces a pantallas que el usuario no puede abrir se omiten, la barra se mantiene alejada del indicador de inicio con `env(safe-area-inset-bottom)` y la página deja espacio abajo para que la barra no oculte nada. El menú superior sigue para el resto. La barra no necesita script.
- **Pie de página.** Columnas de enlaces a pantallas (`link`, omitido para los usuarios que no pueden abrirlas) o a direcciones (`url`: una ruta, `https:`, `http:` o `mailto:`; `App::check` rechaza otros esquemas y los enlaces a pantallas que no están registradas), y una línea de texto.

El paso entre una pantalla del shell del sitio y una del shell de la aplicación se hace a través de `nav.js` sin cargar la página: la respuesta parcial dice a qué shell pertenece (`x-zeyn-shell`), y cuando eso difiere del marco actual, el runtime sustituye todo el marco como lo hace un cambio de idioma y carga la hoja de estilo y el script del otro shell. Los enlaces a `/_zeyn/route/{screen}` (los de las migas de pan) se siguen de la misma manera.

## Comandos

Un comando se declara una sola vez en Rust con su etiqueta, su atajo y lo que hace. Registrado en una feature, la paleta lo lista y su atajo funciona en todas las páginas.

```rust
pub fn new_product() -> Command {
    Command::open("stock.new_product", t!("pro.cmd_new_product"), product_editor, ProductParams { id: None })
        .shortcut("Ctrl+Alt+N")
        .icon(Icon::Plus)
}

pub fn restock() -> Command {
    Command::action("stock.restock_low", t!("pro.cmd_restock"), restock_low, ()).shortcut("Ctrl+Alt+R")
}

Feature::new("pro").command(new_product()).command(restock())
```

| Constructor | Ejecuta |
|---|---|
| `Command::open(id, label, screen, params)` | Abre una pantalla como ventana |
| `Command::navigate(id, label, screen, params)` | Va a una pantalla |
| `Command::effect(id, label, effect)` | Un effect en el navegador, como un toast |
| `Command::action(id, label, action, input)` | Una acción en el servidor; la entrada nunca viaja al navegador |

`App::check` informa de los atajos que no se pueden leer, que dos comandos comparten o que la paleta se reserva para sí.

## La paleta de comandos

**Alt+Shift+P** abre la paleta por defecto (`App::palette_shortcut` lo cambia, también a una tecla del navegador como `Ctrl+Shift+P` a propósito): un combobox ARIA que busca en los comandos de la aplicación, en la barra de herramientas y el menú contextual de la pantalla actual y en las pantallas del menú. Los comandos usados recientemente van primero.

Una feature puede añadirle una búsqueda de servidor. Mientras el usuario escribe, la paleta llama a la acción y lista sus resultados bajo los comandos; al elegir uno se abre la página en el sitio. Este sitio busca así en sus propias páginas:

```rust
#[action(public)]
pub async fn search_docs(query: String, docs: Docs) -> Result<Vec<SearchHit>> {
    Ok(docs.search(&query))
}

Feature::new("docs").search(search_docs)
```

El servidor ejecuta cada búsqueda que el usuario puede ejecutar con los guards de la feature, el permiso de la acción y los interceptores, recorta la consulta a 100 caracteres, descarta los resultados que salen de la aplicación y devuelve como máximo 20.

## Barra de menús, caja de herramientas y barra de estado

```rust
MenuBar::new(t!("pro.menubar"))
    .menu(t!("pro.menu_file"), |m| m.command(new_product()).separator().command(restock()))
    .menu(t!("pro.menu_view"), |m| m.panels())

Toolbox::new("Tools")
    .tool("select", "Select", Some(Icon::Search))
    .tool("draw", "Draw", Some(Icon::Edit))

StatusBar::new("Status").left("Ready").right("UTF-8")
```

@live shell

- `MenuBar` sigue el patrón ARIA menubar y contiene los mismos comandos que la paleta.
- `Toolbox` es un grupo vertical de botones de alternancia; la herramienta elegida se envía como un evento `zeyn:tool`.
- `PropertyInspector::new(title, form)` edita el objeto seleccionado con un formulario dividido en grupos `.section(...)`.
- `StatusBar` tiene una región en vivo, así que los mensajes que se le envían los leen los lectores de pantalla.

## Paneles acoplables

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

Los usuarios arrastran pestañas entre áreas, cambian el tamaño de las áreas con el puntero o el teclado y cierran paneles; `MenuList::panels` en un menú los recupera. El diseño se guarda por usuario y pantalla en el view store de la aplicación, o en el navegador cuando la aplicación no tiene ninguno.
