# CLI y proyectos

El comando `zeyn` crea proyectos, añade features, comprueba una aplicación y ejecuta un servidor MCP para agentes.

## Comandos

| Comando | Qué hace |
|---|---|
| `zeyn new <name>` | Crea una aplicación con la estructura estándar de features |
| `zeyn new <name> --lib` | Crea una biblioteca de modelos, features y textos compartida por varias aplicaciones |
| `zeyn g feature <name>` | Añade `src/features/<name>` con modelos, acciones y pantallas, un archivo por pantalla y por acción |
| `zeyn g screen <feature> <name>` | Añade `screens/<name>.rs` a una feature, lo registra y añade su título a cada archivo de idioma |
| `zeyn g action <feature> <name>` | Añade `actions/<name>.rs` a una feature y lo registra como API |
| `zeyn g component <feature> <name>` | Añade `components/<name>.rs`, una función que devuelve una parte de una pantalla |
| `zeyn routes` | Lista features, pantallas, rutas, APIs y accesos |
| `zeyn check` | Compila e informa de problemas estructurales y traducciones que faltan |
| `zeyn check --fix` | Reescribe las APIs de Zeyn renombradas en `src`, `tests` y `examples`, y luego comprueba |
| `zeyn mcp` | Ejecuta un servidor MCP por stdio para agentes de IA |

Instálalo desde crates.io:

```sh
cargo install zeyn-cli
zeyn new my-app
cd my-app
cargo run
```

Dentro de este repositorio, ejecuta la CLI a través de Cargo:

```sh
cargo run -p zeyn-cli -- new my-app
cd my-app
cargo run
```

## Una aplicación nueva

`zeyn new` escribe un proyecto que compila, pasa sus pruebas y sigue las reglas que se les dan a los agentes:

- `AGENTS.md` y `CLAUDE.md`: las reglas del proyecto y un índice del framework para agentes.
- `.mcp.json`: registra el servidor MCP `zeyn`, para que un agente pueda llamar a `zeyn_check` y `zeyn_docs`.
- `config/app.json`: valores por defecto. Las variables de entorno `APP_*` los sobrescriben, por ejemplo `APP_SERVER__ADDR` para `server.addr`.
- `locales/en.json` y `locales/tr.json`: todos los textos que ve un usuario.
- `src/features/`: una carpeta por feature, con una feature de ejemplo `notes`.
- `rustfmt.toml`, `clippy.toml`, `deny.toml`, `rust-toolchain.toml` y `[lints]` en `Cargo.toml`: estándares que la CI hace cumplir.
  `rustfmt.toml` define `chain_width = 40`, de modo que el código se lee de arriba abajo: una cadena de llamadas más larga
  que eso recibe una llamada por línea, uno o dos puntos por línea de media. Los agentes y las personas revisan una
  lista más rápido que una línea larga, y los diffs muestran la única llamada que cambió. Mantén el ajuste.

  ```rust
  DataGrid::of(search_items)
      .paging(Paging::pages(25))
      .column(Item::code)
      .column(Item::qty)
  ```

  y no `DataGrid::of(search_items).paging(Paging::pages(25)).column(Item::code).column(Item::qty)`.
- `Dockerfile` y `.dockerignore`: una imagen de contenedor pequeña.
- Pipelines para los servidores de CI habituales, todos con las mismas comprobaciones (`cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo deny check`) y que rechazan una etiqueta `vX.Y.Z` que no coincida con `Cargo.toml`:
  - `.github/workflows/ci.yml` para GitHub Actions, con `.github/dependabot.yml` (actualizaciones semanales de Cargo y de actions) y una lista de verificación para pull requests;
  - `.gitlab-ci.yml` para GitLab;
  - `Jenkinsfile` para Jenkins (agente Docker, cobertura con `cargo llvm-cov`).
- `sonar-project.properties`: análisis con SonarQube (incidencias de Clippy y cobertura LCOV desde `target/lcov.info`). El workflow de GitHub lo ejecuta cuando están definidas la variable de repositorio `SONAR_HOST_URL` y el secreto `SONAR_TOKEN`; Jenkins lo ejecuta cuando el job define `SONAR_ENABLED=true`.
- `SECURITY.md` y `CHANGELOG.md` (Keep a Changelog).
- Un repositorio git local en la rama `main`, cuando git está instalado (Windows, macOS o Linux) y la carpeta no está ya dentro de un repositorio. No se confirma nada, así que el primer commit lleva tu propio nombre y correo. `--no-git` lo omite.

Consulta [Estructura del proyecto](structure.md) para ver cada carpeta.

## Generadores

Un archivo por pantalla, acción y componente es la estructura por defecto que crean los generadores:

```sh
zeyn g screen notes detail       # src/features/notes/screens/detail.rs
zeyn g action notes archive      # src/features/notes/actions/archive.rs
zeyn g component notes summary   # src/features/notes/components/summary.rs
```

Las formas cortas `zeyn g s`, `zeyn g a`, `zeyn g c` y `zeyn g f` hacen lo mismo. Una pantalla recibe el permiso `<feature>.<name>.view` y su título en cada archivo de `locales/`; los idiomas distintos del inglés reciben el texto en inglés y una nota para traducirlo. Una acción recibe `<feature>.<name>.run` y se registra con `.api(...)`. La estructura no es obligatoria: si una feature todavía tiene un único `screens.rs` o `actions.rs`, el generador primero lo mueve con su contenido a `screens/mod.rs` (o `actions/mod.rs`) y después añade el archivo nuevo. Los generadores nunca sobrescriben un archivo y rechazan los nombres que no están en snake case.

## Comprobar una aplicación

`zeyn check` ejecuta el compilador y después las comprobaciones propias de la aplicación (`App::check`): rutas duplicadas, pantallas registradas dos veces, servicios que una pantalla necesita pero nadie registró, atajos inutilizables, tokens de tema por debajo del contraste WCAG AA y claves de traducción que faltan en un idioma. Termina con error cuando algo está mal, así que la CI puede ejecutarlo.

También muestra avisos, que nunca hacen fallar la comprobación: un `#[screen]` o `#[action]` que ninguna feature registra, y un único `screens.rs` o `actions.rs` con varios elementos en más de unas 150 líneas.

Cuando una versión de Zeyn renombra una API, `zeyn check --fix` reescribe los nombres antiguos, por ejemplo `Grid` por `DataGrid` y `GridQuery` por `DataQuery`, y vuelve a comprobar.

```sh
zeyn check --fix
```

## Docker

El `Dockerfile` generado compila en dos etapas y se ejecuta sobre una imagen distroless con un usuario sin privilegios de root. El binario comprueba su propia salud con `<app> health`, que usa el `HEALTHCHECK`.

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

La imagen de una aplicación generada pesa unos 43 MB y una aplicación inactiva usa unos pocos MB de RAM.

## El servidor MCP

`zeyn mcp` habla MCP por stdio. Sus herramientas son `zeyn_docs` (referencia del framework por tema), `zeyn_describe` (features, pantallas, rutas y problemas de la aplicación en el directorio actual), `zeyn_check` (con `fix: true` ejecuta `check --fix`), `zeyn_test` y `zeyn_new_feature`. Consulta [MCP y agentes de IA](mcp.md).
