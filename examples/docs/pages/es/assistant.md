# Asistente integrado

Estado: implementado tras la feature de Cargo `assistant` de `zeyn`.

Una app de Zeyn puede llevar su propio asistente de IA: un chat en cada página que responde con los datos de la app
y trabaja con las pantallas y las API de la app como el usuario con sesión iniciada. Las apps construidas con Zeyn pueden activarlo
para sus propios clientes, conectar el modelo que esos clientes quieran y guardar el historial del chat donde
lo necesiten.

```toml
zeyn = { version = "0.0.9", features = ["assistant"] }
```

Sin la feature no se compila nada de esto: ni cliente HTTP, ni rutas adicionales, ni memoria adicional.

## 1. Configuración

```rust
use zeyn::assistant::{Assistant, OpenAiCompatible};

let model = OpenAiCompatible::new(config.require("assistant.base_url")?, config.require("assistant.model")?)
    .api_key(config.secret("assistant.api_key")?);

App::new("Lifts")
    .assistant(
        Assistant::new(model)
            .permission("lifts.assistant.use")
            .instructions("You help service engineers plan elevator maintenance.")
            .suggest(work_orders, "Which lifts are overdue for service?")
            .suggest_anywhere("What can you do?"),
    )
```

Eso es todo. Cada página recibe un lanzador flotante que abre el chat como un panel lateral. Una pantalla puede
mostrar también el propio chat:

```rust
#[screen(route = "/assistant", permission = "lifts.assistant.use", ai = false)]
pub fn assistant_page() -> Screen {
    Screen::new("Assistant").body(AssistantChat::page())
}
```

`AssistantChat::panel()` encaja junto a otro contenido. `Assistant::launcher(false)` elimina el
lanzador flotante, de modo que el chat aparece solo donde una pantalla lo coloca.

| Opción | Valor predeterminado |
|---|---|
| `permission("...")` | todo usuario con sesión iniciada |
| `instructions("...")` | ninguno; las escriben los desarrolladores de la app, nunca los usuarios |
| `suggest(screen, prompt)`, `suggest_anywhere(prompt)` | sin chips de inicio |
| `speech_to_text(p)`, `text_to_speech(p)` | la Web Speech API del navegador |
| `no_voice()` | micrófono y lectura en voz alta activados |
| `store(s)` | `MemoryChatStore::default()` |
| `read_only()` | el asistente también puede guardar, tras confirmación |
| `rate_limit(RateLimit)` | 20 solicitudes por minuto por usuario |
| `token_limit(Some(RateLimit))` | 200.000 tokens por hora por usuario |
| `limits(Limits { .. })` | consulta la sección 6 |

## 2. Modelos

Funciona cualquier tipo que implemente `LlmProvider`: un método que inicia una ronda en streaming y devuelve un
canal acotado de `ChatEvent` (fragmentos de texto, llamadas a herramientas, uso). Incorporados:

| Proveedor | Cubre |
|---|---|
| `OpenAiCompatible::new(base_url, model)` | OpenAI (`https://api.openai.com/v1`), API v1 de Azure OpenAI (`https://NAME.openai.azure.com/openai/v1`, el deployment como modelo, `.azure()` para la cabecera `api-key`), Ollama (`http://localhost:11434/v1`), vLLM, LiteLLM y otros servidores compatibles. `.legacy_max_tokens()` para servidores que no conocen `max_completion_tokens` |
| `Anthropic::new(model)` | La Messages API de Anthropic (`.base_url(...)` para un gateway). Los bloques de razonamiento se reenvían sin cambios dentro de un turno |
| `ScriptedProvider` | Pruebas, demos y desarrollo sin conexión: reglas en lugar de una llamada de red, transmitido palabra por palabra, con llamadas reales a herramientas |

Las claves provienen de valores `Secret` (`Config::secret`), permanecen en el servidor y nunca aparecen en los mensajes
de error. Los errores del proveedor se registran en el servidor; el usuario ve un mensaje genérico.

Apuntar el showcase a un modelo real solo requiere configuración:

```
NORTHWIND_ASSISTANT__PROVIDER=openai
NORTHWIND_ASSISTANT__BASE_URL=https://api.openai.com/v1
NORTHWIND_ASSISTANT__MODEL=gpt-4.1-mini
NORTHWIND_ASSISTANT__API_KEY=...
```

o `NORTHWIND_ASSISTANT__PROVIDER=anthropic` con `NORTHWIND_ASSISTANT__MODEL=claude-opus-5-5`, o
una URL de Ollama sin clave. `NORTHWIND_ASSISTANT__PROVIDER=off` lo desactiva.

## 3. Voz

De forma predeterminada el micrófono usa el reconocimiento de voz del navegador y "leer en voz alta" la síntesis
de voz del navegador: sin coste de servidor, y ningún audio sale del navegador hacia la app. Para voz en el servidor:

```rust
Assistant::new(model)
    .speech_to_text(OpenAiTranscription::new(base, "whisper-1").api_key(key.clone()))
    .text_to_speech(OpenAiSpeech::new(base, "tts-1", "alloy").api_key(key))
```

Ambos hablan los endpoints `/audio/transcriptions` y `/audio/speech` compatibles con OpenAI, que los servidores locales de
Whisper y de voz también ofrecen. Las grabaciones tienen un límite (4 MB, 60 segundos en el navegador);
el audio de voz se transmite al navegador a medida que llega.

## 4. Historial

`ChatStore` conserva las conversaciones con clave por tenant y usuario (`ChatKey`), de modo que los tenants nunca comparten una:

```rust
pub trait ChatStore: Send + Sync + 'static {
    fn load(&self, key: &ChatKey, limit: usize) -> BoxFuture<Result<Vec<ChatMessage>>>;
    fn append(&self, key: &ChatKey, messages: Vec<ChatMessage>) -> BoxFuture<Result<()>>;
    fn clear(&self, key: &ChatKey) -> BoxFuture<Result<()>>;
}
```

`MemoryChatStore::new(max_users, max_messages)` es el valor predeterminado (1.000 conversaciones de 60
mensajes, saliendo primero las usadas hace más tiempo, y se pierden al reiniciar). Para una base de datos, implementa los tres
métodos sobre una tabla con tenant, usuario, posición y el mensaje como JSON. Los mensajes son solo de
añadir; el store decide la retención. Las confirmaciones pendientes también viven en el historial, de modo que el servidor
no guarda ningún otro estado por usuario.

## 5. Herramientas y actions

El modelo ve las mismas herramientas que MCP y WebMCP (consulta `docs/runtime-protocol.md` sección 9), construidas
cada vez para el usuario con sesión iniciada: búsquedas de grid, guardados de formularios y actions de API que el usuario puede usar, menos
todo lo marcado con `ai = false`, con descripciones de los comentarios de documentación `///` y la guía `ai = "..."`.
Se añaden dos herramientas incorporadas:

- `navigate`: abre una pantalla con ruta que el usuario puede abrir, sin carga de página (`zeyn:navigate`), y
  puede filtrar su grid (`zeyn:filter`). El grid se carga entonces mediante `POST /_zeyn/grid`, donde el
  servidor aplica sus comprobaciones habituales.
- `suggest`: indicaciones de seguimiento mostradas como chips.

Cada llamada pasa por la vía normal de actions con el contexto marcado como agente: el acceso de la pantalla,
los guards de la feature, el permiso de la action, los interceptores, los límites de tasa, el límite de tiempo,
la auditoría (cada llamada del asistente se registra con `via_agent: true`) y el enmascarado.

**Confirmación.** Las lecturas se ejecutan de inmediato. Todo lo que escribe (guardados de formularios, actions de API cuya entrada
no es un `DataQuery`) y cada action marcada con `#[action(confirm)]` detiene la ronda y muestra una
tarjeta de confirmación con lo que se ejecutará y sus argumentos. Solo el clic del usuario
(`POST /_zeyn/assistant/confirm`, con CSRF, para una llamada pendiente en el historial propio de ese usuario) la ejecuta,
una sola vez. Un mensaje como "Confirmo" es solo texto: un mensaje nuevo cierra la llamada pendiente como no ejecutada.

## 6. Modelo de seguridad

| Riesgo | Qué hace el framework |
|---|---|
| Claves de API filtradas | Las claves son `Secret`s en el servidor; el navegador habla solo con la app |
| Un modelo que actúa más allá del usuario | Las herramientas se construyen a partir de los permisos del usuario; cada llamada se vuelve a comprobar en el servidor |
| Escrituras que el usuario no quería | Las escrituras y las actions `confirm` necesitan el clic del usuario; el modelo no puede confirmar |
| Datos enmascarados que llegan al modelo | Los campos enmascarados siempre están enmascarados para las llamadas de agente, incluso para los usuarios que pueden revelarlos |
| Prompt injection a través de los datos | Los resultados de las herramientas son texto simple dentro de delimitadores `<tool_result>` con `<` y `>` escapados; el prompt del sistema dice que son datos. El servidor reconstruye el prompt del sistema en cada ronda a partir de reglas fijas, las instrucciones de la app y el nombre de la pantalla, nunca a partir de datos del usuario, de la página o de herramientas |
| Exfiltración a través de enlaces | Las respuestas se renderizan como un pequeño subconjunto de Markdown construido con `textContent`: sin HTML, sin enlaces, sin imágenes |
| Coste descontrolado | Límite de tasa de solicitudes por usuario, límite de tokens por usuario, tokens de salida acotados, como máximo 6 rondas y 4 llamadas a herramientas por ronda |
| Entradas grandes | Mensajes de hasta 2.000 caracteres, 30 mensajes de historial por ronda, resultados de herramientas recortados a 6.000 caracteres, grabaciones de hasta 4 MB |
| Solicitudes entre sitios | Cada `POST` necesita la cabecera CSRF y cuenta para el límite de escritura de la app |
| Actions sin responsable | Cada llamada a una herramienta del asistente se audita con `via_agent` |

Lo que no hace: decidir si la respuesta de un modelo es correcta. Díselo a los usuarios (el panel lo hace) y
mantén las actions importantes tras `confirm`.

## 7. Integración para clientes

Una app que vende el asistente a sus propios clientes suele querer:

- `permission("app.assistant.use")` y una comprobación del módulo de licencia en un guard, de modo que solo los tenants que pagan
  lo vean;
- el modelo por tenant: implementar `LlmProvider` como un pequeño enrutador que elige el proveedor del cliente
  a partir de la configuración según el tenant de `ctx`, o ejecutar un único modelo para todos;
- un `ChatStore` en la base de datos de la app, con las reglas de retención del cliente;
- `instructions(...)` que describan el dominio (una empresa de ascensores: unidades, sedes, intervalos
  de servicio), y guía `ai = "..."` en las actions que la necesiten;
- `token_limit` ajustado al plan que paga el cliente.

## 8. Endpoints

| Endpoint | Hace |
|---|---|
| `GET /_zeyn/assistant/start?screen=` | `{ messages, pending, suggestions, stt, tts, max }` |
| `POST /_zeyn/assistant/chat` `{ message, screen }` | Transmite eventos `application/x-ndjson` |
| `POST /_zeyn/assistant/confirm` `{ id, approve }` | Ejecuta o rechaza la llamada pendiente y luego transmite como el chat |
| `POST /_zeyn/assistant/clear` | Elimina la conversación; `204` |
| `POST /_zeyn/assistant/stt` (cuerpo de audio) | `{ text }`; `404` sin voz a texto en el servidor |
| `POST /_zeyn/assistant/tts` `{ text }` | Audio en streaming; `404` sin texto a voz en el servidor |

Eventos, un objeto JSON por línea: `{t:"text",v}`, `{t:"tool",name,id}`, `{t:"navigate",url,screen,filters}`,
`{t:"suggest",v}`, `{t:"confirm",id,tool,title,question,details}`, `{t:"resolved",id,approved}`,
`{t:"error",message}`, `{t:"done"}`.

`assistant.js` y `assistant.css` se cargan solo para los usuarios que pueden usar el asistente. El registro del chat es una
región activa `role="log"` (`aria-busy` mientras se transmite una respuesta), Enter envía, Shift+Enter inicia una
nueva línea, Escape cierra el panel lateral y devuelve el foco al lanzador, y la tarjeta de confirmación
pone el foco primero en Cancelar.

## 9. Coste

El asistente no guarda en memoria ningún estado por usuario salvo el store de historial acotado y los streams
en curso. Las respuestas se transmiten de extremo a extremo por canales de 32 elementos. Medido en el showcase (compilación debug,
Windows, en reposo tras cinco cargas de página): 14.4 MB de working set y 3.13 MB de memoria privada con el
asistente desactivado, 14.5 MB y 3.19 MB con él activado (unos 100 KB más).
