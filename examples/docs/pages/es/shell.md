# Shell y comandos

El shell es el marco que rodea cada pantalla: el menú, los selectores de idioma, tema y zona horaria, los toasts y la paleta de comandos. Las aplicaciones que funcionan como herramientas de escritorio añaden una barra de menús, una caja de herramientas, un inspector de propiedades, una barra de estado y paneles acoplables.

## El menú

El menú sale de las llamadas `.menu(...)` de las features. Los elementos que el usuario no puede abrir se omiten del HTML, y una feature cuyos guards deniegan al usuario no muestra nada. El enlace de la pantalla actual lleva `aria-current="page"` y se dibuja sobre el tinte sutil de la marca.

`.icon(Icon::...)` después de `.item(...)` pone un icono antes de la etiqueta del elemento; cuando los elementos de un grupo tienen iconos, la etiqueta del grupo se dibuja sin el suyo. El menú lateral empieza con el logotipo y el nombre de la aplicación: `App::logo("/static/logo.svg")` (y `App::logo_dark(...)` para el tema oscuro) establece una marca servida por la aplicación; si no, el shell dibuja un cuadrado redondeado en `--z-brand` con la primera letra del nombre. El shell se mantiene neutro; el color de marca señala el elemento actual, los botones primarios, el anillo de foco y el logotipo.

Los iconos vienen de un único conjunto de 82 dibujos (`Icon::ALL`) en una cuadrícula de 20 px con un trazo redondeado de 1.5 px, mostrados a 16 o 20 px en el color del texto actual. Cada uno es una máscara CSS en `/_zeyn/icons.css`, una hoja de estilo en caché de unos 25 KB (3 KB comprimida) generada a partir de `crates/zeyn/assets/icons/*.svg`, así que un icono cuesta una clase en el HTML y nada en script.

```rust
Feature::new("sales")
    .screen(orders)
    .menu(|m| m.group(t!("sales.menu"), Icon::Cart).item(t!("sales.orders"), orders).icon(Icon::Table))
```

### Grupos anidados y plegables

`.sub(label, |m| ...)` añade un grupo anidado al último grupo; dentro, `item`, `icon`, `open` y `sub` funcionan como en un grupo. Los grupos anidados van después de los elementos de su grupo y bajan como mucho tres niveles por debajo de un grupo de primer nivel (`MENU_DEPTH`); un menú más profundo detiene la aplicación al arrancar con un mensaje que nombra el grupo.

```rust
.menu(|m| {
    m.group(t!("sales.menu"), Icon::Cart)
        .item(t!("sales.orders"), orders)
        .sub(t!("sales.reports"), |m| m.item(t!("sales.by_city"), by_city).item(t!("sales.archive"), archive))
        .accordion()
})
```

- **Plegables.** Cada encabezado de grupo es un botón con `aria-expanded` y `aria-controls` (el patrón de navegación por divulgación de W3C APG). El servidor dibuja abiertos los grupos que contienen la pantalla actual y cerrados los demás, y `nav.js` abre la rama de la nueva pantalla tras cada navegación. Sin el script todos los grupos quedan abiertos, así que nada queda fuera de alcance.
- **Siempre abierto.** `.open()` después de un grupo lo mantiene abierto: su encabezado es texto sin botón.
- **Acordeón.** `.accordion()` en el menú de cualquier feature del app shell convierte todo el menú lateral en un acordeón: abrir un grupo cierra sus hermanos abiertos.
- **Teclado.** Arriba y Abajo recorren los encabezados y enlaces visibles, Inicio y Fin van al primero y al último. Derecha abre un grupo cerrado o entra en uno abierto; Izquierda cierra un grupo abierto o va al encabezado del grupo que lo rodea. Enter y Espacio abren o cierran un grupo. Izquierda y Derecha se intercambian en los idiomas de derecha a izquierda.
- **Permisos y fusión.** Los grupos con la misma etiqueta se fusionan entre features en todos los niveles, y un grupo cuyos elementos el usuario no puede abrir se omite, incluidos los grupos anidados. La ruta de navegación de `App::header(|h| h.breadcrumb())` muestra la ruta completa de grupos. El site shell lista los enlaces de los grupos anidados en su grupo de primer nivel.

### Riel de iconos, insignias, filtro y teléfonos

```rust
.menu(|m| {
    m.group(t!("mail.menu"), Icon::Mail)
        .item(t!("mail.inbox"), inbox)
        .count(unread)
        .count_listens("mail")
        .item(t!("mail.rules"), rules)
        .badge(t!("menu.new"))
})
```

- **Riel de iconos.** En pantallas desde 1024 px, un botón en la barra del menú lo pliega a los iconos de sus grupos de primer nivel (o su primera letra). El nombre de un grupo aparece como información emergente y el grupo se abre como un panel junto al riel; Escape, un clic fuera y una navegación lo cierran. La elección se guarda en la cookie `zeyn_rail`, así la página siguiente se dibuja igual sin saltos.
- **Insignias y contadores.** `.badge(text)` tras un elemento añade un texto fijo corto como "Nuevo". `.count(action)` añade un número contado por una action que recibe `()` y devuelve `u64`: la página pide todos los contadores de su menú en una sola petición después de mostrarse (`GET /_zeyn/menu/counts`), así un conteo lento nunca retrasa la página. Los usuarios sin el permiso de la action no ven número, 0 no muestra nada y más de 99 muestra "99+". `.count_listens(topics)` vuelve a contar cuando una action nombra uno de los temas con `ui.refresh(..)` o el socket en vivo lo notifica.
- **Filtro y favoritos.** Un menú lateral con 10 o más enlaces (`MENU_FILTER_FROM`) recibe un cuadro de filtro: al escribir oculta los enlaces que no coinciden (sin distinguir mayúsculas ni acentos), abre los grupos de los que sí y dice cuántos quedan; Escape lo vacía. Cada enlace recibe un botón para fijarlo, y los enlaces fijados aparecen en un grupo Favoritos arriba, guardados en el navegador por usuario.
- **Teléfonos.** Por debajo de 1024 px el menú se pliega tras un botón de su barra y se abre como un cajón sobre la pantalla. El foco entra en el cajón y queda entre él y el botón, la pantalla de detrás queda inerte y no se desplaza, y el botón, Escape, el fondo, un enlace o un deslizamiento hacia el borde del menú lo cierran y devuelven el foco al botón.

### Menús construidos en cada petición

`App::menu_source(source)` añade entradas de menú que se construyen en cada petición, por ejemplo a partir de una tabla de menú filtrada por los roles del usuario. Una fuente es una función async que recibe el `Ctx` (identidad, tenant, idioma) y devuelve `MenuNode`s: `MenuNode::group(label, children)` en el nivel superior, con enlaces `MenuNode::screen(label, "screen_name")` (`.params(json)` rellena los segmentos de la ruta y la consulta, como hace `screen_url`), enlaces `MenuNode::path(label, "/reports/sales")` y grupos anidados dentro. `.icon(...)`, `.open()` y `.permission("menu.reports")` funcionan en cualquier nodo. Una etiqueta es texto plano, escapado como cualquier otro texto, o una clave del catálogo con `Text::key`.

Los grupos van después de los menús estáticos de las features, y un grupo con la etiqueta de un grupo estático se fusiona con él. La ruta de navegación, la paleta de comandos y `nav.js` tratan sus enlaces como cualquier otro enlace del menú.

La tabla de abajo funciona en PostgreSQL y en Oracle. Un único `SELECT` plano la lee, y el árbol se construye en Rust por `parent_id`:

```sql
CREATE TABLE menu_items (
    id          INTEGER PRIMARY KEY,
    parent_id   INTEGER REFERENCES menu_items (id),
    label       VARCHAR(100) NOT NULL,
    screen      VARCHAR(100),
    path        VARCHAR(500),
    icon        VARCHAR(40),
    permission  VARCHAR(100),
    sort_order  INTEGER DEFAULT 0 NOT NULL
);
```

```rust
#[model]
pub struct MenuRow {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub label: String,
    pub screen: Option<String>,
    pub path: Option<String>,
    pub icon: Option<String>,
    pub permission: Option<String>,
    pub sort_order: i64,
}

async fn menu(ctx: Ctx) -> Result<Vec<MenuNode>> {
    let db = ctx.service::<Db>().ok_or_else(|| Error::Internal("no database".into()))?;
    let rows: Vec<MenuRow> = db
        .query("SELECT id, parent_id, label, screen, path, icon, permission, sort_order FROM menu_items ORDER BY sort_order, id", vec![])
        .await?;
    Ok(tree(&rows, None))
}

fn tree(rows: &[MenuRow], parent: Option<i64>) -> Vec<MenuNode> {
    rows.iter()
        .filter(|r| r.parent_id == parent)
        .map(|r| {
            let mut node = match (&r.screen, &r.path) {
                (Some(screen), _) => MenuNode::screen(r.label.clone(), screen.clone()),
                (None, Some(path)) => MenuNode::path(r.label.clone(), path.clone()),
                (None, None) => MenuNode::group(r.label.clone(), tree(rows, Some(r.id))),
            };
            if let Some(icon) = Icon::ALL.iter().find(|i| Some(i.name()) == r.icon.as_deref()) {
                node = node.icon(*icon);
            }
            match &r.permission {
                Some(p) => node.permission(p.clone()),
                None => node,
            }
        })
        .collect()
}

App::new("Admin").menu_source(menu)
```

- **Nunca se confía en ella.** El servidor comprueba cada enlace en cada petición: los enlaces a screens desconocidas se descartan con un aviso en el log, los enlaces a screens que el usuario no puede abrir (acceso, guards de la feature, el `permission` propio del nodo) se descartan aunque la fuente los haya devuelto, y lo mismo ocurre con los enlaces a screens cuyos segmentos de ruta no rellenan los params. Una ruta debe empezar con una sola `/`: sin esquema, sin `//`, sin barra invertida y sin caracteres de control. Los grupos se anidan como mucho `MENU_DEPTH` niveles, la respuesta de una fuente conserva como mucho `MAX_MENU_NODES` (500) nodos y las etiquetas se cortan en `MAX_MENU_LABEL` (100) caracteres.
- **En caché.** Las respuestas se guardan en caché durante `MENU_TTL` (60 segundos) por tenant, idioma y conjunto de permisos, así que los usuarios con los mismos roles comparten una respuesta y la fuente se ejecuta una vez por minuto para ellos. La caché guarda como mucho `MAX_MENU_CACHE` (256) respuestas y descarta la más antigua. Una fuente cuya respuesta depende del usuario, y no solo de los permisos, necesita `App::menu_per_user()`. Después de que una action edite la tabla, llama a `invalidate()` en el servicio `Menus` inyectado.
- **Nunca rompe la página.** Cuando la fuente devuelve un error, entra en pánico o tarda más que `MENU_TIMEOUT` (2 segundos), la página muestra el menú estático y el servidor registra el fallo. Los fallos no se guardan en caché.
- **Dónde aparece.** El menú pertenece al app shell: las cargas completas de screens y las páginas de error lo incluyen, mientras que el shell del sitio y las páginas de inicio de sesión muestran solo los menús estáticos.

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
    .shell(Shell::site().header(|h| {
        h.center(SearchBox::of(search_products).results(shop_search))
            .center_on_phone(CenterOnPhone::Icon)
            .end(LinkButton::new(t!("shop.cart"), shop_cart, ()))
            .sign_in()
            .language()
            .theme()
    })
    .footer(|f| {
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
- **Pantallas estrechas.** Por debajo de 1024 px el menú se pliega tras un botón de menú con `aria-expanded`; Escape lo cierra. Sin el script el menú permanece abierto, de modo que nada queda inalcanzable.
- **Ranuras de la cabecera.** La barra superior tiene cuatro ranuras que aceptan cualquier componente, como una app bar con nombres lógicos: `start(node)` antes de la marca, `center(node)` entre el menú y el final, `end(node)` al final de la barra y `bottom(node)` como una fila de ancho completo debajo de ella, por ejemplo con chips. Cada ranura contiene varios nodos en orden, y start y end cambian de lado en los idiomas de derecha a izquierda. Los componentes en las ranuras se renderizan con el idioma, los permisos y las máscaras del usuario; los que cargan datos mediante su propia action, como un `SearchBox`, funcionan ahí, comprobados contra los guards de la feature y el permiso de la action. Los grids y los formularios pertenecen a la pantalla. En los teléfonos las ranuras start y end se quedan en la barra, y la ranura center va donde la aplicación indique: `center_on_phone(CenterOnPhone::Row)` (el valor por defecto) la coloca en una fila bajo la barra, `CenterOnPhone::Icon` tras un botón de lupa que la abre (disclosure de W3C APG) y mueve el foco a ella; Escape la cierra y vuelve al botón. `center_button(icon, label)` cambia ese botón. La disposición en teléfono la fija el CSS antes de que se ejecute el script, así que la cabecera no se mueve mientras se carga la página.
- **Acciones.** `sign_in()` muestra un enlace de inicio de sesión a los visitantes y un enlace de cierre de sesión a los usuarios con sesión iniciada cuando la aplicación tiene una página de inicio de sesión; `language()` y `theme()` muestran los mismos selectores que el shell de la aplicación. Van después de la ranura end y se pliegan en el menú en los teléfonos.
- **Barra inferior.** `bottom_nav([...])` añade hasta cinco enlaces fijos en la parte inferior de la pantalla en teléfonos y tabletas (con menos de 1024 px de ancho), para los lugares a los que más se va; más de cinco no compila. Cada elemento es un `BottomNavItem` con una etiqueta, una pantalla con ruta y un icono opcional. El enlace de la página mostrada lleva `aria-current="page"` (lo mantiene al día `nav.js`), los enlaces a pantallas que el usuario no puede abrir se omiten, la barra se mantiene alejada del indicador de inicio con `env(safe-area-inset-bottom)` y la página deja espacio abajo para que la barra no oculte nada. El menú superior sigue para el resto. La barra no necesita script.
- **Pie de página.** Columnas de enlaces a pantallas (`link`, omitido para los usuarios que no pueden abrirlas) o a direcciones (`url`: una ruta, `https:`, `http:` o `mailto:`; `App::check` rechaza otros esquemas y los enlaces a pantallas que no están registradas), y una línea de texto.

El paso entre una pantalla del shell del sitio y una del shell de la aplicación se hace a través de `nav.js` sin cargar la página: la respuesta parcial dice a qué shell pertenece (`x-zeyn-shell`), y cuando eso difiere del marco actual, el runtime sustituye todo el marco como lo hace un cambio de idioma y carga la hoja de estilo y el script del otro shell. Los enlaces a `/_zeyn/route/{screen}` (los de las migas de pan) se siguen de la misma manera.

## El shell de inicio de sesión

Las páginas de inicio de sesión tienen un marco propio: `Shell::auth()` no tiene menú, ni barra superior, ni paleta de comandos. El logotipo de la aplicación (`App::logo`) y su nombre quedan sobre una única tarjeta de 400 px de ancho, y los selectores de idioma y de tema quedan al pie de la página, así que ambos siguen al alcance antes de iniciar sesión. `.split()` pone el formulario en una mitad y un panel de marca en `--z-brand` en la otra a partir de 1024 px; las pantallas más estrechas muestran la tarjeta centrada. En los teléfonos la tarjeta se convierte en la página y sus entradas y botones miden 44 px de alto. Sus estilos (`auth.css`, 3.5 KB) se cargan solo en estas páginas.

Todas las pantallas de inicio de sesión de `zeyn::accounts::ui` (inicio de sesión, registro, contraseña olvidada y restablecimiento, inicio de sesión en dos pasos, código de recuperación, bloqueo, confirmación y cierre de sesión) se abren en él, mientras que las pantallas de cuenta y de administración mantienen el shell de la aplicación. Una pantalla de inicio de sesión propia lo obtiene de su feature:

```rust
Feature::new("entry").screen(sign_in).shell(Shell::auth())
```

El formulario de inicio de sesión rellena el nombre de usuario con `Control::Username` (`autocomplete="username"`) y la contraseña con `current-password`, muestra el botón para ver la contraseña como un ojo dentro del campo y dice "Caps Lock is on" debajo mientras Bloq Mayús está activado (una región viva cortés).

## Selector de tema en una pantalla

`ThemePicker::new()` pone el selector de tema dentro de una pantalla, como la sección Apariencia de una página de ajustes. Ofrece los temas de `App::themes` (con Sistema cuando la aplicación sigue al dispositivo) y muestra el que está en uso.

```rust
#[screen(route = "/settings/appearance", permission = "settings.read")]
pub fn appearance() -> Screen {
    Screen::new(t!("settings.appearance")).body(Panel::new(t!("settings.theme")).add(ThemePicker::new()))
}
```

Es el mismo control que el selector del shell: la elección se aplica al momento sin recargar la página, se guarda en la cookie `zeyn_theme` (y en el almacén de vistas para usuarios con sesión), y el otro selector de la página también la muestra. Si la aplicación no ofrece elección (`theme_picker(false)` o un solo tema), no muestra nada.

## La cabecera de la aplicación

`App::header(|h| ...)` pone una barra sobre las pantallas del shell de la aplicación, con las ranuras de la cabecera del sitio: `start`, `center`, `end` y `bottom`. Un cuadro de búsqueda va en el centro y un enlace a las notificaciones al final. `sign_in()`, `language()` y `theme()` también funcionan ahí, aunque el menú lateral ya tiene los selectores.

```rust
App::new(t!("app.name"))
    .header(|h| {
        h.center(SearchBox::of(search_all).results(search_results))
            .end(LinkButton::new(t!("app.notifications"), notifications, ()))
    })
```

Tres piezas listas convierten la barra en un marco de aplicación:

- `breadcrumb()` empieza la barra con el grupo del menú y el título de la pantalla actual; `nav.js` la reescribe después de cada navegación.
- `search()` añade un campo de búsqueda que abre la paleta de comandos, con su atajo a la vista; el botón de la paleta sale del menú lateral. En los teléfonos es un botón de icono.
- `account()` termina la barra con las iniciales del usuario en un botón redondo. Abre un diálogo (diálogo no modal de W3C APG; Escape y el botón de cierre lo cierran) con quién ha iniciado sesión, los selectores de tema e idioma y el enlace de cierre de sesión, o un enlace de inicio de sesión para los visitantes. Los selectores salen del menú lateral.

```rust
App::new(t!("app.name")).header(|h| h.breadcrumb().search().account())
```

La barra queda arriba mientras la pantalla se desplaza, y las pantallas cambian debajo sin recargar la página. En el teléfono se coloca bajo el botón del menú y la ranura central ocupa una fila propia. Los componentes de la barra se muestran para el usuario como los de una pantalla; un cuadro de búsqueda carga sus sugerencias con su acción, cuyo permiso comprueba el servidor. Sin `App::header`, el shell de la aplicación no tiene barra superior.

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

**Alt+Shift+P** abre la paleta por defecto (`App::palette_shortcut` lo cambia, también a una tecla del navegador como `Ctrl+Shift+P` a propósito): un combobox ARIA que busca en los comandos de la aplicación, en la barra de herramientas y el menú contextual de la pantalla actual y en las pantallas del menú. Los comandos usados recientemente van primero. `/` también la abre cuando el foco no está en un campo, y lo mismo hace el campo de búsqueda de `App::header(|h| h.search())`. Los resultados se agrupan bajo su lugar ("Go to · Sales", el título de la pantalla, "Commands"), cada uno con su icono, las letras que coinciden en negrita y su atajo al final. Los elementos del menú se buscan en la propia página, así que la primera pulsación ya los encuentra.

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
- `PropertyInspector::island(name)` edita lo que selecciona una isla, sin formulario (ver [Island inspector](#island-inspector)).
- `StatusBar` tiene una región en vivo, así que los mensajes que se le envían los leen los lectores de pantalla.

## Island inspector

```rust
Row::new()
    .col(Island::new("designer"))
    .col(PropertyInspector::island("designer"))

let schema = FormSchema::new()
    .section("Position")
    .field(FieldSpec::length("x", "X", &[LengthUnit::Mm]).min(0))
    .section("Look")
    .field(FieldSpec::color("fill", "Fill", BRAND).required())
    .field(FieldSpec::choice("align", "Align", [("left", "Left"), ("right", "Right")]));
```

Una isla envía `zeyn:inspect` con `detail: { title, schema, values }`, donde `schema` es un `FormSchema` serializado. Los tipos de campo son `text`, `multiline`, `integer`, `decimal`, `length`, `color`, `boolean`, `choice` y `expression`; cada uno admite `required`, `min`, `max`, `max_len`, `default` y `help`.

El inspector dibuja los campos con los controles de los formularios de modelo y rellena nombres, etiquetas y valores como texto, nunca como HTML. Al salir de un campo, pulsar Enter o cerrar un selector, comprueba el valor y envía `zeyn:property` con `detail: { name, value }` a la raíz de la isla. Un valor no válido muestra su error y no se envía. Un esquema con más de 200 campos se ignora. `FieldSpec::check` hace las mismas comprobaciones en el servidor.

## Paneles acoplables

```rust
Dock::new(t!("pro.workspace"))
    .left("explorer", t!("pro.panel_explorer"), explorer())
    .center("products", t!("pro.panel_products"), products)
    .bottom("output", t!("pro.panel_output"), output())
    .right("properties", t!("pro.properties"), shape_inspector())
```

Los usuarios arrastran pestañas entre áreas, cambian el tamaño de las áreas con el puntero o el teclado y cierran paneles; `MenuList::panels` en un menú los recupera. El diseño se guarda por usuario y pantalla en el view store de la aplicación, o en el navegador cuando la aplicación no tiene ninguno.
