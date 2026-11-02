# CLI and projects

The `zeyn` command creates projects, adds features, checks an app and runs an MCP server for agents.

## Commands

| Command | What it does |
|---|---|
| `zeyn new <name>` | Creates an app with the standard feature structure |
| `zeyn new <name> --lib` | Creates a library of models, features and texts shared by several apps |
| `zeyn g feature <name>` | Adds `src/features/<name>` with models, store, actions and screens |
| `zeyn routes` | Lists features, screens, routes, APIs and access |
| `zeyn check` | Compiles and reports structural problems and missing translations |
| `zeyn check --fix` | Rewrites renamed Zeyn APIs in `src`, `tests` and `examples`, then checks |
| `zeyn mcp` | Runs an MCP server on stdio for AI agents |

Install it from crates.io:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

Inside this repository, run the CLI through Cargo:

```sh
cargo run -p zeyn-cli -- new my-app
cd my-app
cargo run
```

## A new app

`zeyn new` writes a project that builds, passes its tests and follows the rules agents are given:

- `AGENTS.md` and `CLAUDE.md`: the project rules and a framework index for agents.
- `.mcp.json`: registers the `zeyn` MCP server, so an agent can call `zeyn_check` and `zeyn_docs`.
- `config/app.json`: defaults. Environment variables `APP_*` override them, such as `APP_SERVER__ADDR` for `server.addr`.
- `locales/en.json` and `locales/tr.json`: every text a user sees.
- `src/features/`: one folder per feature, with a sample `notes` feature.
- `rustfmt.toml`, `clippy.toml`, `deny.toml`, `rust-toolchain.toml` and `[lints]` in `Cargo.toml`: standards that CI enforces.
- `Dockerfile`, `.dockerignore`, `.gitlab-ci.yml`: a container and a pipeline.

See [Project structure](structure.md) for each folder.

## Checking an app

`zeyn check` runs the compiler and then the app's own checks (`App::check`): duplicate routes, screens registered twice, services a screen needs but nobody registered, unusable shortcuts, theme tokens below WCAG AA contrast and translation keys missing in a language. It exits with an error when anything is wrong, so CI can run it.

When a Zeyn release renames an API, the old name stays for one release as a deprecated alias. `zeyn check --fix` rewrites the old names, for example `Grid` to `DataGrid` and `GridQuery` to `DataQuery`, then checks again.

```sh
zeyn check --fix
```

## Docker

The generated `Dockerfile` builds in two stages and runs on a distroless image as a user without root rights. The binary checks its own health with `<app> health`, which the `HEALTHCHECK` uses.

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

The image of a generated app is about 43 MB and an idle app uses a few MB of RAM.

## The MCP server

`zeyn mcp` speaks MCP over stdio. Its tools are `zeyn_docs` (framework reference by topic), `zeyn_describe` (features, screens, routes and problems of the app in the current directory), `zeyn_check` (with `fix: true` it runs `check --fix`), `zeyn_test` and `zeyn_new_feature`. See [MCP and AI agents](mcp.md).
