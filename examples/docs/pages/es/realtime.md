# Canal en tiempo real

Las solicitudes de Zeyn no tienen estado: el servidor no mantiene ningún socket ni estado de pantalla por usuario. Algunas
pantallas necesitan más: dos personas editando un diagrama, o una lista que debería actualizarse cuando otra persona
guarda. La feature de Cargo `realtime` añade para eso un canal WebSocket opcional. Las apps que no
lo activan no compilan nada de él, y las páginas que no lo usan no cargan nada.

WebSocket suele estar bloqueado en las redes corporativas, así que trata el canal como una mejora: toda
pantalla debe seguir funcionando sin él (el editor de diagramas sigue guardando mediante su action; una lista
sigue actualizándose cuando el usuario la abre de nuevo).

## 1. Activarlo

```toml
zeyn = { version = "...", features = ["realtime"] }
```

```rust
let realtime = Realtime::new()
    .documents("diagram", "diagram.read", "diagram.write", MyDocStore::new(pool))
    .topic("orders", "sales.order.read")
    .limits(Limits::default());
let hub = realtime.hub();          // clone it into stores that notify
App::new(..).realtime(realtime)    // also registers `Hub` as a service
```

## 2. Endpoints

| Endpoint | Propósito |
|---|---|
| `GET /_zeyn/live/doc/{kind}/{doc}?csrf=TOKEN` | WebSocket: un documento colaborativo |
| `GET /_zeyn/live/topics?t=a,b&csrf=TOKEN` | WebSocket: notificaciones de cambios para topics |
| `GET /_zeyn/live.js` | El script de topics, enlazado solo por las páginas que se suscriben |

Cada handshake se comprueba antes de la actualización, en este orden:

1. El canal está activado (`404` en caso contrario).
2. **Origin:** la cabecera `Origin` debe nombrar el host al que llegó la solicitud, o un origen añadido con
   `Realtime::allow_origin`. Los navegadores la envían siempre en los handshakes de WebSocket; si falta,
   se rechaza. Esto es lo que impide que otro sitio abra un socket con las cookies del usuario.
3. **Token CSRF:** los navegadores no pueden establecer cabeceras en un WebSocket, así que el token de
   `meta[name=zeyn-csrf]` va en el parámetro de consulta `csrf` y se compara en tiempo constante con
   la cookie `zeyn_csrf`, exactamente como se hace con `x-zeyn-csrf` en los `POST`. Los fallos responden `403`.
4. **Identidad:** el mismo autenticador que en cada solicitud; el usuario debe haber iniciado sesión (`403`).
5. **Límite por usuario** (`429`).
6. Documentos: un kind declarado y un id simple (de 1 a 64 letras, dígitos, `-`, `_`, `.`), si no `404`;
   el permiso de lectura del kind (`403`). Topics: cada topic declarado y su permiso concedido (`403`).
7. **Interceptores:** los interceptores de la app se ejecutan con una llamada de tipo `Live` (`screen` es el
   kind del documento, vacío para los topics), de modo que una regla de toda la app como `RequireMfa` o la comprobación de una cuenta
   suspendida cubre también los canales en vivo. Un rechazo responde con su estado.
8. Límites de salas: salas en memoria y clientes por sala (`503` con `Retry-After`).

El permiso de escritura se comprueba una vez por conexión: un cliente sin él recibe actualizaciones pero se
cierra con `1008` en el momento en que envía una.

## 3. Documentos y Y-CRDT

Los documentos son documentos [Y-CRDT](https://docs.yjs.dev) mantenidos con `yrs`. Las salas se identifican por
**tenant, kind e id de documento**, de modo que dos tenants nunca comparten una sala ni siquiera con los mismos ids.

Los frames son binarios. El primer byte es una etiqueta; `1` (`FRAME_UPDATE`) lleva una actualización Y en la codificación v1.

- Al conectar, el servidor envía el documento completo como una sola actualización.
- Un cliente envía actualizaciones por sus propios cambios; el servidor aplica cada una (un frame que no es una
  actualización válida cierra el socket con `1008`) y pasa los mismos bytes a todos los demás clientes de
  la sala.
- Tras cada (re)conexión, la isla devuelve todo su estado, de modo que las ediciones hechas sin conexión se fusionan.
  Las actualizaciones Y son idempotentes y conmutativas, así que el orden y las repeticiones no importan.

La persistencia pasa por un trait:

```rust
pub trait DocStore: Send + Sync + 'static {
    fn load(&self, key: RoomKey) -> BoxFuture<Result<Option<Vec<u8>>>>;
    fn save(&self, key: RoomKey, state: Vec<u8>) -> BoxFuture<Result<()>>;
}
```

`load` se ejecuta cuando se abre una sala (un error rechaza la conexión con `503`, que es también como un
store limita qué documentos existen). `save` recibe el documento completo como una sola actualización y se ejecuta
cada `persist_every` mientras el documento tiene cambios, y antes de descartar una sala inactiva. Un guardado
fallido se reintenta en la siguiente ronda y la sala se mantiene. `MemoryDocStore` es para pruebas y demos.

En el lado de la isla, `zeyn-island` con la feature `realtime` proporciona `live::Channel`:

```rust
let channel = Channel::document("diagram", "main", on_open, on_update, on_state)?;
channel.send_update(&update);   // after a local change
channel.close();                // in unmount
```

Se reconecta tras 1 segundo, duplicando hasta 30 segundos, y se detiene tras `close` o un `1008`.

## 4. Topics: pantallas que se actualizan en su sitio

```rust
Live::new("orders").add(DataGrid::of(search_orders)...)

#[action(permission = "sales.order.write")]
pub async fn save_order(o: Valid<Order>, store: Orders, hub: Hub, ctx: Ctx) -> Result<Order> {
    let saved = store.save(o.into_inner())?;
    hub.notify(&ctx, "orders");
    Ok(saved)
}
```

`Live` renderiza `div[data-zeyn-live="orders"]`. Una página cuya pantalla contiene uno enlaza
`/_zeyn/live.js` (una pantalla que llega mediante `nav.js` recibe `x-zeyn-live: 1` y el script se
importa entonces). El script abre un socket para todos los topics de la página. Ante una notificación, cada
elemento suscrito recarga sus grids en su sitio (`zeyn:reload`, la página actual del grid, el orden y
los filtros), o la pantalla entera en su sitio (`zeyn:refresh`) cuando no contiene ningún grid. Las notificaciones dentro de
300 ms se tratan una sola vez.

Las notificaciones llevan solo el nombre del topic, nunca datos: la actualización pasa por el endpoint
normal del grid con todas las comprobaciones de permisos. `notify` llega solo al tenant de quien llama;
`notify_tenant(Some(id), topic)` nombra uno, `None` es la app sin multitenencia.

## 5. Memoria y contrapresión

Cada pieza de estado tiene un límite (`Limits`, valores predeterminados entre corchetes):

| Límite | Predeterminado | Cuando se alcanza |
|---|---|---|
| sockets `per_user` | 8 | handshake `429` |
| clientes `per_room` | 32 | handshake `503` |
| `rooms` en memoria | 256 | handshake `503` |
| `message_bytes` de un cliente | 64 KB | socket cerrado |
| `doc_bytes` por documento | 1 MB | la actualización se rechaza y el socket se cierra (`1009`) |
| `queue` por cliente | 32 mensajes | el cliente se expulsa de la sala y se cierra con `1013` |
| sala `idle` sin clientes | 30 s | persistida y descartada de la memoria |
| `heartbeat` | 20 s | un ping; un cliente en silencio durante 3 intervalos se cierra |
| `persist_every` | 2 s | se guardan los documentos modificados |

Las difusiones nunca esperan: una actualización se ofrece a la cola acotada de cada cliente con `try_send`. Un
cliente cuya cola está llena se elimina de la sala de inmediato, de modo que un lector lento no puede retener memoria
ni ralentizar a los demás; se reconecta y recibe el documento completo. Un suscriptor de topic con la cola llena
solo se pierde esa notificación (la siguiente lo actualiza de todos modos).

Por tanto, la memoria en el peor caso es de unos `rooms * doc_bytes` para los documentos más
`clients * queue * message_bytes` para las colas (una actualización se comparte entre colas, no se copia). El
servidor del showcase con dos pestañas en vivo sobre el diagrama usa unos 19 MB en total (compilación debug).
`Hub::stats()` informa de salas, clientes, suscriptores y bytes de documentos para la monitorización.

## 6. Notas de seguridad

- El canal nunca ejecuta actions: los documentos son datos que la app lee mediante su `DocStore`, que
  es donde los valida (el showcase acota cada diagrama guardado con `Diagram::clamped`).
- Un documento es legible por cualquiera con el permiso de lectura del kind en el tenant; usa kinds
  separados (o ids que el store rechace) para documentos con audiencias distintas.
- La política de cada página permite WebSocket al propio origen de la app mediante `default-src 'self'`;
  no se relaja nada de la content security policy para el canal.

## 7. Ejemplo

`examples/showcase`: abre Islands, Diagram editor en dos pestañas. Mover o añadir una forma en una pestaña
aparece en la otra, y el grid "Shapes stored on the server" de abajo se actualiza en su sitio en
unos dos segundos, cuando el documento se guarda mediante `DocStore` y se notifica el topic `diagram`.

Límites de la versión actual: las salas viven en un solo proceso (varias instancias de servidor necesitan sesiones
persistentes (sticky sessions) o un broker compartido), la presencia (quién más está aquí, cursores) no está incorporada, y un documento
se envía completo en cada conexión.
