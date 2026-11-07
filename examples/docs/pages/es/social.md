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
