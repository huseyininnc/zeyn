# Operaciones

Cómo ejecutar una app de Zeyn en producción: datos de zonas horarias, licencias firmadas, estado compartido para varios
servidores, logs y trazas, documentos PDF y correo electrónico.

## Zonas horarias

`TimeZone::named` acepta todas las zonas IANA (`Europe/Istanbul`, `America/New_York`,
`Australia/Sydney`, en cualquier combinación de mayúsculas y minúsculas) y desfases fijos (`UTC+03:00`, `+05:30`).

- Los datos de zonas son la base de datos de zonas horarias IANA incluida en el binario mediante `jiff` (unos 200 KB).
  Las respuestas son las mismas en todas las plataformas, Windows incluido, y no se lee ningún zoneinfo del sistema.
- La historia está modelada: el horario de verano de Turquía hasta 2016 y su `+03` permanente desde entonces,
  las reglas de Estados Unidos anteriores a 2007 y todos los demás cambios registrados.
- Una hora de reloj que no existe (los relojes se adelantaron) se rechaza con
  `LocalTimeError::Skipped`. Una que ocurre dos veces (los relojes se atrasaron) significa el momento anterior.
- Una zona se lee del paquete la primera vez que se usa su nombre y se conserva durante la vida del
  proceso, unos pocos kilobytes cada una. Los nombres desconocidos nunca se conservan, por lo que los clientes no pueden hacer crecer la caché.
- `TimeZone::names()` es una lista corta de zonas comunes para el selector de zona del shell. La zona de un usuario
  que no está en la lista se añade al selector para ese usuario.
- Las nuevas reglas de zonas llegan con una versión más reciente de `jiff` (`cargo update -p jiff-tzdb`). Recompila y
  despliega cuando un país cambie sus reglas.

## Licencias firmadas

Un archivo de licencia se firma con una clave ed25519 que solo posee el proveedor. La app incorpora la clave
pública, de modo que un cliente puede leer una licencia pero no puede modificarla ni crear una nueva. Actívalo con la
feature de Cargo `signed-licenses`.

```
zeyn license keygen                      # writes zeyn-license.key, prints the public key
zeyn license sign acme.json --out license.json
```

`acme.json` contiene los campos de la licencia:

```json
{ "holder": "Acme", "valid_until": "2027-12-31", "modules": ["pos", "stock"], "max_users": 50 }
```

```rust
const LICENSE_KEY: &str = "f414a89e9dae57598d60141df96dd1c9fb655fdcf49f5eaf24e093fda7d332e3";

App::new("Shop")
    .license(SignedLicenseFile::new("license.json", LICENSE_KEY), Duration::from_secs(3600))
    .feature(Feature::new("pos").guard(Licensed::module("pos")).screen(pos))
```

- El archivo es JSON con la licencia como texto en `payload` y la firma en `signature`. Lo que
  se firma es `zeyn-license-v1\n` seguido del texto del payload, de modo que los bytes firmados son exactamente
  los bytes almacenados.
- La clave privada permanece en la máquina de firma. `keygen` nunca sobrescribe un archivo de clave, y `*.key`
  está en el `.gitignore` de los proyectos nuevos. `sign` lee la clave de `--key`, de la
  variable `ZEYN_LICENSE_KEY` o de `zeyn-license.key`, y rechaza una licencia sin `holder`
  o con un `valid_until` que no sea `YYYY-MM-DD`.
- Un archivo modificado, un archivo firmado con otra clave o un archivo que no es una licencia se rechaza:
  `License::from_signed` devuelve `LicenseError` (`BadSignature`, `InvalidPublicKey`,
  `Malformed`), el servidor registra el motivo, y cada guard `Licensed` deniega con "Your license does not
  cover this." Un archivo que no se puede leer conserva la última licencia, como cualquier fuente de licencia que falla.
- La caducidad la comprueba el guard en cada solicitud frente al día de hoy del usuario, de modo que una licencia
  caducada bloquea la app con "Your license has expired." sin reiniciar. Un archivo renovado se
  lee en la siguiente actualización.
- ed25519 no puede distinguir un archivo modificado de uno firmado con otra clave; ambos son una firma que
  no coincide, y el mensaje lo dice.
- `max_users` se transporta y se firma, pero el framework no lo aplica; la app decide qué
  cuenta como usuario.

## Varios servidores: estado compartido en Redis

Con la feature de Cargo `redis`, `RedisStore` mantiene el estado que debe ser el mismo en cada
instancia detrás de un balanceador de carga:

```rust
let redis = RedisStore::new(config.require("redis.url")?, "billing").map_err(Error::Internal)?;
App::new("Billing")
    .rate_limit(RateLimit::per_ip(300, Duration::from_secs(60)))
    .rate_store(redis.clone())
    .datagrid_views(redis)
```

Qué conserva el servidor por usuario, y dónde:

| Estado | Dónde | Compartido |
|---|---|---|
| Contadores de límite de tasa | `RateStore` | `RedisStore` |
| Vistas de grid guardadas y disposiciones de dock | `DataGridViewStore` | `RedisStore` |
| Tiempo de inactividad | el navegador (todas las pestañas de la app) | nada en el servidor |
| Inicio de sesión y reautenticación | el proxy de inicio de sesión único (single sign on) | nada en el servidor |
| Motivo para una pantalla sensible | una cookie `HttpOnly` | nada en el servidor |
| Borradores de formularios | `sessionStorage` | nada en el servidor |
| Caché de licencia | por proceso, recargada desde la fuente | cada servidor carga la suya |
| Trabajos en segundo plano | por proceso | no compartido; consulta la sección de trabajos de `enterprise.md` |

- Los límites de tasa son un token bucket en un único script Lua, de modo que las solicitudes concurrentes en distintos servidores
  nunca superan el límite. La hora proviene de Redis, no de los servidores de la app. Los contadores caducan una
  ventana después de su último uso.
- Las vistas son JSON bajo `zeyn:<prefix>:view:<user length>:<user>:<grid>` y nunca caducan.
- La conexión se establece en el primer uso y se recupera sola. Las llamadas agotan el tiempo a los 500 ms.
  Cuando Redis está caído, los límites de tasa dejan pasar las solicitudes, las vistas no se cargan ni se guardan (el
  grid recurre a su propia disposición, el dock a `localStorage`), la caída y la recuperación se
  registran una vez cada una, y se deja a Redis en paz durante cinco segundos tras cada fallo para que las solicitudes no
  esperen cada una un tiempo de espera agotado.
- `RateStore::take` devuelve un future para que los stores puedan hablar con una red. `MemoryRateStore::take`
  sigue siendo síncrono para uso directo.
- TLS (`rediss://`) no está incorporado.
- Las pruebas se ejecutan contra un servidor real cuando `ZEYN_TEST_REDIS` está definida:

  ```
  docker run -d --rm --name zeyn-redis -p 6379:6379 redis:7-alpine
  ZEYN_TEST_REDIS=redis://127.0.0.1:6379 cargo test -p zeyn --features redis --test redis
  docker stop zeyn-redis
  ```

## Logs y trazas

El framework informa mediante `tracing`. Nunca imprime por sí mismo.

- `App::serve` y `App::serve_until` instalan un subscriber a menos que la app haya instalado uno antes:
  los eventos van al error estándar como líneas de texto, filtrados por `log.level` (`info` de forma predeterminada; cualquier
  filtro de target de `tracing` como `warn,zeyn=debug`). Una app que quiera líneas JSON u otro
  destino instala su propio subscriber antes de servir, y Zeyn lo deja en paz.
- En `info` la salida es la de siempre: la dirección de escucha, las advertencias de arranque
  (traducciones que faltan, `DevAuth`, `dev_latency`), las fuentes de licencia que fallan y los errores internos.
  En `debug`, cada llamada añade una línea `call finished` con el tipo, la pantalla, la action, el id de usuario, el id de
  tenant, el resultado, el código de error y los milisegundos.
- Spans, para backends de trazas:

  | Span | Nivel | Campos |
  |---|---|---|
  | `request` | info | método, ruta (sin query string), estado, ms, y una vez conocidos: tipo, pantalla, action, usuario, tenant, ok, error |
  | `run` | info | código de la aplicación: una action, un origen de datos (obtención del grid, búsqueda de fila, view store), un constructor de pantalla |
  | `interceptor` | debug | nombre (el nombre del tipo, o `Interceptor::name`), etapa (`before` o `after`) |

- Con la feature de Cargo `otel`, los spans se exportan por OTLP/HTTP cuando `otel.endpoint` está definido
  en `Config` (`http://collector:4318`; los spans van a `/v1/traces`). `otel.service_name` da nombre al
  servicio, el nombre de la app de forma predeterminada. Los spans se agrupan por lotes en un hilo en segundo plano y se vuelcan cuando
  `serve_until` se detiene.
- Sin exportación, el error estándar recibe solo eventos, por lo que no se construye ningún span y los spans cuestan
  una comprobación en caché cada uno. Las apps montadas con `App::router` en otro servidor, y las pruebas, pagan lo
  mismo.
- Lo que nunca se registra: valores de campos (enmascarados o no), entrada de formularios y de API, query strings,
  cookies, cabeceras y secretos. Una prueba ejecuta grids, guardados de formularios, llamadas de API y revelados bajo un
  subscriber de nivel trace y comprueba que no aparece ningún valor enmascarado. Dos cosas quedan en manos de la app:
  las rutas se registran, así que no pongas datos personales en las rutas, y los mensajes de `Error::Internal` se
  registran tal como se escribieron.

## Documentos PDF

La feature de Cargo `pdf` renderiza documentos PDF a partir de plantillas de [Typst](https://typst.app). Está desactivada
de forma predeterminada porque Typst es grande (consulta las cifras más abajo).

```rust
const INVOICE: PdfTemplate = PdfTemplate::new("invoice", include_str!("../templates/invoice.typ"));

#[action(permission = "sales.order.read")]
pub async fn invoice_data(p: OrderParams, store: Orders, ctx: Ctx) -> Result<InvoiceData> { ... }

Screen::window("Order").body(PdfLink::new("Print invoice", INVOICE, invoice_data, params))

App::new("Shop").service(Pdf::from_config(&config)?)
```

- `PdfLink` coloca un botón de descarga en una pantalla. `GET /_zeyn/pdf` abre la pantalla con sus
  comprobaciones de acceso, ejecuta la action para el usuario (se aplican su permiso y los interceptores, el tipo de
  llamada es `Document`) y renderiza la plantilla con lo que devuelve la action. Los usuarios que no pueden
  ejecutar la action no ven el botón.
- Los grids con `exportable()` también imprimen un informe de lista desde `Export::Pdf` de la barra de herramientas
  (`GET /_zeyn/grid/export?format=pdf`): las columnas, etiquetas, filtros y orden del usuario, números
  alineados a la derecha, una fila de encabezado repetida y números de página. Un informe contiene como máximo 2.000 filas
  (`MAX_PDF_ROWS`, o el límite de exportación del grid cuando es menor) y lo indica cuando se dejaron filas fuera.
- **Los datos nunca se convierten en código Typst.** La salida de la action se serializa a JSON y la plantilla
  la lee como `data` (el renderizador enlaza `#let data = json("/data.json")` antes de la plantilla).
  Valores como `#panic()`, `]` o `$x$` se imprimen como texto; una prueba lo comprueba. Las plantillas no pueden
  leer nada más: ni archivos, ni paquetes.
- El renderizado se ejecuta en un hilo de bloqueo. Como máximo `pdf.max_concurrent` documentos (de 1 a 64; dos de forma
  predeterminada, uno en una sola CPU) se renderizan al mismo tiempo; el resto espera un hueco. La caché de
  memoización de Typst se vacía tras cada documento, de modo que la memoria vuelve a bajar.

| Clave | Significado |
|---|---|
| `pdf.fonts` | Un directorio de fuentes adicionales `.ttf`, `.otf`, `.ttc` u `.otc`, leído una vez al arrancar |
| `pdf.max_concurrent` | Documentos renderizados al mismo tiempo |

Fuentes: Libertinus Serif (regular, negrita, cursiva, negrita cursiva; SIL Open Font License) viene incorporada y
cubre el turco; DejaVu Sans Mono viene incorporada como alternativa para los signos que Libertinus no tiene, como
`₺`. Sus licencias están en `crates/zeyn/assets/fonts`. No hay fuente matemática; añade una con
`pdf.fonts` si las plantillas definen ecuaciones.

Medido en Windows (compilación release, x86_64):

| | Resultado |
|---|---|
| Tamaño del binario | 5.5 MB sin `pdf`, unos 46 MB con él (Typst añade unos 39 MB, las fuentes 1.6 MB) |
| Un documento de una página | de 4 a 7 ms, menos de 1 MB de heap durante el renderizado, archivo de 10 KB |
| Un informe de lista de 1.000 filas (25 páginas) | de 0.54 a 0.74 s, 51 MB de heap máximo durante el renderizado, 0 conservados después, archivo de 0.87 MB |
| Primer documento tras el arranque | Unos 42 MB de heap permanecen para las tablas construidas de forma perezosa por el propio Typst |
| Fuentes | 0.25 MB de heap para el índice de fuentes |

Una compilación debug es unas diez veces más lenta (un informe de 2.000 filas tardó 71 s en el showcase).

## Correo electrónico

La feature de Cargo `email` envía notificaciones con [lettre](https://lettre.rs).

```rust
let mailer = Mailer::from_config(&config)?;
App::new("People").mailer(mailer)

#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(r: TransitionRequest<LeaveRequestId>, store: Leaves, mailer: Mailer, ctx: Ctx) -> Result<LeaveState> {
    let state = leave_flow().apply(&ctx, current, &r.transition, r.comment.as_deref())?;
    mailer.send(&ctx, Mail::transition(&ctx, &manager_email, &employee)?)?;
    Ok(state)
}
```

- `Mailer::send` renderiza el mensaje y lo coloca en una cola en memoria (como máximo 10.000
  mensajes), y retorna. El trabajo en segundo plano `zeyn.mail` envía la cola de inmediato, reintenta los mensajes
  fallidos con backoff (cinco veces a partir de diez segundos, salvo que `Mailer::retry` diga otra cosa)
  y revisa la cola cada minuto. Un mensaje se descarta, con un error en el log, tras ocho
  intentos fallidos. Como todo trabajo, una cola que no se ha enviado cuando el proceso se detiene se pierde;
  la entrega duradera necesita la cola respaldada por la base de datos de la capa de datos.
- Los mensajes provienen de los catálogos de i18n: `<template>.subject`, `<template>.text` y, opcionalmente,
  `<template>.html`, con argumentos `{name}`. El idioma del destinatario es el del remitente, salvo que
  `Mail::locale` lo defina. Todo mensaje tiene una parte de texto simple y una HTML; en la parte HTML cada
  argumento se escapa, y sin una entrada `.html` el HTML es el texto escapado. Se eliminan los saltos de línea
  de los asuntos y las direcciones se analizan, de modo que los datos no pueden añadir cabeceras ni destinatarios.
- `Mail::transition` escribe el movimiento que la llamada acaba de hacer con `Workflow::apply` (registro, origen, destino,
  quién y el comentario) con la plantilla incorporada `zeyn.mail.transition`. El showcase avisa al
  responsable cuando se mueve una solicitud de permiso.

| Clave | Significado |
|---|---|
| `mail.from` | Remitente, como `Northwind <no-reply@example.com>` (obligatorio) |
| `mail.transport` | `smtp`, `file` (un archivo `.eml` por mensaje, para desarrollo y pruebas) o `log` (una línea de log con el número de destinatarios y el tamaño, nunca direcciones ni contenido; el valor predeterminado) |
| `mail.smtp.host`, `mail.smtp.port` | El servidor SMTP |
| `mail.smtp.tls` | `starttls` (predeterminado), `tls` (TLS implícito, normalmente el puerto 465) o `none` (solo relés locales) |
| `mail.smtp.user`, `mail.smtp.password` | Credenciales; lee la contraseña de una fuente secreta, como el entorno |
| `mail.file.dir` | Directorio del transporte `file` |

TLS usa rustls con los certificados raíz de Mozilla (`webpki-roots`).
