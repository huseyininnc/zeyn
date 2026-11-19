# ZeynSocket

Una sola conexión en vivo por página lleva todo lo que el servidor envía: notificaciones de
topics que actualizan pantallas, eventos tipados para un usuario, un grupo o un tenant, y actions
que el navegador llama sin una solicitud de página. Se activa con la feature `realtime`:

```toml
zeyn = { version = "...", features = ["realtime"] }
```

```rust
App::new("Shop")
    .socket(
        ZeynSocket::new()
            .topic("orders", "sales.order.read")
            .event::<OrderShipped>("sales.order.read")
            .event::<MessageSent>("chat.read"),
    )
```

`App::socket` también registra el servicio `Socket`, así que las actions reciben `socket: Socket`.
El protocolo de transmisión, los límites y sus motivos están en `docs/zeynsocket.md`.

## Topics: pantallas que se actualizan

Un topic no lleva datos. Indica a las pantallas que muestran ciertos datos que los vuelvan a
cargar, a través de los endpoints normales y sus comprobaciones de permisos.

```rust
Live::new("orders").add(DataGrid::of(search_orders))

#[action(permission = "sales.order.write")]
pub async fn ship(o: OrderId, store: Orders, socket: Socket, ctx: Ctx) -> Result<()> {
    store.ship(&o)?;
    socket.notify(&ctx, "orders");
    Ok(())
}
```

Declara cada topic con `ZeynSocket::topic` y el permiso que necesita un suscriptor. Los nodos
marcados con `.listens("orders")` también se actualizan.

## Eventos: datos para algunos usuarios

Un evento es un tipo `Serialize` con un nombre estable. Cada receptor debe tener el permiso con el
que se declaró el evento; un grupo nunca lo concede.

```rust
#[derive(Serialize, Event)]
#[event(name = "chat.message")]
pub struct MessageSent {
    pub room: String,
    pub text: String,
}

socket.to_user(&ctx, "ayse").emit(&notice)?;          // every open page of one user
socket.to_group(&ctx, format!("room:{id}")).emit(&m)?; // members of a group
socket.to_tenant(&ctx).emit(&notice)?;                 // the whole tenant
socket.to_user_id(Some("acme"), "ayse").emit(&n)?;     // from a job, without a request
```

Todo queda dentro de un mismo tenant. Los eventos nunca llevan valores enmascarados: envía un id y
deja que el receptor cargue el registro mediante una action, que enmascara como siempre. Un evento
mayor que `Limits::event_bytes` (16 KB) o que nunca se declaró devuelve un error y no envía nada.

## Grupos

Una conexión se une a un grupo solo dentro de una action llamada a través del socket. En esa
action la app comprueba que el usuario puede estar en el grupo.

```rust
#[action(permission = "chat.read")]
pub async fn join_room(r: RoomId, conn: Conn, rooms: Rooms, ctx: Ctx) -> Result<()> {
    rooms.check_member(&ctx, &r)?;
    conn.join(format!("room:{}", r.0))
}
```

`Conn` existe solo en las llamadas a través del socket; la misma action llamada por HTTP responde
`422`. Una conexión está como mucho en `Limits::groups` (32) grupos.

## Pantallas sin JavaScript

```rust
List::of(messages).on_event::<MessageSent>(OnEvent::Refresh)
Alert::info(t!("orders.title")).on_event::<OrderShipped>(OnEvent::Toast(t!("orders.shipped")))
```

`OnEvent::Refresh` actualiza el nodo en su sitio cuando el evento llega a este usuario;
`OnEvent::Toast` muestra un aviso (toast). Solo llegan los eventos que el usuario puede recibir.

## Código del navegador

Las islas y los scripts propios usan `zeyn.socket`:

```js
const off = zeyn.socket.on('chat.message', (m) => show(m));
await zeyn.socket.call('send_message', { room, text }); // rejects with the action's problem
zeyn.socket.state; // 'connected' | 'reconnecting' | 'offline', also as zeyn:socket-state
```

Se pueden llamar las actions publicadas con `Feature::api`. Cada llamada pasa las mismas
comprobaciones que una llamada a la API: permiso, guards, interceptors (tipo de llamada `Live`),
validación, auditoría y el límite de escritura de la app para las actions que cambian datos.

## Una conexión que no se cae

El navegador se reconecta con pausas aleatorias cada vez más largas, y de inmediato cuando vuelve
a estar en línea. Tras una caída, el servidor conserva los topics, los grupos y los últimos
mensajes de la conexión durante `Limits::detached` (60 segundos) y reenvía lo que la página se
perdió; cuando eso no es posible, la página recarga sus partes en vivo. Donde los proxies bloquean
WebSocket, la página cambia por sí sola a Server-Sent Events para recibir y `POST` para enviar,
con los mismos mensajes.

## Varios servidores

```rust
let redis = RedisStore::new("redis://cache:6379", "shop")?;
ZeynSocket::new().redis(&redis)   // features `realtime` and `redis`
```

Cada instancia entrega a sus propias conexiones lo que publican las demás, así que no hacen falta
sesiones fijas (sticky sessions). Mientras Redis no está disponible, los envíos se quedan en la
instancia.

## Pruebas

La feature `socket-client` añade `TestSocket`, un cliente que habla con el socket como un
navegador:

```rust
let addr = TestSocket::serve(app.router()?).await;
let mut c = TestSocket::connect(addr, &[("authorization", "Bearer ..")]).await?;
c.call("join_room", json!({ "room": "1" })).await?;
let m: MessageSent = c.next_event().await?;
```

## Límites

| Límite | Valor por defecto |
|---|---|
| Sockets abiertos por usuario | 8 |
| Mensajes en espera por conexión | 32 |
| Llamadas por conexión por segundo | 20 |
| Frames por conexión por segundo | 100 |
| Frame más grande | 64 KB |
| Evento más grande | 16 KB |
| Búfer de reenvío por conexión | 64 mensajes, 64 KB |
| Tiempo que se conserva una conexión caída | 60 s |

Una conexión inactiva cuesta unos 16 KB, medidos con el servidor y el cliente en un mismo proceso.
