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
```

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

Filas una debajo de otra en un `<ul>`. Cada `ListItem` tiene un título y puede tener una pequeña imagen al inicio (las mismas direcciones seguras que las imágenes de las tarjetas: una ruta, `https:` o `data:image/`), un subtítulo y un texto breve de metadatos al final. Con `.link(screen, params)` toda la fila es un único enlace, de modo que es una sola parada en el orden de tabulación, con un anillo de foco visible, y se abre sin recargar la página. El texto largo se ajusta; en los teléfonos el texto de metadatos pasa bajo el título, y las páginas de derecha a izquierda reflejan la fila.

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
            .meta(p.joined.format(&locale))
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

## Chips

Una fila de pequeños enlaces redondeados que pasan a más líneas. `Chips::links(label)` contiene enlaces simples a pantallas, como las etiquetas de un artículo, dentro de un `nav` nombrado por la etiqueta. `Chips::filter(label, screen, params)` crea botones de alternancia sobre un parámetro de pantalla: cada chip es un enlace a la misma pantalla con su valor añadido al parámetro o quitado de él, de modo que el servidor genera `aria-pressed` a partir de los parámetros recibidos y no se guarda nada en el navegador. Pulsar un chip abre la nueva dirección mediante `nav.js` sin recargar la página, y el foco y la posición de desplazamiento se quedan en el chip. Solo hay un valor pulsado a la vez salvo que se llame a `.multiple()`; los valores de los parámetros que no son opciones se ignoran. Teclado: Tab se mueve entre chips, Enter o Space pulsa uno. Un chip pulsado también muestra una marca de verificación, así que el estado no depende solo del color. El script (`chips.js`, menos de 1 KB) se carga solo en páginas con chips de filtro.

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

Una sección grande en la parte superior de una página: una imagen de fondo con un título, una línea de texto y acciones como botones. Es estática por defecto. `.slide(title)` añade otra diapositiva, y `.text`, `.image` y `.action` siempre afectan a la última. Con varias diapositivas sigue el patrón de carrusel de W3C APG: botones para elegir diapositiva, cada diapositiva es un grupo llamado «2 de 3» y la diapositiva elegida se anuncia con cortesía. Las diapositivas cambian solo cuando el usuario elige una, salvo que la app llame a `.rotate(seconds)` (de 4 a 30). Entonces va primero un botón de pausa y reproducción (WCAG 2.2.2), la rotación se detiene mientras el puntero está sobre el hero o el foco está dentro y mientras la pestaña está oculta, y nunca empieza si el usuario prefiere movimiento reducido; las diapositivas no se anuncian mientras se mueven. Sin script se muestra la primera diapositiva.

La imagen sigue las reglas de `Image` (una ruta de la app o una dirección `https` en un host del `img-src` de la app). Es decoración tras el texto, así que su texto alternativo está vacío: pon lo importante en el título y el texto, que se sitúan sobre un degradado oscuro para dar contraste. La imagen de la primera diapositiva se carga con prioridad alta para un primer pintado rápido, las demás de forma diferida. El script (`hero.js`, 1,6 KB gzip) se carga solo en páginas con diapositivas.

```rust
Hero::new(t!("home.fresh"))
    .text(t!("home.fresh_text"))
    .image("/img/hero/kitchen.jpg")
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
