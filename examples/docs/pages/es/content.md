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
