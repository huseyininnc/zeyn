# Texto y contenido

Las páginas de lectura, como artículos, perfiles y listados, se construyen con unos pocos componentes pequeños. Son HTML plano renderizado en el servidor, sin script, y cada texto procede de los archivos de locale o se escapa.

@live content

## Heading

De `Heading::h2(text)` a `Heading::h6(text)`. El título del screen es el único `h1` de la página, así que aquí no hay `h1`: elige el nivel que sigue al esquema, no el tamaño que quieres. `.id("anchor")` permite que los enlaces apunten a él.

```rust
Heading::h2(t!("shop.new_arrivals")).id("new")
```

## Paragraph

Un párrafo de texto escapado. `.muted()` usa el color de texto secundario y `.small()` el tamaño de fuente pequeño. Las palabras largas se parten.

```rust
Paragraph::new(t!("shop.intro"))
Paragraph::new(t!("shop.tax_note")).muted().small()
```

## MetaLine

Datos breves en una sola línea, como una fecha, un tiempo de lectura y una categoría. El separador (un punto medio, salvo que `.separator("/")` indique otro) se oculta a los lectores de pantalla, y las partes vacías se omiten.

```rust
MetaLine::new().part(article.published.format(ctx.locale())).part(t!("blog.read_time")).part(article.category.clone())
```

## PrevNext

Enlaces al elemento anterior y al siguiente, con `rel="prev"` y `rel="next"`. Se abren sin recargar la página. Las palabras «Anterior» y «Siguiente» proceden de los textos del framework; `.prev_title` y `.next_title` añaden debajo el título del elemento. Las flechas se invierten en los idiomas de derecha a izquierda, y en los teléfonos los dos enlaces se apilan.

```rust
PrevNext::new()
    .prev(article, ArticleParams { id: prev.id })
    .prev_title(prev.title.clone())
    .next(article, ArticleParams { id: next.id })
    .next_title(next.title.clone())
    .shortcut("Alt+Shift+N")
```

### Atajos de enlace

`.shortcut(keys)` da un atajo de teclado al lado establecido en último lugar, y `Chips::links(...).link(...).shortcut(keys)` da uno al chip de enlace añadido en último lugar. Al pulsar las teclas se sigue el enlace mediante `nav.js` sin recargar la página, tanto en las páginas del shell de la aplicación como en las del shell del sitio. Las teclas no hacen nada mientras el foco está en un input, un text area, un select o un área editable, así que nunca se le quita al usuario lo que escribe. El enlace lleva `aria-keyshortcuts` para los lectores de pantalla y muestra las teclas en una pista `<kbd>`, que las pantallas táctiles ocultan. Los atajos se leen como `Command::shortcut`: los atajos que el navegador se reserva (`Ctrl+T`, `Ctrl+W`) o usa (`Ctrl+P`, `F5`) no se enlazan. `App::check` informa de los atajos de enlace en las ranuras de la cabecera del sitio que no se pueden leer, que pertenecen al navegador, que son una tecla de un solo carácter (WCAG 2.2, 2.1.4: `Shift+N` cuenta, `Alt+Shift+N` no) o que chocan con un comando, la paleta u otro enlace; `App::check_page(&screen)` hace lo mismo para una pantalla construida en un test con sus params, ya que `App::check` no puede construir pantallas por sí mismo.

Las teclas pulsadas mientras el foco está dentro de un marco de otro origen, como un reproductor de vídeo incrustado, van al documento de ese marco y nunca llegan a la página; ningún script puede cambiarlo. Indica a los usuarios que hagan clic fuera del marco, o que pulsen Tab para salir de él, antes de usar un atajo.

## AutoGrid

Elementos hijos en una cuadrícula cuyo número de columnas sigue el ancho disponible: tantas columnas de al menos `min` píxeles (240 si no se indica) como quepan, y una sola columna en un teléfono. El navegador la dispone con `repeat(auto-fill, minmax(min(240px, 100%), 1fr))`, de modo que no hay puntos de ruptura ni script. Admite cualquier nodo: tarjetas, paneles, cuadrículas. El nombre la distingue de `DataGrid`, que muestra filas de datos.

```rust
AutoGrid::new()
    .min(220)
    .gap(4)
    .add(Card::new().title(t!("home.orders")))
    .add(Card::new().title(t!("home.stock")))
    .add(Card::new().title(t!("home.people")))
```

@live autogrid

## List

Filas una debajo de otra en un `<ul>`. Cada `ListItem` tiene un título y puede tener una pequeña imagen al inicio (las mismas direcciones seguras que las imágenes de las tarjetas: una ruta, `https:` o `data:image/`), un subtítulo y metadatos breves al final: texto o un componente en línea como `RelativeTime`. Con `.link(screen, params)` toda la fila es un único enlace, de modo que es una sola parada en el orden de tabulación, con un anillo de foco visible, y se abre sin recargar la página. El texto largo se ajusta; en los teléfonos el texto de metadatos pasa bajo el título, y las páginas de derecha a izquierda reflejan la fila.

`List::new()` contiene los elementos que se le dan. `List::of(action)` carga filas mediante el mismo tipo de action que usa un DataGrid y pagina como una [cuadrícula de tarjetas](catalog.md#card-grid): 20 filas con un botón «Show more» salvo que `paging` diga otra cosa, opciones de orden, búsqueda en el servidor con `.search(text)` y el acceso de la action comprobado en cada página.

```rust
List::new()
    .title(t!("team.title"))
    .item(ListItem::new("Ada Lovelace").subtitle(t!("team.math")))
    .item(ListItem::new("Alan Turing").subtitle(t!("team.computing")).meta(t!("team.lead")))

List::of(search_people)
    .title(t!("people.title"))
    .item(|p: &Person| {
        ListItem::new(p.name.clone())
            .image(p.photo.clone())
            .subtitle(p.city.clone())
            .meta(RelativeTime::new(p.joined))
            .link(person, PersonParams { id: p.id })
    })
    .sort_option(t!("people.by_name"), Person::name, false)
    .paging(Paging::pages(20))
```

@live list

## Avatar

Una imagen redonda de una persona o de un equipo, de 32, 40 o 64 píxeles (`AvatarSize::Small`, `Medium`, `Large`). El nombre es obligatorio: es el texto alternativo y el origen de las iniciales que se muestran cuando no hay imagen, o cuando la imagen no está en un host que permita el `img-src` de la aplicación (la misma regla que para `Image`). Las iniciales siguen las mayúsculas del idioma, de modo que la `i` turca pasa a `İ`. Cuando el nombre está escrito justo al lado del avatar, `.decorative()` lo oculta a los lectores de pantalla para que no lean el nombre dos veces.

```rust
Avatar::new(p.name.clone()).image(p.photo.clone()).size(AvatarSize::Large)
Avatar::new(p.name.clone()).size(AvatarSize::Small).decorative()
```

## Spoiler

Contenido oculto hasta que el lector lo abre, como la respuesta a una pregunta. Es un control de divulgación nativo (`<details>` y `<summary>`), por lo que funciona con el teclado y con lectores de pantalla sin ningún script, y el contenido queda fuera del orden de lectura hasta que se abre. La etiqueta la pone la aplicación.

```rust
Spoiler::new(t!("quiz.show_answer")).add(Paragraph::new(answer.clone()))
```

@live people

## AvatarGroup

Las personas de una cosa, como los asignados de una tarea, en avatares que se solapan en una fila. La etiqueta nombra el grupo para los lectores de pantalla. El grupo muestra los primeros avatares, 5 salvo que `.max(n)` diga otra cosa y nunca más de 12, y después el número de los restantes como "+3", leído como "3 más" en el idioma del usuario. `.size(...)` fija el tamaño de todos los avatares del grupo.

```rust
AvatarGroup::new(t!("task.assignees"))
    .add(Avatar::new(a.name.clone()).image(a.photo.clone()))
    .add(Avatar::new(b.name.clone()))
    .max(3)
    .size(AvatarSize::Small)
```

## Quote

Palabras citadas de una persona o de una obra, como un `blockquote` dentro de un `figure`. `.author(name)` dice quién las dijo y `.source(title)` nombra la obra, escrita como un título (`cite`).

```rust
Quote::new(review.text.clone()).author(review.name.clone()).source(t!("shop.verified_buyer"))
```

## Kbd

Teclas que pulsar, escritas como entrada de teclado (`kbd`). `Kbd::new("Ctrl+K")` lee una combinación con `+` entre sus teclas; `Kbd::keys(["Ctrl", "+"])` toma las teclas una a una, para una combinación que contiene la propia tecla más. Los nombres de las teclas se escriben tal como se dan, 6 como máximo. Solo muestra las teclas; un atajo que hace algo se define donde está el comando, como hace `PrevNext::shortcut`.

```rust
Kbd::new("Alt+Shift+P")
```

## Divider

Una línea entre dos partes de una página. `Divider::new()` es una separación temática (`hr`). `.label(text)` pone un texto en medio de la línea, como "o" entre dos formas de iniciar sesión; el divisor es entonces un `separator` nombrado por ese texto. `.vertical()` dibuja la línea de arriba abajo, para el espacio entre las columnas de un `Row` compacto.

```rust
Divider::new().label(t!("signin.or"))
```

@live pieces

## Highlight

`Highlight::new(text, query)` escribe un texto con las palabras de una búsqueda marcadas en él (`mark`), como el nombre de un producto en una lista de resultados. La consulta se divide por los espacios en blanco, se buscan 8 de sus palabras como máximo, y cada lugar donde aparece una de ellas se marca sin distinguir mayúsculas de minúsculas; una consulta vacía no marca nada. Tanto el texto como la consulta son datos del usuario: el texto se escapa, y la consulta solo se compara, nunca se escribe en la página. Las letras se comparan en minúsculas tal como las define Unicode, por lo que los pares turcos `I ı` e `İ i` no se igualan entre sí.

```rust
Highlight::new(product.name.clone(), p.q.clone())
```

## Toc

Una tabla de contenidos: enlaces a los encabezados de la página que se muestra, para páginas largas como un artículo o una política. `Toc::new(label)` es un `nav` nombrado por la etiqueta; `.item(title, id)` enlaza el encabezado que recibió la misma ancla con `Heading::id`, y `.sub(title, id)` añade una entrada un nivel por debajo de la entrada anterior. Las anclas se limpian como las limpia `Heading::id`, de modo que la misma cadena sirve en los dos sitios, y una entrada cuya ancla queda vacía se omite; una tabla de contenidos contiene 100 entradas como máximo. Los enlaces son enlaces simples a `#anchor`, así que funcionan sin script, el navegador se desplaza hasta el encabezado y la dirección se puede compartir. Nada marca la sección que se está leyendo, porque eso necesitaría un script.

```rust
Toc::new(t!("page.contents"))
    .item(t!("policy.delivery"), "delivery")
    .sub(t!("policy.abroad"), "delivery-abroad")
    .item(t!("policy.returns"), "returns")
Heading::h2(t!("policy.delivery")).id("delivery")
```

@live marks

## Stepper

Dónde está un proceso: sus pasos en orden, los hechos, el actual y los que faltan, como el camino de un pedido desde que se hace hasta que se entrega. `.step(title)` añade un paso y `.description(text)` pone una línea bajo el último paso añadido. `.current(index)` cuenta desde 0 y marca como hechos los pasos anteriores; `.finished()` marca todos los pasos como hechos. `.vertical()` pone los pasos uno debajo de otro, que es también como se ven en pantallas estrechas. Un stepper contiene 12 pasos como máximo.

```rust
Stepper::new(t!("order.progress"))
    .step(t!("order.placed"))
    .step(t!("order.packed"))
    .step(t!("order.shipped"))
    .description(order.shipped_note.clone())
    .step(t!("order.delivered"))
    .current(order.stage as usize)
```

@live stepper

Los pasos son una lista ordenada nombrada por la etiqueta. El paso actual lleva `aria-current="step"`, y un paso hecho dice "completado" a los lectores de pantalla en el idioma del usuario, de modo que el estado no se da solo con el color. Un stepper solo muestra el estado que le da el servidor y nada en él se puede pulsar; un formulario que se rellena por pasos es un `Form` con sus propios pasos (véase [Formularios](forms.md)).

## Pagination

Enlaces a las páginas de una lista que la propia pantalla pagina, como los artículos de un blog: anterior, los números de página y siguiente. Cada uno es un enlace a la misma pantalla con la página en sus parámetros, de modo que el servidor renderiza la página que se le pide, la dirección se puede compartir y nada se guarda en el navegador. Los enlaces se abren mediante `nav.js` sin recargar la página y funcionan sin script. `DataGrid`, `List` y `CardGrid` se paginan solos con `.paging(...)`; use esto solo donde la pantalla construye su propio contenido.

```rust
#[screen(route = "/blog", public)]
pub fn blog(p: BlogParams) -> Screen {
    Screen::new(t!("blog.title")).body(
        Pagination::new(blog, |page| BlogParams { page })
            .pages(page_count)
            .current(p.page),
    )
}
```

Las páginas se cuentan desde 1. La primera y la última página se muestran siempre, con una página a cada lado de la actual; `.around(n)` muestra más, hasta 4. Las páginas omitidas se convierten en un hueco, y un hueco nunca oculta una sola página. El número de página viene de la dirección, así que la pantalla lo lleva al rango antes de cargar datos; el componente hace lo mismo con lo que marca. Con una página o ninguna no renderiza nada.

Los enlaces son un `nav` llamado "Paginación" en el idioma del usuario. La página actual lleva `aria-current="page"`, cada número se nombra "Página 3" para los lectores de pantalla, anterior y siguiente llevan `rel="prev"` y `rel="next"`, y en los extremos son texto plano marcado con `aria-disabled` en lugar de enlaces que no llevan a ningún sitio.

| Tecla | Qué hace |
|---|---|
| `Tab` | Avanza por anterior, los números de página y siguiente |
| `Enter` | Abre la página; el foco y la posición de desplazamiento siguen a `nav.js` |

## Chips

Una fila de pequeños enlaces redondeados que pasan a más líneas. `Chips::links(label)` contiene enlaces simples a pantallas, como las etiquetas de un artículo, dentro de un `nav` nombrado por la etiqueta. `Chips::filter(label, screen, params)` crea botones de alternancia sobre un parámetro de pantalla: cada chip es un enlace a la misma pantalla con su valor añadido al parámetro o quitado de él, de modo que el servidor genera `aria-pressed` a partir de los parámetros recibidos y no se guarda nada en el navegador. Pulsar un chip abre la nueva dirección mediante `nav.js` sin recargar la página, y el foco y la posición de desplazamiento se quedan en el chip. Solo hay un valor pulsado a la vez salvo que se llame a `.multiple()`; los valores de los parámetros que no son opciones se ignoran. Teclado: Tab se mueve entre chips, Enter o Space pulsa uno, y un chip de enlace puede tener su propio atajo (consulta [Atajos de enlace](#link-shortcuts)). Un chip pulsado también muestra una marca de verificación, así que el estado no depende solo del color. El script (`chips.js`, menos de 1 KB) se carga solo en páginas con chips de filtro.

```rust
#[screen(route = "/articles", public)]
pub fn articles(p: ArticleParams) -> Screen {
    Screen::new(t!("blog.articles")).body(
        Chips::filter(t!("blog.tags"), articles, |tags| ArticleParams { tags: tags.join(",") })
            .current(p.tags.split(','))
            .option("guide", t!("blog.guides"))
            .option("news", t!("blog.news"))
            .multiple(),
    )
}
```

@live chips


## Hero

Una sección grande en la parte superior de una página: una imagen de fondo con un título, una línea de texto y acciones como botones. `.link(label, screen, params)` añade un `LinkButton` a otra página: el primero de una diapositiva tiene el aspecto primario, los siguientes el secundario. Es estática por defecto. `.slide(title)` añade otra diapositiva, y `.text`, `.image` y `.action` siempre afectan a la última. Con varias diapositivas sigue el patrón de carrusel de W3C APG: botones para elegir diapositiva, cada diapositiva es un grupo llamado «2 de 3» y la diapositiva elegida se anuncia con cortesía. Las diapositivas cambian solo cuando el usuario elige una, salvo que la app llame a `.rotate(seconds)` (de 4 a 30). Entonces va primero un botón de pausa y reproducción (WCAG 2.2.2), la rotación se detiene mientras el puntero está sobre el hero o el foco está dentro y mientras la pestaña está oculta, y nunca empieza si el usuario prefiere movimiento reducido; las diapositivas no se anuncian mientras se mueven. Sin script se muestra la primera diapositiva.

La imagen sigue las reglas de `Image` (una ruta de la app o una dirección `https` en un host del `img-src` de la app). Es decoración tras el texto, así que su texto alternativo está vacío: pon lo importante en el título y el texto, que se sitúan sobre un velo oscuro que mantiene el texto blanco con un contraste de al menos 4,5:1 sobre cualquier imagen. Los botones de enlace conservan sus colores sólidos y reciben un anillo de foco blanco sobre la imagen. La imagen de la primera diapositiva se carga con prioridad alta para un primer pintado rápido, las demás de forma diferida. El script (`hero.js`, 1,6 KB gzip) se carga solo en páginas con diapositivas.

```rust
Hero::new(t!("home.fresh"))
    .text(t!("home.fresh_text"))
    .image("/img/hero/kitchen.jpg")
    .link(t!("home.menu"), menu, ())
    .action(ActionButton::new(t!("home.order"), start_order, ()))
    .slide(t!("home.new_teas"))
    .image("/img/hero/teas.jpg")
    .rotate(8)
```

@live hero


## Reactions

Una fila de botones de reacción elegidos por la app, cada uno con una clave, un nombre y un símbolo como un emoji, y su recuento. `Reactions::of(action, target)` indica el elemento sobre el que son las reacciones; el destino lo fija la pantalla y nunca se lee del navegador. Al pulsar un botón se envía la reacción a `POST /_zeyn/data`, que comprueba CSRF, cuenta para el límite de escritura, abre la pantalla, comprueba el permiso de la acción y rechaza una reacción fuera del conjunto antes de que se ejecute la acción. La acción recibe un `ReactionPress` (target, reaction, pressed), lo guarda y devuelve el nuevo `ReactionState` (los recuentos y las reacciones del propio usuario), y los recuentos y los estados `aria-pressed` se actualizan en el sitio. Cada botón es un toggle button nombrado por el nombre de la reacción y su recuento; el símbolo se oculta a los lectores de pantalla. `.state(...)` da los recuentos del primer renderizado, así que se ven sin script. El script (`reactions.js`, 1,3 KB gzip) se carga solo en páginas con reacciones.

```rust
#[action(permission = "answers.react")]
pub async fn react_to_answer(press: ReactionPress, db: Db, ctx: Ctx) -> Result<ReactionState> {
    db.toggle_reaction(&ctx, &press.target, &press.reaction, press.pressed).await?;
    db.reaction_state(&ctx, &press.target).await
}

Reactions::of(react_to_answer, format!("answer-{}", answer.id))
    .reaction("helpful", t!("answers.helpful"), "\u{1f44d}")
    .reaction("thanks", t!("answers.thanks"), "\u{1f64f}")
    .state(answer.reactions.clone())
```

@live reactions


## RelativeTime

Un momento mostrado como tiempo relativo, como «hace 3 minutos» o «dentro de 2 días», en el idioma de la página. El servidor escribe la fecha y hora exactas en la zona horaria del usuario dentro de `<time datetime="...">`, así que el HTML es igual de un minuto a otro y una página en caché sigue siendo correcta. El runtime reescribe el texto con `Intl.RelativeTimeFormat` cuando se carga la página, cuando `nav.js` incorpora una pantalla y una vez por minuto; la hora exacta queda en el tooltip. Sin script, el texto es la hora exacta.

```rust
RelativeTime::new(post.published)
```


## Share

Un botón que comparte un enlace: la hoja de compartir del sistema donde el navegador tiene la Web Share API (la mayoría de los teléfonos) y, si no, una copia al portapapeles con un toast que lo indica. Comparte la página mostrada, o una pantalla con `Share::screen(screen, params)`, cuyos parámetros comprueba el compilador. `.title` fija el título que muestra la hoja de compartir, y la etiqueta es «Compartir» en el idioma del usuario salvo que `.label` dé otra. El servidor renderiza el botón oculto y su script lo muestra, así que una página sin script no tiene un botón que no hace nada. El script (`share.js`, menos de 1 KB) se carga solo en páginas con un botón de compartir.

```rust
Share::page().title(post.title.clone())
Share::screen(article, ArticleParams { id: post.id }).label(t!("blog.share_article"))
```

@live engage


## CopyButton

Un botón que copia un texto al portapapeles: un número de seguimiento, una dirección de API, un enlace de invitación. `.shown()` escribe el texto junto al botón con una fuente de ancho fijo, para que también se pueda leer y seleccionar; sin él, el texto está en la página pero oculto. La etiqueta es "Copiar" en el idioma del usuario salvo que `.label` fije otra, y un toast dice si la copia funcionó. Usa el mismo script pequeño que `Share`, cargado solo en las páginas que contienen uno, y se renderiza oculto y lo muestra ese script, de modo que una página sin script no tiene un botón que no hace nada. El texto se corta a los 4096 bytes y, como forma parte de la página, debe ser uno que el usuario pueda ver, nunca un valor enmascarado.

```rust
CopyButton::new(order.tracking.clone()).shown()
CopyButton::new(invite_url).label(t!("team.copy_invitation"))
```

## AlphaIndex

Una barra de letras, cada una un enlace a un screen con la letra en sus parámetros, por ejemplo las personas cuyo nombre empieza por ella. Las letras proceden del idioma del usuario, en su propio orden (`zeyn::ui::alphabet`): el turco tiene `Ç Ğ I İ Ö Ş Ü` en su sitio, el español tiene `Ñ`, el ruso y el árabe tienen sus propios alfabetos, y cualquier otro idioma recibe de la `A` a la `Z`. La letra del screen que se muestra se marca con `aria-current="page"` (una `i` turca marca `İ`), y `.other()` añade al final un enlace `#` para los nombres que empiezan por cualquier otro carácter. Los enlaces se abren sin recargar la página y en los teléfonos se reparten en más líneas.

```rust
#[screen(route = "/people/{letter}", public)]
pub fn people_by_letter(p: LetterParams) -> Screen {
    Screen::new(t!("people.title"))
        .body(AlphaIndex::new(people_by_letter, |l| LetterParams { letter: l.to_string() }).current(&p.letter).other())
}
```

## WeekSchedule

Siete columnas de días con filas procedentes de una action, como las clases, los turnos o los eventos de una semana. La action es del mismo tipo que usa un DataGrid y se comprueba igual. `.day` asigna una fila a su día: un `Date`, un `Option<Date>` (las filas sin fecha se omiten) o un número de día de la semana del 1 (lunes) al 7 (domingo); cualquier otra cosa no compila. Cada fila se muestra como un `ListItem`, de modo que puede enlazar a un screen. La semana empieza en el primer día de la semana del usuario (lunes en turco, domingo en inglés estadounidense), se marca el día de hoy, los días sin filas lo indican, y en pantallas estrechas los días se apilan. El servidor carga hasta `limit` filas (100 si no se indica, como máximo 500) en el orden de la action; dale a la action las filas de la semana, por ejemplo con un filtro por fecha.

```rust
WeekSchedule::of(this_week_classes)
    .title(t!("classes.this_week"))
    .day(|c: &Class| c.date)
    .item(|c: &Class| {
        ListItem::new(c.name.clone())
            .meta(c.starts.to_string())
            .link(class, ClassParams { id: c.id })
    })
```

@live week
