# Chat

Componentes para conversaciones: los mensajes de una conversación, la caja en la que escribe el usuario y el aviso de que alguien está escribiendo. Cada uno se renderiza en el servidor y carga su hoja de estilos y su script solo en las páginas que lo usan.

## MessageThread

`MessageThread` muestra los mensajes de una conversación como burbujas de chat: los mensajes propios del usuario en el lado final, los demás en el lado inicial (en espejo en los idiomas de derecha a izquierda), los mensajes consecutivos de un mismo remitente en menos de cinco minutos agrupados bajo un solo nombre, un encabezado para cada día en la zona horaria del usuario, la hora de cada mensaje y, en los mensajes propios, su estado de entrega.

```rust
#[action(permission = "chat.read")]
pub async fn conversation_messages(q: ThreadQuery, store: Chats, ctx: Ctx) -> Result<Page<Msg>> {
    store.check_member(&q.conversation, &ctx)?;
    Ok(q.query.apply(store.messages(&q.conversation)))
}

MessageThread::of(conversation_messages)
    .conversation(params.id.to_string())
    .author(Msg::author)
    .name(Msg::author_name)
    .text(Msg::body)
    .when(Msg::sent_at)
    .status(Msg::status)
    .attachments(Msg::files, CHAT_FILES)
    .live()
```

La acción del historial recibe un `ThreadQuery`: la conversación, que el servidor toma del componente (el navegador nunca la envía), y un `DataQuery` ordenado del más nuevo al más antiguo. Compruebe en la acción que el usuario participa en la conversación. `.author` es el id de usuario del remitente, que se compara con el usuario que ha iniciado sesión para distinguir los mensajes propios de los demás; `.name` es el nombre que se muestra. El modelo necesita un `#[key]`, de modo que un mensaje que se carga dos veces se muestra una sola vez.

Los primeros 50 mensajes (`THREAD_PAGE`) se renderizan con la pantalla. Al desplazarse hacia arriba se carga la siguiente página más antigua mediante `POST /_zeyn/grid`, comprobando cada vez el acceso de la pantalla y el permiso de la acción, y el mensaje que mira el lector se mantiene en su sitio. En la página quedan como máximo 200 mensajes: al avanzar más se descarta el extremo lejano, que se vuelve a cargar cuando el lector regresa, así que una conversación de miles de mensajes cuesta la misma memoria que una corta. Desplazarse por 5.000 mensajes funciona a la frecuencia de fotogramas de la pantalla con unos 2.000 elementos en la página.

`Delivery` tiene los estados `Sending`, `Sent`, `Delivered`, `Read` (la confirmación de lectura) y `Failed`, cada uno mostrado con un icono y un texto para lectores de pantalla. `.attachments(field, rules)` muestra los archivos de un mensaje (un campo `Vec<File>` y las reglas de un almacén registrado con `Feature::files`): las imágenes en una caja fija que se carga de forma diferida sobre un marcador de posición, los demás archivos como enlaces de descarga que pasan por el permiso de lectura del almacén.

Los mensajes nuevos se obtienen cuando se nombra uno de los topics del hilo, `"chat"` salvo que `.topic(..)` indique otro: después de que un `Composer` envíe, después de que una acción termine con `ui.refresh("chat")` o, con `.live()` y la feature `realtime`, cuando se ejecute `socket.notify(&ctx, "chat")` en cualquier lugar. Los mensajes de otros se anuncian en una región viva educada (polite).

### Respuestas de IA

```rust
#[action(permission = "chat.ai")]
pub async fn ask_model(r: ReplyRequest, store: Chats) -> Result<Reply> {
    let id = store.reserve_id();
    let (tx, reply) = Reply::channel(id.to_string());
    tokio::spawn(async move {
        let mut text = String::new();
        for piece in my_model_stream(&r.text).await {
            text.push_str(&piece);
            if !tx.send(piece).await {
                break;
            }
        }
        store.save_ai_answer(id, &r.conversation, text);
    });
    Ok(reply)
}

MessageThread::of(conversation_messages)
    .conversation(id)
    .ai(Msg::from_ai)
    .reply(ask_model)
```

Con `.reply(action)`, un mensaje enviado con un `Composer` de la misma conversación inicia una respuesta de IA: el navegador envía el texto a `POST /_zeyn/stream` (CSRF, el límite de escritura, el acceso de la pantalla, el permiso de la acción y los interceptores) y la respuesta vuelve en streaming como líneas JSON en el formato del asistente integrado. Los fragmentos pasan por un canal acotado (`REPLY_BUFFER`); el servidor se detiene tras `MAX_REPLY_CHARS` (32.000) caracteres o `REPLY_SECONDS` (120) segundos, y un botón de detener termina la respuesta antes. `ReplySender::send` devuelve `false` cuando el lector se ha ido, que es la señal para que el generador se detenga.

La respuesta se muestra como un pequeño subconjunto de Markdown construido solo con nodos de texto, de modo que nada de lo que escribe el modelo se interpreta nunca como HTML. Los mensajes anteriores nunca se vuelven a renderizar, solo el párrafo que se está escribiendo: una respuesta de 2.000 fragmentos cuesta por fotograma lo mismo al final que al principio, y nada de lo que hay encima se mueve. La región viva lee la respuesta una vez terminada, nunca cada fragmento. Los mensajes marcados con `.ai(field)` también se cargan desde el historial como Markdown seguro (con la feature `markdown`; sin ella, como texto plano).

La clave de la respuesta (`Reply::channel(key)`) es la clave que tendrá el mensaje guardado, así que el historial muestra la respuesta guardada en su lugar en vez de una segunda copia.

## Composer

`Composer` es la caja en la que el usuario escribe mensajes: un área de texto que crece con su texto hasta una altura máxima, un botón de enviar y, si se piden, adjuntos, menciones y un borrador guardado.

```rust
#[action(permission = "chat.write")]
pub async fn send_message(m: ChatSend, store: Chats, socket: Socket, ctx: Ctx) -> Result<()> {
    store.check_member(&m.conversation, &ctx)?;
    store.add(&m.conversation, &ctx, m.text, m.attachments, m.mentions)?;
    socket.notify(&ctx, "chat");
    Ok(())
}

pub const CHAT_DRAFTS: ClientPref<Draft> =
    ClientPref::draft("chat-draft", Duration::from_secs(7 * 86_400), Purpose::Preference);

Composer::of(send_message)
    .conversation(params.id.to_string())
    .attachments(CHAT_FILES)
    .mentions(search_people)
    .remember(CHAT_DRAFTS)
```

La acción de envío recibe un `ChatSend`: la conversación, que el servidor toma del componente, el texto recortado, los archivos adjuntos y los ids de las personas mencionadas. El envío pasa por `POST /_zeyn/data` con el token CSRF, cuenta para el límite de escritura y necesita el permiso de la acción; un composer con el que el usuario no puede enviar se omite de la página. El servidor rechaza un mensaje vacío, uno más largo que `.max_chars(n)` (4.000 si no se indica), más archivos de los que permiten las reglas del almacén, ids de archivo con forma incorrecta y más de 20 menciones. Trate el resto también como no fiable: compruebe que el usuario participa en la conversación y puede mencionar a esas personas.

`SendKey::Enter`, el valor por defecto, envía con Enter y empieza una línea nueva con Shift+Enter; `.send_key(SendKey::CtrlEnter)` envía con Ctrl+Enter (Cmd+Enter en un Mac) y deja Enter para las líneas nuevas. El botón de enviar está desactivado mientras el mensaje está vacío o es demasiado largo, y cerca del límite aparece un contador. Tras un envío, el composer nombra sus topics (`"chat"` salvo que `.topic(..)` indique otro), de modo que un `MessageThread` de la misma conversación muestra el mensaje enseguida y transmite su respuesta de IA cuando tiene `.reply(..)`.

`.attachments(rules)` añade el campo de subida de un almacén registrado con `Feature::files`: los archivos se suben uno a uno a `POST /_zeyn/files/{store}` con una barra de progreso, bajo el permiso de escritura y las comprobaciones de tipo del almacén, y sus ids van con el mensaje.

`.mentions(action)` busca personas después de `@` con una acción de búsqueda (entra un `LookupQuery`, sale una página de `LookupItem`). La búsqueda empieza 200 ms después de la última tecla, una búsqueda nueva cancela la anterior, y el servidor pide como máximo `MAX_MENTIONS` (8) personas y devuelve como mucho esa cantidad. La lista es un listbox controlado desde el área de texto: las flechas mueven, Enter o Tab eligen, Escape cierra.

`.remember(key)` guarda el texto no enviado de cada conversación en el almacén de preferencias del cliente bajo una clave `Draft` registrada con `App::client_prefs`, hasta que se envía. Un borrador tiene como máximo `MAX_DRAFT_CHARS` (4.000) caracteres, se guardan como máximo `MAX_DRAFTS` (20) conversaciones y la más antigua sale primero. El id de la conversación se guarda solo como hash. Con `Purpose::Preference` los borradores se mantienen durante la sesión hasta que el usuario consiente más.

## TypingIndicator

`TypingIndicator` muestra quién más está escribiendo en una conversación: "Ada está escribiendo", "Ada y Grace están escribiendo", "3 personas están escribiendo", en una región viva educada (polite).

```rust
let live = ZeynSocket::new().topic("chat", "chat.read");

TypingIndicator::new("chat")
    .conversation(params.id.to_string())
    .person("ada", "Ada Lovelace")
    .person("grace", "Grace Hopper")
```

Funciona sobre el canal en tiempo real (feature `realtime`, véase `realtime.md`) en un topic declarado con `ZeynSocket::topic`. Mientras el usuario escribe en un `Composer` de la misma conversación, el navegador envía una pequeña señal por el socket en vivo de la página como mucho cada 3 segundos; el servidor descarta las más rápidas por usuario y pasa la señal a los demás suscriptores del topic en el tenant, nunca de vuelta al mismo usuario. Una señal contiene el id de la conversación y una clave del usuario que solo este servidor puede generar, nunca el id de usuario ni ningún texto. Un nombre se muestra durante 6 segundos después de la última señal; las personas no indicadas con `.person(user, name)` aparecen como "alguien".

Sin la feature `realtime`, o cuando la app no tiene canal en tiempo real, el indicador no renderiza nada, su script nunca se carga y no se envía nada. Las señales llegan a todos los suscriptores del topic en el tenant, así que el id de la conversación es visible para ellos: use ids opacos, o un topic por audiencia.
