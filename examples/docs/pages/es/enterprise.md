# Funciones empresariales

Funciones transversales para aplicaciones de negocio: auditoría de datos, protección de datos personales, despliegues seguros,
comprobaciones de estado, seguridad de sesión, importación de CSV y XLSX, trabajos en segundo plano y flujos de aprobación. Todo se
comprueba en el servidor; el runtime del navegador solo lo hace agradable de usar. KVKK y GDPR son las
leyes de privacidad que se tienen presentes, pero nada de lo que sigue es específico de un país o de un sector.

## Auditoría de datos a nivel de campo

`AuditLog` escribe un `AuditEntry` por cada envío de formulario y por cada llamada a una action marcada con `audit`. En
los guardados, la entrada también nombra el registro y cada campo que cambió:

```json
{"kind":"form","screen":"customer","action":"save_customer","ok":true,"user":"ayse",
 "model":"Customer","record":"2","changes":[{"field":"name","old":"Customer 2","new":"Renamed"}]}
```

- Los formularios lo hacen por sí solos: los valores antiguos provienen del registro que la pantalla cargó en el
  servidor, y los nuevos del `Valid<M>` validado. Nada de lo que envía el cliente se usa como valor
  antiguo, y los campos que el formulario no toma del cliente nunca aparecen como modificados.
- Los guardados por API registran el modelo y la clave a partir de su entrada `Valid<M>`. Solo la action sabe qué hay en
  la base de datos, así que es ella quien informa de la diferencia:

  ```rust
  #[action(permission = "crm.customer.write", audit)]
  pub async fn save_customer(c: Valid<Customer>, store: Customers, ctx: Ctx) -> Result<Customer> {
      let current = store.get(c.id)?;
      ctx.record_changes(&current, &c);
      store.save(c.into_inner())
  }
  ```

  Hacer lo mismo en las actions de guardado de formularios es válido: el informe de la action reemplaza al del formulario.
- Los valores son cadenas de texto simples e independientes del idioma (`1234.56 TRY`, `2024-05-01`, valores de choice).
  Las versiones de fila se omiten. Los campos enmascarados siguen enmascarados (véase más abajo).
- Un guardado fallido sigue nombrando el registro, sin cambios.

## Enmascaramiento de campos

Marca con `#[mask]` los campos de texto que contengan datos personales. Aquí el documento de identidad se muestra como `•••• 8901`, el correo como `p••••@example.com` y las notas como `••••`:

```rust
#[entity(table = "people")]
pub struct Person {
    #[key]
    pub id: PersonId,
    pub name: String,
    #[mask(last4)]
    pub national_id: String,
    #[mask(email, permission = "hr.contact.unmask")]
    pub email: Option<String>,
    #[mask]
    pub notes: Option<String>,
}
```

- El enmascaramiento ocurre en el servidor: en las celdas del grid, los formularios, la salida de la API, las herramientas de agentes y, por tanto, en
  las exportaciones CSV, que se construyen a partir de las celdas. El valor sin procesar nunca llega a un navegador ni a un cliente de API
  sin el permiso.
- El permiso para desenmascarar es `<model>.<field>.unmask` en snake case (`person.national_id.unmask`)
  salvo que el atributo indique otro.
- Los usuarios con el permiso siguen viendo primero el texto enmascarado, con un botón para revelar. Revelar llama a
  `POST /_zeyn/reveal`, que vuelve a comprobar el permiso y escribe un registro de auditoría (kind
  `reveal`, campo en `revealed`). Los intentos rechazados también se auditan.
- Los clientes de API con el permiso reciben los valores sin procesar; cada uno de esos campos aparece en `revealed` del
  registro de auditoría de la llamada.
- Los formularios muestran los campos enmascarados como texto de solo lectura. El valor almacenado se conserva al guardar, salvo que el usuario
  pueda revelarlo y lo haya cambiado después de revelarlo. Un campo enmascarado vacío es una entrada normal, de modo que el personal
  de captura de datos puede rellenarlo una vez sin poder volver a leerlo después.
- Los filtros y las ordenaciones sobre un campo enmascarado se ignoran para los usuarios que no pueden revelarlo, de modo que un valor
  no se pueda adivinar buscando. Esto se aplica a cada lectura de un grid (páginas, exportaciones, acciones masivas
  sobre todas las filas, vistas guardadas, herramientas de agentes) y a las actions de API que reciben un `DataQuery`.
  No añadas `.filter()` a las columnas enmascaradas.
- Los cursores de paginación nunca contienen el valor de un campo enmascarado. Una lista ordenada por un campo enmascarado pagina por
  posición en lugar de por clave.
- Un grid nunca agrupa por un campo enmascarado; `group_by` sobre uno se ignora y se registra en el log.
- Las salidas de islas y de otras fuentes de datos se enmascaran a cualquier profundidad, igual que la salida de la API: los campos enmascarados de
  cada modelo contenido salen como texto enmascarado, y las claves de los modelos sensibles se registran como lecturas.
- La revelación en el grid busca la fila mediante la action del grid con un filtro sobre la clave, por lo que la action
  debe respetar los filtros sobre su campo clave.
- `#[mask]` solo compila sobre `String` y `Option<String>`.

## Lecturas de registros sensibles

Se registra quién vio qué datos personales y cuándo, además de quién los cambió.

- `#[entity(table = "cases", sensitive)]` (o `#[sensitive]` debajo de `#[entity]`) marca un modelo.
  Cada vez que se muestra un registro así a un usuario, el registro de auditoría de la llamada lo enumera en `reads`
  (`{"model":"Case","id":"2"}`): filas de una página de grid, un formulario, el resultado de una herramienta de agente y la salida
  de la API. Las lecturas se registran donde se renderizan los datos, así que los componentes dentro de layouts también cuentan.
- `#[screen(..., sensitive)]` registra cada apertura de la pantalla como una lectura cuyo `model` es el
  nombre de la pantalla y cuyo `id` son sus parámetros, para pantallas que muestran datos personales no ligados a un solo modelo.
- `#[screen(..., reason)]` además pregunta al usuario el motivo antes de mostrar cualquier dato. El servidor responde
  `428` con el código `reason_required` (una página con un formulario de motivo en las cargas de página completa) a la página,
  sus fragmentos, y a las llamadas del grid, del formulario y de revelación hasta que se da un motivo. El runtime lo pregunta en un
  cuadro de diálogo, envía `POST /_zeyn/reason {screen, reason}` (de 3 a 300 caracteres) y repite la
  petición. El motivo se conserva durante 15 minutos en una cookie `HttpOnly`, `SameSite=Strict`, solo
  abre la pantalla para la que se dio y se escribe en `reason` de cada registro de auditoría de esa
  pantalla. Los agentes y los clientes de API no pueden dar un motivo, por lo que no pueden abrir estas pantallas.
- Las exportaciones (CSV, XLSX, PDF) de un grid sobre un modelo sensible escriben un registro de auditoría por archivo con
  `export` establecido: el modelo, el formato, el número de filas y la ordenación y los filtros solicitados.
  No enumeran cada fila en `reads`, así que una exportación grande sigue siendo barata; las exportaciones de otros modelos
  no cuestan nada extra. Exportar desde una pantalla `sensitive` o `reason` también registra la lectura de la pantalla.
- Las partes diferidas de una pantalla (`POST /_zeyn/node`, usado por las pestañas diferidas) ejecutan los interceptores y registran
  lecturas exactamente igual que al abrir la pantalla.
- `ctx.record_read(&record)` registra una lectura desde el código de la aplicación.
- `AuditLog` escribe los registros de lectura para cualquier llamada, incluidas las cargas de página, en cuanto se mostró un registro
  sensible. Sin un interceptor de auditoría no se registra nada; las aplicaciones que guardan datos
  personales siempre deberían registrar uno.

## Flujos de aprobación

Un flujo de trabajo es una máquina de estados pequeña y tipada. Los estados son un enum `#[choice]`; cada transición nombra su
estado inicial, su estado final y el permiso que necesita:

```rust
#[choice]
pub enum LeaveState { Draft, Submitted, Approved, Rejected }

pub fn leave_flow() -> Workflow<LeaveState> {
    Workflow::new()
        .transition("submit", LeaveState::Draft, LeaveState::Submitted, "people.leave.request")
        .transition("approve", LeaveState::Submitted, LeaveState::Approved, "people.leave.approve")
        .transition("reject", LeaveState::Submitted, LeaveState::Rejected, "people.leave.approve")
}

#[action(permission = "people.leave.request", audit)]
pub async fn move_leave(req: TransitionRequest<LeaveRequestId>, store: LeaveStore, ctx: Ctx) -> Result<LeaveState> {
    let mut leave = store.get(req.id)?;
    leave.state = leave_flow().apply(&ctx, leave.state, &req.transition, req.comment.as_deref())?;
    store.save(leave)
}
```

- `apply` se ejecuta en el servidor con el estado leído del almacén. Una transición desconocida da `404`, un
  registro que ya no está en el estado inicial da `409` (alguien lo movió antes), un permiso ausente da
  `403` y un comentario de más de 1000 caracteres da `422`.
- Cada transición se escribe en el registro de auditoría en `transition`: nombre, origen, destino y comentario; el usuario
  y la hora están en el propio registro.
- `Transitions::of(flow, state, id, action)` muestra el estado actual, un comentario opcional y un
  botón por cada transición que el usuario puede hacer desde ese estado (`Workflow::allowed`). La clave del registro la toma
  la pantalla en el servidor, así que un cliente no puede mover otro registro. Los agentes lo ven como una herramienta de formulario.
- Las etiquetas de los botones provienen de `workflow.<name>` en los catálogos; `.label(...)` establece una a mano.
- `Workflow::check` informa de permisos no válidos y de transiciones declaradas dos veces desde el mismo estado;
  llámalo en una prueba.
- Las solicitudes de permiso del showcase usan esto: guardar como borrador, enviar y luego aprobar o rechazar.

## Despliegues seguros

- Cada respuesta lleva `x-zeyn-build`, y cada página `<meta name="zeyn-build">`. El id es
  `ZEYN_BUILD_ID` cuando está definido (letras, dígitos, `.`, `_`, `-`, hasta 64), y si no, un hash del
  ejecutable. `zeyn::server::build_id()` lo devuelve, para los logs.
- Cuando el runtime ve una respuesta de otra compilación, muestra un aviso cortés: hay una versión nueva
  disponible, recarga cuando estés listo. Nunca recarga por sí mismo.
- Lo que los usuarios escriben en los formularios se conserva en `sessionStorage` por pantalla, parámetros y formulario, y se restaura
  tras una recarga con una nota y un botón para descartarlo. Las entradas enmascaradas, de contraseña, de archivo y ocultas
  nunca se conservan. El borrador conserva la versión de fila sobre la que se escribió, de modo que guardar un borrador de un registro
  que alguien cambió entretanto se rechaza como conflicto. Un guardado correcto borra el borrador;
  los borradores de más de un día se descartan. `sessionStorage` vive solo en esa pestaña del navegador.

## Comprobaciones de estado

- `GET /_zeyn/health` responde `200 ok` mientras el proceso se ejecuta. Úsalo para liveness.
- `GET /_zeyn/ready` ejecuta a la vez todas las comprobaciones añadidas con `App::readiness(name, check)`,
  cinco segundos cada una, y responde `200 {"status":"ready"}` o
  `503 {"status":"not_ready","failed":["database"]}`. Solo se muestran los nombres de las comprobaciones, nunca los errores.
- Ambos viven bajo `base_path` (`/billing/_zeyn/health`), responden con `Cache-Control: no-store`
  y no requieren inicio de sesión.
- El resultado de readiness se reutiliza durante un segundo (`READY_CACHE`), y solo una petición a la vez
  ejecuta las comprobaciones, así que una avalancha de sondas no puede cargar la base de datos.
- Health nunca tiene límite de frecuencia, de modo que una sonda muy activa no puede bloquearse a sí misma y reiniciar una aplicación sana.
  Readiness cuenta para los límites de frecuencia salvo que provenga de una dirección de `probe_ips`.
- Las reglas de IP siguen aplicándose: `deny_ips` siempre, `allow_only_ips` también salvo que la dirección de la sonda esté indicada
  en `App::probe_ips([...])`, que abre solo estos dos endpoints para ella.

## Latencia de desarrollo

`App::dev_latency(Duration::from_millis(400))` retrasa cada llamada (páginas, fragmentos, grids, formularios,
API, MCP y llamadas del runtime; no los recursos estáticos ni las sondas de estado) para que los desarrolladores vean los estados de carga
y los doble clics como los ven los usuarios con una red lenta. Funciona solo en compilaciones de depuración: en las compilaciones
release el método no hace nada y el código del retraso se elimina en la compilación, de modo que dejarlo por error
no puede ralentizar producción. Se imprime una advertencia al iniciar mientras está activo.

## Seguridad de sesión

El inicio de sesión proviene de un proxy de inicio de sesión único, del inicio de sesión OpenID Connect integrado o de bearer tokens
(véase `docs/auth.md`), de modo que estas funciones solo necesitan que el autenticador indique quién es el usuario y, para la reautenticación, cuándo inició sesión
(`Identity::authenticated_at`; `ForwardAuth::auth_time("x-auth-time")` lee segundos Unix o
RFC 3339; `DevAuth` informa cada petición como recién iniciada).

- `App::idle_timeout(Duration)` hace que el runtime avise al usuario `idle_warning` antes del final
  (un minuto por defecto) en un cuadro de diálogo de alerta con una cuenta atrás y un botón "seguir conectado", que
  llama a `POST /_zeyn/keepalive` para que un proxy situado delante vea actividad. Cuenta la entrada en cualquier pestaña de la aplicación.
  Al final el usuario va a `logout_url`, o si no a `login_url` con `return_to`, o si no
  ve una pantalla de sesión cerrada. Los borradores de formularios se conservan. El tiempo de inactividad solo cierra pantallas olvidadas;
  terminar la sesión real es tarea del proveedor de identidad.
- `#[action(permission = "...", reauth)]` exige un inicio de sesión dentro de `App::reauth_within` (cinco
  minutos por defecto). En caso contrario el servidor responde `401` con el código `reauth_required` (tipo de problema
  `urn:zeyn:problem:reauth_required` en la API), sea cual sea lo que envíe el cliente. El runtime pide
  al usuario que inicie sesión de nuevo en una pestaña nueva (`reauth_url`, o si no `login_url`) y repite la petición
  cuando vuelve, de modo que no se pierde nada de lo escrito. Las identidades sin hora de inicio de sesión nunca pasan.

## Importación de CSV

```rust
#[action(permission = "people.leave.import", audit)]
pub async fn import_leaves(rows: Vec<Valid<LeaveRequest>>, store: LeaveStore) -> Result<usize> { ... }

Screen::new("Import").body(Import::of(import_leaves).max_rows(1000))
```

- El usuario elige un archivo CSV. El runtime lo lee como UTF-8, o como Windows-1254 cuando no es
  UTF-8 válido (hojas de cálculo turcas), y envía el texto en un cuerpo JSON a `POST /_zeyn/import`
  (sin necesidad de multipart; como máximo 8 MB). El servidor responde con un resumen, una vista previa de las diez
  primeras filas, los errores por fila y un informe de errores descargable.
- Análisis: se descarta la marca de orden de bytes, se detecta `,`, `;` o tabulador a partir de la línea de encabezado, se admite el entrecomillado con `"` con
  separadores y saltos de línea dentro, y `\r\n` o `\n`.
- Los límites se comprueban mientras se lee el archivo, no después: la lectura se detiene en el límite de filas, se rechaza un encabezado
  de más de 512 celdas, y las filas de datos conservan como máximo una celda más allá del encabezado (de modo que los valores
  sobrantes se siguen notificando). Un archivo XLSX lee como máximo tantas columnas como tiene el encabezado, su
  encabezado puede nombrar como máximo tantas columnas como el modelo puede importar, y se rechaza una parte de cadenas compartidas
  de más de 16 MB antes de descomprimir nada.
- Las columnas se emparejan por nombre de campo o etiqueta en cualquier idioma de la aplicación, ignorando mayúsculas y minúsculas y la i turca
  con punto y sin punto. Se rechazan las columnas desconocidas, de solo lectura, de clave y de versión de fila.
- Cada fila se lee y se valida exactamente como un formulario del modelo: valores tal como los escribe un usuario de ese
  idioma (`1.234,56`, `05.12.2026`), las reglas del campo y el propio `validate` del modelo.
- La importación envía a la action solo las filas válidas; `.all_or_nothing()` no importa nada salvo que
  todas las filas sean válidas. Al importar, el servidor vuelve a leer y validar el archivo; no se confía en nada de la
  vista previa. Ambos pasos requieren el permiso de la action, y la importación se audita.
- `.max_rows(n)` limita las filas (5000 por defecto, nunca más de 50000). El informe de errores neutraliza las
  fórmulas de las hojas de cálculo. Los campos enmascarados se muestran enmascarados en la vista previa.

### Importación de XLSX

Con la feature de Cargo `xlsx`, el mismo componente también acepta libros de Excel (`.xlsx`). El
runtime envía el archivo en base64 (`"xlsx"` en lugar de `"csv"` en el mismo cuerpo JSON), y el
servidor lee la primera hoja con `calamine` hacia la misma validación fila por fila, vista previa e
informe de errores que en CSV.

- Las celdas conservan sus tipos: un número se lee como número y una celda de fecha como fecha, sea cual sea el idioma
  del usuario, de modo que `1234.5` en una sesión turca sigue siendo 1.234,50. Las horas de reloj en una celda de
  fecha se leen en la zona horaria del usuario, como las lee un formulario. Las choices se emparejan por valor o por etiqueta.
- Límites: como máximo 8 MB de archivo, y el archivo comprimido puede descomprimirse hasta un máximo de 64 MB en como máximo 1000 partes,
  comprobado a partir de los tamaños que declara el archivo antes de descomprimir nada. Las celdas se leen una a
  una y la lectura se detiene en el límite de filas, de modo que una hoja con un millón de filas no cuesta más que el
  límite. Un archivo que no es un libro, está protegido con contraseña o está truncado recibe un error claro.
- Solo se lee la primera hoja. Las fórmulas cuentan con su valor en caché.

## Trabajos en segundo plano

```rust
App::new("Billing")
    .job_concurrency(2)
    .job("send_reminders", Schedule::every(Duration::from_secs(3600)).retry(3, Duration::from_secs(30)), send_reminders)
    .job("rebuild_index", Schedule::manual(), rebuild_index)
    .jobs_permission("ops.jobs.read")
    .serve_until("0.0.0.0:3000", shutdown_signal())
```

- Un job es `async fn(Ctx) -> Result<()>`. Su contexto tiene los servicios, la configuración y el
  idioma predeterminado de la aplicación, y la identidad `job:<name>` sin permisos.
- `Schedule::every(d)` se ejecuta cada intervalo; `Schedule::manual()` solo cuando se dispara. Las actions
  disparan jobs mediante el servicio inyectable `Jobs`: `jobs.trigger("rebuild_index")`. Un job
  disparado mientras se ejecuta se ejecuta una vez más después; un job nunca se ejecuta dos veces a la vez.
- `.retry(times, first_delay)` vuelve a intentar una ejecución fallida, duplicando la espera hasta diez minutos.
- Como máximo `job_concurrency` jobs (cuatro por defecto) se ejecutan a la vez.
- El estado (estado, ejecuciones, fallos, último inicio y fin, último código de error) es `jobs.status()` en el código
  y `GET /_zeyn/jobs` para los usuarios con `jobs_permission`. Los detalles de los errores solo van al log del servidor.
- `App::serve_until(addr, signal)` deja de aceptar peticiones cuando la señal se completa, permite que las peticiones en curso
  terminen, cancela las esperas de reintento y da 30 segundos a los jobs en ejecución.
- Sin un almacén, los jobs están en memoria: el trabajo que está esperando o ejecutándose cuando el proceso se detiene
  se pierde, y con varios servidores cada uno ejecuta su propia programación.

### Jobs duraderos

```rust
App::new("Billing")
    .job_store(SqlJobStore::new(db))
    .queued_job("send_invoice", Schedule::manual().retry(5, Duration::from_secs(10)), send_invoice)
    .job("close_day", Schedule::every(Duration::from_secs(86_400)), close_day)

async fn send_invoice(ctx: Ctx, invoice_id: String) -> Result<()> { ... }
jobs.enqueue("send_invoice", "42").await?;
```

- `JobStore` guarda las ejecuciones: `enqueue`, `schedule`, `claim` con un arrendamiento (id del worker y hora de fin),
  `heartbeat`, `complete` y `fail`. `MemoryJobStore` (como máximo 10 000 ejecuciones, se descarta primero la
  ejecución fallida más antigua) es el predeterminado; `SqlJobStore` (features `postgres`, `oracle`) guarda las ejecuciones en
  `zeyn_jobs`. `create_job_tables(dialect, prefix)` da las sentencias para una migración.
- Con un almacén configurado, los jobs de `Schedule::every` guardan su próxima ejecución en el almacén, de modo que un reinicio no la
  mueve y solo un servidor ejecuta cada intervalo. `trigger` también encola una ejecución allí.
- Los jobs de `queued_job` reciben el payload de cada ejecución (como máximo `MAX_JOB_PAYLOAD`, 4000 bytes); los jobs
  normales también pueden encolarse e ignoran el payload. `enqueue_after` retrasa una ejecución.
- El claim toma las ejecuciones vencidas de los jobs que conoce este servidor, empezando por las más antiguas, y las retiene durante
  `JOB_LEASE` (60 segundos), renovado cada 20 segundos mientras la ejecución trabaja. `complete` y `fail`
  modifican una ejecución solo mientras el worker aún la retiene. Una ejecución cuyo worker se detuvo se reclama de nuevo
  una vez terminado su arrendamiento; el intento perdido cuenta, de modo que una ejecución que bloquea su servidor termina como fallida.
- Un intento fallido espera el backoff (se duplica, como máximo `MAX_BACKOFF_MS`, diez minutos) mientras queden intentos,
  y luego queda como `failed` con su código de error. Una programación pasa a su siguiente intervalo.
- Los workers buscan ejecuciones vencidas cada `JOB_POLL` (un segundo) y de inmediato tras un `enqueue` en el
  mismo servidor. Cada claim toma como máximo tantas ejecuciones como ranuras de job libres haya, de modo que la memoria
  no crece con la cola.
- PostgreSQL hace el claim en un solo `UPDATE ... WHERE id IN (SELECT ... FOR UPDATE SKIP LOCKED) RETURNING`.
  Oracle 11g y posteriores selecciona ids con `ROWNUM` y `FOR UPDATE SKIP LOCKED` en una transacción,
  los actualiza y los lee de vuelta antes del commit. Oracle no puede ordenar un select con bloqueo, así que
  las ejecuciones vencidas allí se toman sin un orden fijo. Las horas son milisegundos Unix en columnas `BIGINT` o
  `NUMBER(19)`.
- Con un almacén, una ejecución programada y una ejecución disparada del mismo job pueden solaparse; los jobs que no
  deben ejecutarse dos veces a la vez deben protegerse a sí mismos (con un bloqueo de fila, por ejemplo).
