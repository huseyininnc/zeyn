# Apps de escritorio

Una app de Zeyn puede distribuirse como un único ejecutable de escritorio. El crate `zeyn-desktop` inicia el propio
servidor de la app dentro del proceso y lo muestra en una ventana nativa mediante el WebView del sistema:

| Plataforma | WebView | Estado |
|----------|---------|--------|
| Windows 10 y 11 | WebView2 (Edge) | compilado y comprobado a mano |
| macOS | WKWebView | compila mediante `wry`, no comprobado a mano |
| Linux | WebKitGTK 4.1 | compila mediante `wry` cuando `libwebkit2gtk-4.1-dev` y `libgtk-3-dev` están instalados, no comprobado a mano |

Nada sale de la máquina. La app funciona sin conexión siempre que ella misma no llame a servicios remotos.

## Uso

Añade el crate tras una feature, para que la compilación normal del servidor no compile un WebView:

```toml
[features]
desktop = ["dep:zeyn-desktop"]

[dependencies]
zeyn-desktop = { version = "0.0.9", optional = true }
```

Inicia la ventana en lugar de `App::serve`:

```rust
#[cfg(feature = "desktop")]
fn main() -> std::io::Result<()> {
    zeyn_desktop::Desktop::new(app()?)
        .id("my-app")
        .title("My App")
        .size(1280, 800)
        .run()
}
```

- `id` da nombre a la carpeta de datos por usuario y mantiene una instancia por id. De forma predeterminada
  es el nombre del ejecutable.
- `title` se muestra hasta que la página define `document.title`; después el título de la ventana sigue
  a la página.
- `size` es el primer tamaño en píxeles lógicos. Los siguientes inicios reutilizan el último tamaño y posición.

`run` se bloquea hasta que se cierra la ventana, detiene el servidor y termina el proceso. Con
`ZEYN_DESCRIBE=1` imprime la descripción de la app y retorna, de modo que `zeyn check` y `zeyn routes`
siguen funcionando.

Para ocultar la ventana de consola de una compilación release en Windows, pon esto al principio de `main.rs`:

```rust
#![cfg_attr(all(feature = "desktop", not(debug_assertions)), windows_subsystem = "windows")]
```

`examples/adisyon` es un ejemplo completo, una app de punto de venta:

```
cargo run -p zeyn-example-adisyon --features desktop
```

## Compilar el ejecutable

```
cd my-app
zeyn build-desktop                    # same as --features desktop
zeyn build-desktop --package my-app --features desktop,postgres
```

Ejecuta `cargo build --release` con optimización en tiempo de enlace, una unidad de codegen y símbolos
eliminados, y luego imprime la ruta y el tamaño de cada ejecutable. La velocidad se mantiene en `opt-level = 3`.

### Icono, información de versión y manifiesto (Windows)

Windows los lee de recursos dentro del exe. Añade un script de compilación que los incorpore solo para
la compilación de escritorio:

```toml
[features]
desktop = ["dep:zeyn-desktop", "dep:winresource"]

[build-dependencies]
winresource = { version = "0.1", optional = true, default-features = false }
```

`build.rs`:

```rust
fn main() {
    #[cfg(feature = "desktop")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app.ico")
            .set("ProductName", "My App")
            .set("FileDescription", "My App")
            .set_manifest_file("assets/app.exe.manifest");
        res.compile().unwrap();
    }
}
```

El icono debe tener el id de recurso 1 (el valor predeterminado de `set_icon`); la ventana y la barra de tareas lo usan.
La versión proviene de `package.version`. Consulta `examples/adisyon/build.rs` y
`examples/adisyon/assets` para un manifiesto con reconocimiento de DPI por monitor, UTF-8 y controles
comunes 6. La toolchain MSVC encuentra `rc.exe` en el SDK de Windows; la toolchain GNU usa
`windres`.

## Runtime de WebView2

Windows 11 y las versiones actuales de Windows 10 tienen el runtime de WebView2 preinstalado y lo actualizan
por sí mismos (el runtime evergreen). El exe no lo incluye, lo que lo mantiene pequeño.

Para máquinas más antiguas o restringidas:

- Distribuye el bootstrapper evergreen de Microsoft (`MicrosoftEdgeWebview2Setup.exe`, unos 2 MB) con
  tu instalador y ejecútalo con `/silent /install` cuando falte el runtime. Descarga
  el runtime una vez.
- Para máquinas sin internet, usa el instalador evergreen independiente (grande, sin conexión), o el
  runtime de versión fija colocado junto al exe. Una versión fija no se actualiza sola, así que
  tú te encargas de sus actualizaciones de seguridad.

Sin el runtime la ventana no puede abrirse y `run` devuelve un error.

## Modelo de seguridad

El servidor es accesible solo para la propia ventana de la app.

- **Solo loopback.** El listener se enlaza a `127.0.0.1` en un puerto que elige el sistema. Nunca se enlaza
  a `0.0.0.0` ni a una dirección de la LAN.
- **Token de inicio.** Cada inicio genera un token aleatorio de 256 bits. La ventana abre primero
  `/_desktop/start?t=<token>`, que establece el token como una cookie `HttpOnly`, `SameSite=Strict` y
  redirige a `/`. Toda otra solicitud debe llevar el token en esa cookie o en la
  cabecera `x-zeyn-desktop`, o recibe `403`. Los recursos estáticos también están protegidos. El token vive solo
  en memoria y se compara en tiempo constante.
- **Host y Origin.** La cabecera `Host` debe ser exactamente `127.0.0.1:<port>`, lo que impide el DNS
  rebinding (un sitio cuyo nombre se resuelve a 127.0.0.1 envía su propio nombre como host). Una cabecera `Origin`,
  cuando está presente, debe ser exactamente `http://127.0.0.1:<port>`. Ambas se aplican a toda ruta,
  incluidos los endpoints de inicio y de foco.
- **Otros programas locales** pueden conectarse al puerto pero no conocen el token. Otras cuentas de
  usuario no pueden leer el archivo de instancia de la carpeta de datos por usuario.
- **Navegación.** La ventana solo muestra su propio origen, sus URL `blob:` y `about:blank`.
  Los enlaces `http`, `https` y `mailto` se abren en el manejador predeterminado del sistema; cualquier otro esquema
  (`file:`, manejadores de protocolo personalizados, `javascript:`) se descarta. `window.open` nunca crea un
  segundo WebView.
- **Devtools** están activadas en las compilaciones debug y desactivadas en las compilaciones release.
- **Las descargas** se permiten solo desde el origen de la app y van a la carpeta Descargas del usuario,
  sin sobrescribir nunca un archivo (`orders.csv`, `orders (1).csv`, ...). Varias descargas seguidas se
  permiten sin preguntar. Las demás solicitudes de permisos (cámara, micrófono, portapapeles) reciben el
  aviso normal del WebView.

Cosas que la protección no cubre:

- Un programa que se ejecuta como el mismo usuario puede leer la memoria del proceso o adjuntar un depurador; ninguna
  app local puede defenderse de eso.
- WebView2 respeta la variable de entorno `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`, así que quien
  pueda establecer variables de entorno para el usuario puede activar la depuración remota. Mismo usuario, mismo
  límite que arriba.
- La autenticación propia de la app sigue aplicándose dentro de la ventana. `DevAuth` en una compilación de escritorio
  da acceso total a quien esté sentado ante la máquina.

## Comportamiento

- **Instancia única.** El primer inicio escribe `instance` (puerto y una clave de foco independiente) en
  la carpeta de datos. Un segundo inicio envía la clave a `/_desktop/focus`; la primera ventana pasa
  al frente y el segundo proceso termina en unos 150 ms. Cuando dos inicios arrancan en el
  mismo instante, ambos pueden abrirse.
- **Estado de la ventana.** El tamaño, la posición y el indicador de maximizada se guardan en `window.json` al cerrar.
  Una posición guardada que ya no está en ningún monitor se ignora.
- **Apagado.** Cerrar la ventana libera el WebView, detiene el servidor de forma ordenada (2 segundos como
  máximo) y termina el proceso. Los procesos del WebView terminan con él.
- **Carpeta de datos.** `%LOCALAPPDATA%\<id>` en Windows, `~/Library/Application Support/<id>` en
  macOS, `~/.local/share/<id>` en Linux. El perfil del WebView (caché, almacenamiento local) vive en
  `webview` dentro de ella, así que el exe también funciona desde una carpeta de solo lectura como Program Files.
- **Sin icono en la bandeja.** No está incorporado.

## Tamaño y memoria

Medido en Windows 11 con `examples/adisyon`, compilación release, runtime de WebView2 156:

| | |
|---|---|
| exe de `zeyn build-desktop` | 6.72 MB (7.25 MB con el perfil release simple) |
| Proceso de la app, en reposo | 4.3 MB privados, 25.6 MB de working set |
| Proceso de la app y procesos de WebView2, en reposo | 7 procesos, unos 180 MB privados, 370 MB de working set |
| Lo mismo con `--disable-gpu` | unos 114 MB privados, 330 MB de working set |
| Segundo inicio hasta que termina | unos 130 ms |

El WebView domina. Su proceso de GPU por sí solo ocupa unos 88 MB. Para pantallas simples en máquinas con poca memoria,
inicia el exe con `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu` (por ejemplo
desde un acceso directo o un script lanzador) para ahorrar unos 65 MB, a costa de un renderizado por software.
