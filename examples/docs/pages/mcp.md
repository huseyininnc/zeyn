# MCP and AI agents

Zeyn is built for agents on two sides: agents that write Zeyn apps, and agents that use a running app on behalf of a user.

## Agents that write apps

The project an agent works in carries its own rules and tools:

- `AGENTS.md` and `CLAUDE.md` list the commands, the project structure, the rules of the language and a framework index.
- `.mcp.json` registers `zeyn mcp`, an MCP server on stdio with these tools:

| Tool | Does |
|---|---|
| `zeyn_docs` | Framework reference by topic: `rules`, `language`, `protocol`; `search: "words"` ranks the docs pages of this version, `page: "/language/forms"` returns one as Markdown |
| `zeyn_describe` | Features, screens, routes, access, APIs, problems and missing translations of the app |
| `zeyn_inspect` | The node tree of one screen: path, component type and the Rust `file:line:column` that placed each node |
| `zeyn_check` | Compiles and reports problems; `fix: true` first rewrites renamed APIs |
| `zeyn_test` | Runs `cargo test` and returns the end of the output |
| `zeyn_new_feature` | Creates a feature folder and registers it |
| `zeyn_new_screen` | Adds a screen file to a feature and registers it |

The compiler is the main guard. Every rule it can enforce has a `compile_fail` test and a `#[diagnostic::on_unimplemented]` message written for an agent to act on, such as:

```sh
error[E0277]: `min()` and `max()` cannot be used on a `bool` column
  = note: `min()` and `max()` work on i32, i64, Money, Date, Time, DateTime, LocalDateTime and their Option forms
```

`ZEYN_DESCRIBE=1 cargo run` prints the same description `zeyn_describe` returns and exits.

### Source inspector

Debug builds remember the line of Rust that placed each node on a screen (`Screen::body`, `Stack::add`, `Tabs::tab` and the other container methods). `ZEYN_INSPECT=<screen> cargo run` prints the node tree `zeyn_inspect` returns and exits.

A debug build served with `cargo run` writes the line into the page as `data-zeyn-src` on each node's first element. Alt+Shift+I opens an overlay that outlines the node under the pointer or the focused element and names its line, followed by the lines of the nodes that hold it; Alt+Shift+I or Escape closes it. Release builds keep no lines: a node stays one boxed pointer, the page carries no attribute and the inspector script is not in the binary.

## Agents that use an app

```rust
App::new(t!("app.name")).mcp(Mcp::read_only())
```

With `agents`, the app's screens become tools:

- one search tool for each grid,
- one save tool for each form, with the form's fields and rules as its schema (only with `Mcp::read_write()`).

Agents reach them in two ways:

- **In the browser** through WebMCP (`document.modelContext.registerTool`), when the browser supports it. The tools of the current screen are registered as it opens and removed when it closes.
- **On the server** at `POST /_zeyn/mcp` (JSON-RPC). It needs an `Authorization` header, so a browser session alone can never drive it.

## The same rules as people

- An agent gets exactly the permissions of the user it acts for. Tools for screens and actions the user may not open are not listed.
- Calls run the guards, interceptors, validation and rate limits of a normal call.
- Audit records mark calls made by agents, so a reviewer can tell them apart.
- `Mcp::read_only()` lets agents search but not save. Choose `read_write()` only for apps where an agent should change data.
