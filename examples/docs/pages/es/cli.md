# CLI y proyectos

El comando `zeyn` crea proyectos, añade features, comprueba una aplicación y ejecuta un servidor MCP para agentes.

## Comandos

| Comando | Qué hace |
|---|---|
| `zeyn new <name>` | Crea una aplicación con la estructura estándar de features |
| `zeyn new <name> --lib` | Crea una biblioteca de modelos, features y textos compartida por varias aplicaciones |
| `zeyn g feature <name>` | Añade `src/features/<name>` con `models`, `services`, `actions` y `screens` y una pantalla de lista que funciona |
| `zeyn g screen <feature> <name>` | Añade `screens/<name>.rs` a una feature, lo registra y añade su título a cada archivo de idioma |
| `zeyn routes` | Lista features, pantallas, rutas, APIs y accesos |
| `zeyn check` | Compila e informa de problemas estructurales y traducciones que faltan |
| `zeyn check --fix` | Reescribe las APIs de Zeyn renombradas en `src`, `tests` y `examples`, y luego comprueba |
| `zeyn check --json` | Imprime el informe como JSON con códigos estables, motivos y soluciones |
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
- `build.rs` y `max_file_lines = 300` en `[package.metadata.zeyn]` de `Cargo.toml`: `cargo build` se detiene cuando un archivo de `src/` tiene más líneas no vacías; clippy `too_many_lines` (60, en `clippy.toml`) hace lo mismo con las funciones. Consulta [Project structure](structure.md).
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

`zeyn g feature` crea una feature con un archivo por modelo, servicio, acción y pantalla:

```sh
zeyn g feature orders            # src/features/orders/: models/, services/, actions/, screens/
zeyn g screen orders detail      # src/features/orders/screens/detail.rs
```

Las formas cortas `zeyn g f` y `zeyn g s` hacen lo mismo. La feature nueva funciona desde el principio: un modelo, un servicio de datos en memoria, una acción de búsqueda y una pantalla de lista con su entrada de menú. Una feature que no necesita una carpeta la borra (la carpeta y su línea `mod` en el `mod.rs` de la feature). `components/` nunca se genera: añádela a mano, una función que devuelve un `Node` por archivo (`components/<name>.rs`), cuando una parte de una pantalla se comparte o un archivo de pantalla crece. Una pantalla recibe el permiso `<feature>.<name>.view` y su título en cada archivo de `locales/`; los idiomas distintos del inglés reciben el texto en inglés y una nota para traducirlo. La estructura no es obligatoria: si una feature tiene un único `screens.rs`, `zeyn g screen` primero lo mueve con su contenido a `screens/mod.rs`, y si `screens/` se borró, la vuelve a crear. Los generadores nunca sobrescriben un archivo y rechazan los nombres que no están en snake case.

## Comprobar una aplicación

`zeyn check` ejecuta el compilador y después las comprobaciones propias de la aplicación (`App::check`): rutas duplicadas, pantallas registradas dos veces, servicios que una pantalla necesita pero nadie registró, atajos inutilizables, tokens de tema por debajo del contraste WCAG AA y claves de traducción que faltan en un idioma. Termina con error cuando algo está mal, así que la CI puede ejecutarlo.

También muestra avisos, que nunca hacen fallar la comprobación: un `#[screen]` o `#[action]` que ninguna feature registra, y un único `screens.rs` o `actions.rs` con varios elementos en más de unas 150 líneas.

Cuando una versión de Zeyn renombra una API, `zeyn check --fix` reescribe los nombres antiguos, por ejemplo `Grid` por `DataGrid` y `GridQuery` por `DataQuery`, y vuelve a comprobar.

```sh
zeyn check --fix
```

Cada problema tiene un código estable: `Z01xx` para la app, las features, las rutas y los servicios, `Z02xx` para seguridad, `Z03xx` para páginas públicas, `Z04xx` para menús, comandos y atajos, `Z05xx` para datos, `Z06xx` para las comprobaciones de página y `W01xx` para avisos. La salida de texto muestra cada problema con su motivo (`why`), su solución (`help`), un enlace a esta documentación y su código. `zeyn check --json` imprime el mismo informe como un único documento JSON para agentes y CI, y termina con error cuando `ok` es false.

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

`zeyn check` también lee el código de la app y avisa, con archivo y línea, de `W0106` una cadena que contiene un `<script>` sin `src` o un manejador de eventos en línea como `onclick=` (la CSP bloquea ambos), `W0107` un componente escrito a mano que escribe HTML sin `escape(..)`, `r.text(..)` ni `r.value(..)`, `W0108` una action que devuelve un `Vec` entero sin una consulta que lo limite y `W0109` un `DocumentViewer` marcado `.trusted()`. Un `permission` o `public` ausente no necesita aviso: no compila.

## DevTools

Una compilación de depuración servida con `cargo run` (o `zeyn dev`) registra lo que hizo cada solicitud: la pantalla y la acción que ejecutó, su estado y su tiempo, cuánto tardó la pantalla en construirse y en renderizarse, cada sentencia SQL con su tiempo y las filas afectadas, y los efectos y temas que devolvió la acción. Alt+Shift+D abre un panel con las últimas 50 solicitudes; cada una se abre para mostrar sus sentencias. `GET /_zeyn/devtools` devuelve la misma lista en JSON.

Las sentencias se guardan sin sus argumentos, así que ningún valor llega al panel. Las compilaciones de release no tienen DevTools: ni código, ni ruta, ni memoria para ellas. Una aplicación que sirve `App::router` por su cuenta, o una prueba, las activa con `zeyn::server::devtools(true)`.

## Docker

El `Dockerfile` generado compila en dos etapas y se ejecuta sobre una imagen distroless con un usuario sin privilegios de root. El binario comprueba su propia salud con `<app> health`, que usa el `HEALTHCHECK`.

```sh
docker build -t my-app .
docker run -p 3000:3000 -e APP_SERVER__ADDR=0.0.0.0:3000 my-app
```

La imagen de una aplicación generada pesa unos 43 MB y una aplicación inactiva usa unos pocos MB de RAM.

## El servidor MCP

`zeyn mcp` habla MCP por stdio. Sus herramientas son `zeyn_docs` (referencia del framework por tema), `zeyn_describe` (features, pantallas, rutas y problemas de la aplicación en el directorio actual), `zeyn_check` (con `fix: true` ejecuta `check --fix`), `zeyn_test`, `zeyn_new_feature` y `zeyn_new_screen`. Consulta [MCP y agentes de IA](mcp.md).
