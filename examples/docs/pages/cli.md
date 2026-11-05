# CLI and projects

The `zeyn` command creates projects, adds features, checks an app and runs an MCP server for agents.

## Commands

| Command | What it does |
|---|---|
| `zeyn new <name>` | Creates an app with the standard feature structure |
| `zeyn new <name> --lib` | Creates a library of models, features and texts shared by several apps |
| `zeyn g feature <name>` | Adds `src/features/<name>` with models, actions and screens, one file per screen and action |
| `zeyn g screen <feature> <name>` | Adds `screens/<name>.rs` to a feature, registers it and adds its title to every locale file |
| `zeyn g action <feature> <name>` | Adds `actions/<name>.rs` to a feature and registers it as an API |
| `zeyn g component <feature> <name>` | Adds `components/<name>.rs`, a function that returns a part of a screen |
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

One file per screen, action and component is the default layout the generators produce:

```sh
zeyn g screen notes detail       # src/features/notes/screens/detail.rs
zeyn g action notes archive      # src/features/notes/actions/archive.rs
zeyn g component notes summary   # src/features/notes/components/summary.rs
```

The short forms `zeyn g s`, `zeyn g a`, `zeyn g c` and `zeyn g f` do the same. A screen gets the permission `<feature>.<name>.view` and its title in every file in `locales/`; languages other than English get the English text and a note to translate it. An action gets `<feature>.<name>.run` and is registered with `.api(...)`. The layout is not enforced: when a feature still has a flat `screens.rs` or `actions.rs`, the generator first moves it to `screens/mod.rs` (or `actions/mod.rs`) with its content, then adds the new file. Generators never overwrite a file and refuse names that are not snake case.

## Checking an app

`zeyn check` runs the compiler and then the app's own checks (`App::check`): duplicate routes, screens registered twice, services a screen needs but nobody registered, unusable shortcuts, theme tokens below WCAG AA contrast and translation keys missing in a language. It exits with an error when anything is wrong, so CI can run it.

It also prints warnings, which never fail the check: a `#[screen]` or `#[action]` that no feature registers, and a flat `screens.rs` or `actions.rs` with several items in more than about 150 lines.

When a Zeyn release renames an API, `zeyn check --fix` rewrites the old names, for example `Grid` to `DataGrid` and `GridQuery` to `DataQuery`, then checks again.

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
