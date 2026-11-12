# CLI and projects

The `zeyn` command creates projects, adds features, checks an app and runs an MCP server for agents.

## Commands

| Command | What it does |
|---|---|
| `zeyn new <name>` | Creates an app with the standard feature structure |
| `zeyn new <name> --lib` | Creates a library of models, features and texts shared by several apps |
| `zeyn g feature <name>` | Adds `src/features/<name>` with `models`, `services`, `actions` and `screens` and a working list screen |
| `zeyn g screen <feature> <name>` | Adds `screens/<name>.rs` to a feature, registers it and adds its title to every locale file |
| `zeyn routes` | Lists features, screens, routes, APIs and access |
| `zeyn check` | Compiles and reports structural problems and missing translations |
| `zeyn check --fix` | Rewrites renamed Zeyn APIs in `src`, `tests` and `examples`, then checks |
| `zeyn check --json` | Prints the report as JSON with stable codes, reasons and fixes |
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
  `rustfmt.toml` sets `chain_width = 40`, so code reads top to bottom: a chain of calls longer than
  that gets one call per line, one or two dots per line on average. Agents and people review a
  list faster than one long line, and diffs show the one call that changed. Keep the setting.

  ```rust
  DataGrid::of(search_items)
      .paging(Paging::pages(25))
      .column(Item::code)
      .column(Item::qty)
  ```

  not `DataGrid::of(search_items).paging(Paging::pages(25)).column(Item::code).column(Item::qty)`.
- `build.rs` and `max_file_lines = 300` under `[package.metadata.zeyn]` in `Cargo.toml`: `cargo build` stops when a file in `src/` has more non blank lines; clippy `too_many_lines` (60, in `clippy.toml`) does the same for functions. See [Project structure](structure.md).
- `Dockerfile` and `.dockerignore`: a small container image.
- Pipelines for the common CI servers, all running the same checks (`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check`) and refusing a tag `vX.Y.Z` that does not match `Cargo.toml`:
  - `.github/workflows/ci.yml` for GitHub Actions, with `.github/dependabot.yml` (weekly Cargo and action updates) and a pull request checklist;
  - `.gitlab-ci.yml` for GitLab;
  - `Jenkinsfile` for Jenkins (Docker agent, coverage with `cargo llvm-cov`).
- `sonar-project.properties`: SonarQube analysis (Clippy issues and LCOV coverage from `target/lcov.info`). The GitHub workflow runs it when the repository variable `SONAR_HOST_URL` and the secret `SONAR_TOKEN` are set; Jenkins runs it when the job sets `SONAR_ENABLED=true`.
- `SECURITY.md` and `CHANGELOG.md` (Keep a Changelog).
- A local git repository on branch `main`, when git is installed (Windows, macOS or Linux) and the folder is not already inside a repository. Nothing is committed, so the first commit carries your own name and email. `--no-git` skips it.

See [Project structure](structure.md) for each folder.

## Generators

`zeyn g feature` creates a feature with one file per model, service, action and screen:

```sh
zeyn g feature orders            # src/features/orders/: models/, services/, actions/, screens/
zeyn g screen orders detail      # src/features/orders/screens/detail.rs
```

The short forms `zeyn g f` and `zeyn g s` do the same. The new feature works at once: a model, an in memory data service, a search action and a list screen with its menu entry. A feature that does not need a folder deletes it (the folder and its `mod` line in the feature's `mod.rs`). `components/` is never generated: add it by hand, one function that returns a `Node` per file (`components/<name>.rs`), when a part of a screen is shared or a screen file grows. A screen gets the permission `<feature>.<name>.view` and its title in every file in `locales/`; languages other than English get the English text and a note to translate it. The layout is not enforced: when a feature has a flat `screens.rs`, `zeyn g screen` first moves it to `screens/mod.rs` with its content, and when `screens/` was deleted it creates it again. Generators never overwrite a file and refuse names that are not snake case.

## Checking an app

`zeyn check` runs the compiler and then the app's own checks (`App::check`): duplicate routes, screens registered twice, services a screen needs but nobody registered, unusable shortcuts, theme tokens below WCAG AA contrast and translation keys missing in a language. It exits with an error when anything is wrong, so CI can run it.

It also prints warnings, which never fail the check: a `#[screen]` or `#[action]` that no feature registers, and a flat `screens.rs` or `actions.rs` with several items in more than about 150 lines.

When a Zeyn release renames an API, `zeyn check --fix` rewrites the old names, for example `Grid` to `DataGrid` and `GridQuery` to `DataQuery`, then checks again.

```sh
zeyn check --fix
```

Every problem has a stable code: `Z01xx` for the app, features, routes and services, `Z02xx` for security, `Z03xx` for public pages, `Z04xx` for menus, commands and shortcuts, `Z05xx` for data, `Z06xx` for page checks and `W01xx` for warnings. The text output shows each problem with its reason (`why`), its fix (`help`), a link to these docs and its code. `zeyn check --json` prints the same report as one JSON document for agents and CI, and exits with an error when `ok` is false.

```sh
zeyn check --json
```

```json
{
  "ok": false,
  "errors": [
    {
      "code": "Z0103",
      "summary": "screen `order_list` needs service `OrderStore`, which is not registered",
      "why": "the screen takes the service as an argument, so every request to it would fail",
      "fix": "add `.service(...)` to the App",
      "docs": "https://docs.zeynsoft.com/start/structure#services"
    }
  ],
  "warnings": [
    { "code": "W0105", "message": "translation `orders.title` is missing in `tr`" }
  ]
}
```

## Docker

The generated `Dockerfile` builds in two stages and runs on a distroless image as a user without root rights. The binary checks its own health with `<app> health`, which the `HEALTHCHECK` uses.

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

The image of a generated app is about 43 MB and an idle app uses a few MB of RAM.

## The MCP server

`zeyn mcp` speaks MCP over stdio. Its tools are `zeyn_docs` (framework reference by topic), `zeyn_describe` (features, screens, routes and problems of the app in the current directory), `zeyn_check` (with `fix: true` it runs `check --fix`), `zeyn_test`, `zeyn_new_feature` and `zeyn_new_screen`. See [MCP and AI agents](mcp.md).
