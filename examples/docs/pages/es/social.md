# Social

Componentes para páginas sociales. Cada uno se dibuja en el servidor y carga su hoja de estilos y su script solo en las páginas que lo usan.

## StoryRing

`StoryRing` es un avatar dentro de un anillo que indica si las historias de una persona son nuevas para quien las ve. `StoryState::Unseen` dibuja un anillo grueso con los colores de la marca y `StoryState::Seen` uno fino y neutro. El estado viene del servidor, de lo que la aplicación guardó sobre el espectador; el navegador nunca lo cambia. `StoryRow` coloca los anillos en una fila que se desplaza hacia los lados, como un carril.

```rust
StoryRow::new()
    .add(StoryRing::new(ada.name.clone(), StoryState::Unseen)
        .image(ada.photo.clone())
        .link(stories, StoryParams { person: ada.id }))
    .add(StoryRing::new(alan.name.clone(), StoryState::Seen).image(alan.photo.clone()))
```

Con `.link(screen, params)` el anillo es un enlace a la pantalla que muestra las historias. Su nombre accesible es el de la persona y el estado, por ejemplo "Ada Lovelace, historias sin ver"; el nombre visible bajo el anillo empieza igual, de modo que quienes usan control por voz pueden decirlo. Los dos estados se distinguen tanto por el grosor del anillo como por su color, y en el modo de colores forzados el anillo visto se dibuja discontinuo. La imagen sigue las reglas de `Avatar` (una ruta de la aplicación o una dirección `https` permitida); sin ella se muestran las iniciales.

El anillo se dibuja solo con CSS: sin script y sin imagen. `stories.css` (2,2 KB) se carga solo en las páginas que tienen un anillo. Una fila admite como máximo `MAX_STORY_RINGS` (100) anillos. El tacto, el panel táctil y Shift+rueda la desplazan, los anillos se ajustan a su posición y un anillo que recibe el foco del teclado se desplaza a la vista. Cuando ningún anillo de una fila tiene enlace, la propia fila recibe el foco, de modo que el teclado aún puede desplazarla.

| Tecla | Qué hace |
|---|---|
| Tab | Pasa al siguiente anillo y lo desplaza a la vista |
| Enter | Abre las historias del anillo con foco |
| Left, Right | Cuando la propia fila tiene el foco: la desplaza |


## Lightbox

Cualquier `Image` se abre a pantalla completa con `.lightbox()`: un diálogo modal con la imagen tan grande como permite la pantalla, su pie y el zoom. `.full(src)` da el archivo más grande que muestra el lightbox mientras la página conserva el pequeño, `.caption(text)` añade un pie debajo (ambos activan el lightbox) y `.video(source, file)` convierte la imagen en el póster de un vídeo de una fuente de medios, que el lightbox reproduce. Las imágenes de una publicación, o de un contenedor como `Row`, `Stack` o `AutoGrid`, forman un conjunto: las teclas de flecha y los gestos de deslizamiento se mueven entre ellas.

```rust
AutoGrid::new().min(140)
    .add(Image::new(p.thumb.clone()).alt(p.title.clone()).size(300, 300)
        .full(p.photo.clone())
        .caption(p.title.clone()))
    .add(Image::new(clip.poster.clone()).alt(clip.title.clone()).size(320, 180)
        .video("films", clip.file.clone()))
```

Sin script la imagen es un enlace a su archivo, así que sigue abriéndose. Con script, `lightbox.js` (13 KB, 4,4 KB gzip) y `lightbox.css` (4 KB) se cargan solo en las páginas que tienen una imagen con lightbox, y el diálogo se construye la primera vez que se abre uno. Solo el elemento abierto está en el diálogo; los archivos más grandes de sus vecinos se descargan antes como imágenes, los vídeos nunca. La miniatura se muestra de inmediato, desenfocada, y el archivo grande la cubre cuando termina de cargar. La apertura tarda unos 16 ms en un escritorio.

El archivo grande sigue las reglas de dirección de la imagen (una ruta de la aplicación o una dirección `https` permitida); si las incumple, se usa el archivo propio de la imagen. Un archivo de vídeo debe ser un id de medio válido de la fuente, que comprueba su permiso y sirve rangos en cada petición. El diálogo sigue el patrón de diálogo de W3C APG: el foco pasa al botón de cierre, Tab y Shift+Tab se quedan dentro y al cerrar el foco vuelve a la imagen que lo abrió (o a su publicación, si el feed ha descargado la imagen entretanto). Cada enlace de imagen se nombra con su texto alt, o "Abrir imagen" cuando está vacío. La posición ("2 de 4") se anuncia. El movimiento sigue `prefers-reduced-motion` y la página de fondo no se desplaza mientras el diálogo está abierto.

| Key | What it does |
|---|---|
| Enter | En una imagen: abre el lightbox |
| Right, Left | Elemento siguiente y anterior (invertido de derecha a izquierda); desplaza una imagen ampliada |
| Up, Down | Desplaza una imagen ampliada |
| Home, End | Primer y último elemento |
| +, -, 0 | Acerca, aleja, restablece el zoom |
| Escape | Cierra el lightbox y devuelve el foco |
| Tab, Shift+Tab | Se mueve entre los botones del diálogo sin salir de él |

El doble clic, el doble toque y el botón de zoom también hacen zoom, y una imagen ampliada se desplaza arrastrando.
