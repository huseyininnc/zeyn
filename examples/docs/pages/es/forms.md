# Formularios

`Form::of(value)` edita un modelo o una entidad. Los campos salen de las referencias de campo, las etiquetas de los archivos de locale y la validación de los atributos `#[rule]`, así que el navegador y el servidor comprueban las mismas reglas.

## Un formulario

```rust
Form::of(Contact { quantity: 1, ..Contact::default() })
    .row(|r| r.field(Contact::name).field(Contact::email))
    .row(|r| r.field(Contact::quantity).field(Contact::size))
    .submit(t!("docs.live.send"), send_contact)
    .on_success(|_, ui| ui.toast(t!("docs.live.sent")))
```

@live form

Deja el nombre vacío o escribe un correo incorrecto y pulsa *Send*: el navegador muestra el error al instante, y el servidor rechazaría la misma entrada con el mismo mensaje.

## Disposición

| Llamada | Resultado |
|---|---|
| `.field(M::f)` | Un campo en su propia línea |
| `.row(\|r\| r.field(a).field(b))` | Campos uno al lado de otro; se apilan en pantallas estrechas |
| `.section(title)` | Un grupo con título de los campos que siguen |
| `.step(title)` | Un paso de asistente; el formulario muestra un paso cada vez |
| `.range(M::from, M::to)` | Dos fechas como un selector de rango |
| `Form::auto(value)` | Todos los campos, en el orden de declaración |

## Guardado

`.submit(label, action)` nombra la acción que guarda. La acción debe recibir `Valid<M>` para el modelo del formulario; cualquier otra cosa no compila. El runtime envía los valores, el servidor ejecuta todas las reglas y después la acción.

- Los errores de campo de las reglas o de `ValidationErrors` en la acción aparecen junto a los campos.
- `Error::Conflict` (un `#[version]` antiguo) muestra un mensaje de que otra persona cambió el registro.
- `.on_success(|result, ui| ...)` ejecuta effects tras un guardado correcto (consulta [Effects y ventanas](effects.md)).

## Controles

Cada tipo de campo tiene un control por defecto: entradas de texto, entradas numéricas, dinero con la moneda, selectores de fecha y hora, casillas de verificación para `bool` y selects para choices. `.control(M::f, Control::...)` elige otro, y `.lookup(M::f, action)` busca registros en el servidor. Consulta [Controles y selectores](controls.md).

## Campos de solo lectura

Los campos `#[readonly]` se muestran en los formularios pero no se pueden editar, como un nivel de existencias que solo cambian los movimientos.

## Subida de archivos

Los campos `File`, `Option<File>` y `Vec<File>` contienen archivos guardados por un `FileStore`. Declara las reglas una vez como un `const`, registra el store en la feature con un permiso y usa las mismas reglas en el formulario:

```rust
pub const ATTACHMENTS: FileRules = FileRules::new("attachments")
    .max_bytes(5 * 1024 * 1024)
    .max_files(5)
    .accept(&[FileType::Pdf, FileType::Png, FileType::Jpeg]);

Feature::new("support")
    .files(ATTACHMENTS, "support.ticket.files", DiskStore::new("data/attachments"))
    .scan_files(MyVirusScanner)

Form::of(ticket).field(Ticket::title).file(Ticket::attachments, ATTACHMENTS)
```

Un nombre de permiso permite a los mismos usuarios subir y descargar. Para que los lectores abran archivos que no pueden añadir, divídalo: `.files(ATTACHMENTS, FileAccess::read("support.ticket.read").write("support.ticket.files"), store)`.

El control es una entrada de archivo nativa dentro de una zona de arrastre. Cada archivo se sube por separado con una barra de progreso, se puede cancelar mientras se ejecuta, reintentar si falló y quitar cuando termina. Las imágenes muestran una vista previa hecha a partir de una URL de objeto del archivo elegido. El progreso y los resultados se anuncian en una región en vivo educada. El showcase tiene un ejemplo funcional en `/uploads`; este sitio de documentación no tiene ningún usuario con sesión iniciada, así que no muestra ninguna subida en vivo.

| Tecla | Qué hace |
|---|---|
| Tab | Pasa a la entrada de archivo, y después al enlace de descarga de cada archivo y a su botón Cancel, Retry o Remove |
| Enter o Espacio en la entrada de archivo | Abre el selector de archivos del sistema (el camino con teclado para arrastrar y soltar) |
| Enter o Espacio en Cancel, Retry, Remove | Lo ejecuta; tras Cancel el foco pasa a Retry, tras Remove al siguiente archivo o de vuelta a la entrada |

Lo que hace el servidor para `POST /_zeyn/files/{store}`:

1. comprueba la cabecera CSRF, el permiso del store y los guards de la feature, y ejecuta los interceptores (`CallKind::Upload`, auditado con el id del archivo nuevo); las subidas cuentan contra el límite de escritura;
2. rechaza un `Content-Length` declarado por encima del límite (`413`), después lee los primeros 4 KiB y averigua el tipo a partir del contenido; la extensión debe pertenecer al mismo tipo y el tipo debe estar en `accept`, de lo contrario `415`. HTML, SVG, XML y scripts nunca se aceptan. El tipo de contenido del navegador se ignora;
3. envía el resto en streaming al store en trozos de como máximo 64 KiB a través de un canal de cuatro, contando bytes; superar el límite, una conexión rota o 30 segundos sin datos detienen la subida y el store elimina lo que escribió;
4. ejecuta el `FileScanner` de la feature; un archivo rechazado, o un escáner que falla, se elimina y la respuesta es `422`;
5. responde `201` con `{id, name, size, type}`. El id son 128 bits aleatorios del servidor; los nombres se limpian de rutas y caracteres de control.

Las descargas (`GET /_zeyn/files/{store}/{id}`) comprueban la forma del id, el permiso, los guards y el tenant, y pasan por el servicio de medios (consulta `docs/media.md`): rangos, etiquetas de entidad y `nosniff`. Solo las imágenes ráster, el audio y el vídeo se muestran en línea; todo lo demás se envía como `application/octet-stream` con `Content-Disposition: attachment`, un nombre ASCII entre comillas más el nombre UTF-8, bajo `Content-Security-Policy: sandbox`.

`DiskStore` guarda `{id}.bin` y `{id}.json` en una carpeta y escribe primero las subidas en `{id}.part`. `MemoryStore` guarda los archivos en memoria con un límite de bytes, para pruebas y demostraciones. El valor del formulario solo nombra archivos: una acción de guardado que deba estar segura de que un archivo existe llama a `store.meta(id)`, como hace el showcase.

Presupuesto: una subida de 256 MiB hace crecer el heap del servidor unos 0,4 MiB (`tests/files_memory.rs`); el control renderiza 10 nodos DOM más 5 por archivo; `files.js` añade 12 KB sin comprimir y 4 KB con gzip, `files.css` 3 KB sin comprimir y 1 KB con gzip.
