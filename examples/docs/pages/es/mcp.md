# MCP y agentes de IA

Zeyn está pensado para agentes en dos frentes: agentes que escriben aplicaciones Zeyn y agentes que usan una aplicación en ejecución en nombre de un usuario.

## Agentes que escriben aplicaciones

El proyecto en el que trabaja un agente incluye sus propias reglas y herramientas:

- `AGENTS.md` y `CLAUDE.md` listan los comandos, la estructura del proyecto, las reglas del lenguaje y un índice del framework.
- `.mcp.json` registra `zeyn mcp`, un servidor MCP por stdio con estas herramientas:

| Herramienta | Qué hace |
|---|---|
| `zeyn_docs` | Referencia del framework por tema: `rules`, `language`, `protocol`; `search: "palabras"` ordena las páginas de la documentación de esta versión, `page: "/language/forms"` devuelve una en Markdown |
| `zeyn_describe` | Features, pantallas, rutas, accesos, APIs, problemas y traducciones que faltan de la aplicación |
| `zeyn_check` | Compila e informa de problemas; `fix: true` reescribe primero las APIs renombradas |
| `zeyn_test` | Ejecuta `cargo test` y devuelve el final de la salida |
| `zeyn_new_feature` | Crea una carpeta de feature y la registra |
| `zeyn_new_screen` | Añade un archivo de pantalla a una feature y lo registra |

El compilador es la defensa principal. Cada regla que puede hacer cumplir tiene una prueba `compile_fail` y un mensaje `#[diagnostic::on_unimplemented]` escrito para que un agente actúe en consecuencia, como este:

```sh
error[E0277]: `min()` and `max()` cannot be used on a `bool` column
  = note: `min()` and `max()` work on i32, i64, Money, Date, Time, DateTime, LocalDateTime and their Option forms
```

`ZEYN_DESCRIBE=1 cargo run` imprime la misma descripción que devuelve `zeyn_describe` y termina.

## Agentes que usan una aplicación

```rust
App::new(t!("app.name")).mcp(Mcp::read_only())
```

Con `agents`, las pantallas de la aplicación se convierten en herramientas:

- una herramienta de búsqueda por cada tabla,
- una herramienta de guardado por cada formulario, con los campos y las reglas del formulario como esquema (solo con `Mcp::read_write()`).

Los agentes llegan a ellas de dos maneras:

- **En el navegador** mediante WebMCP (`document.modelContext.registerTool`), cuando el navegador lo admite. Las herramientas de la pantalla actual se registran al abrirla y se quitan al cerrarla.
- **En el servidor** en `POST /_zeyn/mcp` (JSON-RPC). Necesita una cabecera `Authorization`, así que una sesión de navegador por sí sola nunca puede controlarlo.

## Las mismas reglas que las personas

- Un agente recibe exactamente los permisos del usuario en cuyo nombre actúa. Las herramientas de pantallas y acciones que el usuario no puede abrir no se listan.
- Las llamadas ejecutan los guards, los interceptores, la validación y los límites de frecuencia de una llamada normal.
- Los registros de auditoría marcan las llamadas hechas por agentes, de modo que un revisor pueda distinguirlas.
- `Mcp::read_only()` permite a los agentes buscar pero no guardar. Elige `read_write()` solo para aplicaciones en las que un agente deba cambiar datos.
