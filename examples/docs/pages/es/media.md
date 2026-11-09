# Media

Zeyn sirve archivos multimedia con solicitudes de rango HTTP y los reproduce en los componentes `Video` y `Audio`. Los elementos nativos `<video>` y `<audio>` se encargan de la decodificación; Zeyn dibuja encima sus propios controles accesibles.

## Servir archivos

Registra un `MediaSource` en una feature con un permiso:

```rust
Feature::new("library")
    .media("films", "library.watch", FilmFolder::new())
    .media("music", "library.listen", SongTable::new())
```

El navegador solicita `GET /_zeyn/media/{source}/{id}`. Antes de que el source vea nada, el servidor:

1. rechaza los ids que no tengan de 1 a 128 caracteres de `A-Z a-z 0-9 - _ .`, que empiecen por un punto o que contengan `..` (`404`);
2. comprueba el permiso del source (`403`) y los guards de la feature;
3. ejecuta los interceptores (`CallKind::Media`, con el nombre del source en `screen`).

`open(ctx, id)` busca el id en el catálogo propio de la app y devuelve un `Media`: `Media::file(path)` para un archivo en disco (tipo según la extensión, entity tag a partir del tamaño y la fecha de modificación), `Media::bytes(data, type)` para datos en memoria, o `Media::new(reader, len, type)` para cualquier `Read + Seek`. El id es un nombre, nunca una ruta: construye la ruta a partir de tu búsqueda, no del id.

Respuestas:

| Solicitud | Respuesta |
|---|---|
| sin `Range`, otra unidad o varios rangos | `200` con el archivo completo |
| `bytes=a-b`, `bytes=a-`, `bytes=-n` | `206` con `Content-Range: bytes a-b/len` |
| un rango de bytes mal formado o que empieza después del final | `416` con `Content-Range: bytes */len` |
| un `If-Range` que no coincide con el entity tag | `200` con el archivo completo |
| un `If-None-Match` que coincide con el entity tag | `304` |

Toda respuesta lleva `Accept-Ranges: bytes`, `Cache-Control: private, no-cache` y `X-Content-Type-Options: nosniff`. Solo el audio, el vídeo, las imágenes raster, WebVTT y las listas de reproducción de streaming se sirven en línea; cualquier otro tipo (HTML, SVG, scripts) se envía como `application/octet-stream` con `Content-Disposition: attachment`, de modo que un archivo nunca pueda ejecutarse como una página de la app. Los archivos se leen en bloques de 64 KiB en el pool de bloqueo con un máximo de cuatro bloques en cola, así que la memoria se mantiene plana para cualquier tamaño de archivo y un cliente que se va detiene la lectura. Los límites de tasa se aplican como en las demás solicitudes; las lecturas de media no son escrituras.

La content security policy no necesita cambios: `default-src 'self'` ya permite el media de la app. Una biblioteca de streaming adaptativo que use Media Source Extensions necesita `blob:` en `media-src`, algo que Zeyn no añade.

## Archivos públicos: imágenes, logotipos, descargas

Los archivos que cualquiera puede ver, haya iniciado sesión o no, van en un source público:

```rust
Feature::new("catalog").public_media("images", 86_400, ImageFolder::new("data/images"))
```

`GET /_zeyn/media/images/{id}` omite la comprobación de permisos (los ids, los guards y los interceptores siguen ejecutándose), no establece ninguna cookie y responde con `Cache-Control: public, max-age=86400` (como máximo `MAX_PUBLIC_MEDIA_CACHE`, un año), de modo que los navegadores y una CDN delante de la app lo conservan. El entity tag sigue permitiéndoles revalidar con `304`. Usa la ruta como `src` de un `Image`. Para muchas imágenes grandes, un host de imágenes aparte detrás de una CDN sigue siendo la opción más barata; permítelo con `App::csp(|c| c.img_src([...]))`.

- `open` debe devolver los mismos bytes para todos los visitantes. Nunca hagas variar un archivo público según el usuario, el rol, el tenant o el idioma: una CDN o un proxy conserva la primera respuesta y se la entrega a todos. Los archivos que difieren por usuario pertenecen a un source registrado con `Feature::media`.
- Con un `max_age` largo, usa ids que cambien con el contenido, como un hash del contenido (`poster-3f9a2c.jpg`). Así un archivo modificado obtiene una dirección nueva, en lugar de que las copias antiguas sigan vivas en las cachés hasta que caduquen.
- Las lecturas de sources públicos (`GET` y `HEAD`) no cuentan para `App::rate_limit`: una página puede cargar decenas de pósteres, y muchos visitantes móviles comparten una dirección detrás del NAT del operador. Los sources privados siguen contando, y las reglas de IP permitidas y denegadas se siguen aplicando a ambos. Limitar el abuso de ancho de banda es tarea de una CDN o un reverse proxy delante de la app.

## Reproductores

```rust
Video::new("films", t!("films.harbor"))
    .subtitle(t!("films.harbor_sub"))
    .file("harbor.webm")
    .file("harbor.mp4")
    .hls("harbor.m3u8")
    .poster("harbor.jpg")
    .captions("harbor.en.vtt", "en", "English")
    .captions("harbor.tr.vtt", "tr", "Türkçe")

Audio::new("music", t!("songs.dawn")).subtitle(t!("songs.artist")).file("dawn.mp3")
```

Un reproductor no descarga nada hasta que entra en la vista: tiene `preload="none"`, y sus fuentes y su póster se asignan la primera vez que aparece al desplazarse (un solo observador atiende a todos los reproductores de la página). `.poster_low(id)` muestra primero un póster pequeño, `.placeholder(...)` colorea el escenario antes de eso (el `Placeholder` de `Image`), `.on_error(text or component)` sustituye el mensaje predeterminado cuando el medio no se puede reproducir, y `.sources([(id, media_query)])` añade archivos para otras pantallas, como `("clip-480.mp4", "(max-width: 600px)")`. Para muchas calidades usa HLS o DASH (más abajo).

Para listas, `.preview_when_visible()` convierte el reproductor en una vista previa breve: se reproduce silenciado y en bucle mientras al menos una cuarta parte es visible y se pausa cuando sale. Nunca empieza solo cuando el visitante prefiere movimiento reducido o ahorra datos (`prefers-reduced-data`, Save-Data), nunca con sonido, y en cuanto el visitante usa sus controles deja de iniciarse o detenerse por sí mismo.

Controles: reproducir y pausar, un control deslizante de búsqueda, el tiempo, silenciar, un control deslizante de volumen, un menú de velocidad de reproducción, un botón de subtítulos y (con más de un archivo de subtítulos) un menú de idioma, y para vídeo imagen en imagen (donde el navegador lo admita) y pantalla completa. Los controles deslizantes siguen el patrón slider de WAI-ARIA: las flechas mueven 5 segundos (o el 5 por ciento del volumen), Re Pág y Av Pág mueven una décima parte, Inicio y Fin van a los extremos. Los menús siguen el patrón de botón de menú. Mientras el foco está dentro de un reproductor, `k` o Espacio reproduce y pausa, `j` y `l` saltan 10 segundos, `m` silencia, `c` alterna los subtítulos y `f` alterna la pantalla completa; las teclas no hacen nada en el resto de la página.

Los subtítulos son archivos WebVTT servidos por el mismo source. El reproductor dibuja la línea activa como texto simple (se descarta el marcado de la línea), sobre la imagen en el vídeo y bajo el título en el audio, de modo que los subtítulos se ven igual en todos los navegadores y en pantalla completa.

## Reanudar

El reproductor guarda dónde se detuvo el usuario: cada 15 segundos durante la reproducción, al pausar, cuando la página se oculta o se cierra, y `0` cuando termina el media. Reanuda cuando el punto guardado está a más de 5 segundos de cualquiera de los extremos.

- Cuando el source implementa `position` y `save_position`, las posiciones van al servidor: `GET /_zeyn/media/position?source=&id=` responde `{"at": seconds, "kept": true}`, y `POST /_zeyn/media/position` con `{source, id, at}` guarda (`204`). Ambos comprueban el id, el permiso y los guards y requieren un usuario con sesión iniciada; el POST también requiere la cabecera CSRF y cuenta como escritura para los límites de tasa. Las posiciones por encima de `MAX_POSITION_SECONDS` o por debajo de cero devuelven `422`.
- En otro caso (los métodos predeterminados del source, o un usuario anónimo) la lectura responde `{"at": null, "kept": false}` y el reproductor guarda la posición en el almacenamiento local del navegador bajo `zeyn.media.{source}/{id}`. Los errores de almacenamiento (ventanas privadas, almacenamiento bloqueado) se ignoran.

Las posiciones en el servidor son la mejor opción para usuarios con sesión iniciada: siguen al usuario a cada dispositivo y la app puede leerlas para una fila de "seguir viendo". El almacenamiento local no necesita código, pero se queda en un solo navegador y lo comparten todos los que lo usan.

## HLS y DASH

`hls(id)` y `dash(id)` añaden un stream adaptativo. Safari reproduce HLS de forma nativa, por lo que usa la lista de reproducción. Los demás navegadores necesitan una biblioteca de JavaScript (como hls.js o dash.js), que Zeyn no incluye. Antes de recurrir a los archivos progresivos, el reproductor despacha un evento cancelable `zeyn:media-adaptive` en su raíz:

```js
document.addEventListener('zeyn:media-adaptive', (e) => {
  const { media, kind, url } = e.detail;
  if (kind !== 'hls' || !window.Hls?.isSupported()) return;
  e.preventDefault();
  const hls = new window.Hls();
  hls.loadSource(url);
  hls.attachMedia(media);
});
```

Carga ese script desde un archivo de tu app (los scripts en línea los bloquea la content security policy). Cuando ningún listener llama a `preventDefault()`, las entradas `file` se reproducen en orden. Los archivos de lista de reproducción y de segmentos los sirve el mismo source, por lo que sus URL relativas funcionan siempre que sus ids estén listados en él.

## Archivos subidos

Los archivos subidos mediante `Form::file` se sirven con `GET /_zeyn/files/{store}/{id}` con las mismas respuestas que los archivos multimedia (rangos, entity tags, `nosniff`, en línea solo para imágenes raster, audio y vídeo), después del permiso del store, los guards de la feature y una comprobación de tenant. Consulta la sección de subida de archivos de `examples/docs/pages/forms.md`.
