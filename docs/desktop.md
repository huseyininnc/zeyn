# Desktop apps

A Zeyn app can ship as one desktop executable. The `zeyn-desktop` crate starts the app's own
server inside the process and shows it in a native window through the system WebView:

| Platform | WebView | Status |
|----------|---------|--------|
| Windows 10 and 11 | WebView2 (Edge) | built and checked by hand |
| macOS | WKWebView | compiles through `wry`, not checked by hand |
| Linux | WebKitGTK 4.1 | compiles through `wry` when `libwebkit2gtk-4.1-dev` and `libgtk-3-dev` are installed, not checked by hand |

Nothing leaves the machine. The app works offline as long as it does not call remote services
itself.

## Use it

Add the crate behind a feature, so the normal server build does not compile a WebView:

```toml
[features]
desktop = ["dep:zeyn-desktop"]

[dependencies]
zeyn-desktop = { version = "0.0.2", optional = true }
```

Start the window instead of `App::serve`:

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

- `id` names the per user data folder and keeps one instance per id. It defaults to the
  executable name.
- `title` is shown until the page sets `document.title`; after that the window title follows
  the page.
- `size` is the first size in logical pixels. Later launches reuse the last size and position.

`run` blocks until the window closes, stops the server and ends the process. With
`ZEYN_DESCRIBE=1` it prints the app description and returns, so `zeyn check` and `zeyn routes`
keep working.

To hide the console window of a release build on Windows, put this at the top of `main.rs`:

```rust
#![cfg_attr(all(feature = "desktop", not(debug_assertions)), windows_subsystem = "windows")]
```

`examples/adisyon` is a complete example, a point of sale app:

```
cargo run -p zeyn-example-adisyon --features desktop
```

## Build the executable

```
cd my-app
zeyn build-desktop                    # same as --features desktop
zeyn build-desktop --package my-app --features desktop,postgres
```

It runs `cargo build --release` with link time optimization, one codegen unit and stripped
symbols, then prints the path and size of every executable. Speed stays at `opt-level = 3`.

### Icon, version info and manifest (Windows)

Windows reads these from resources inside the exe. Add a build script that embeds them only for
the desktop build:

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

The icon must have resource id 1 (the default of `set_icon`); the window and the taskbar use it.
The version comes from `package.version`. See `examples/adisyon/build.rs` and
`examples/adisyon/assets` for a manifest with per monitor DPI awareness, UTF-8 and common
controls 6. The MSVC toolchain finds `rc.exe` in the Windows SDK; the GNU toolchain uses
`windres`.

## WebView2 runtime

Windows 11 and current Windows 10 have the WebView2 runtime preinstalled and update it
themselves (the evergreen runtime). The exe does not bundle it, which keeps it small.

For older or locked down machines:

- Ship Microsoft's evergreen bootstrapper (`MicrosoftEdgeWebview2Setup.exe`, about 2 MB) with
  your installer and run it with `/silent /install` when the runtime is missing. It downloads
  the runtime once.
- For machines without internet, use the evergreen standalone installer (large, offline), or the
  fixed version runtime placed next to the exe. A fixed version does not update itself, so you
  own its security updates.

Without the runtime the window cannot open and `run` returns an error.

## Security model

The server is reachable only by the app's own window.

- **Loopback only.** The listener binds `127.0.0.1` on a port the system picks. It never binds
  `0.0.0.0` or a LAN address.
- **Launch token.** Each launch makes a random 256 bit token. The window first opens
  `/_desktop/start?t=<token>`, which sets the token as an `HttpOnly`, `SameSite=Strict` cookie and
  redirects to `/`. Every other request must carry the token in that cookie or in the
  `x-zeyn-desktop` header, or it gets `403`. Static assets are guarded too. The token lives only
  in memory and is compared in constant time.
- **Host and Origin.** The `Host` header must be exactly `127.0.0.1:<port>`, which stops DNS
  rebinding (a site whose name resolves to 127.0.0.1 sends its own name as the host). An `Origin`
  header, when present, must be exactly `http://127.0.0.1:<port>`. Both apply to every path,
  including the start and focus endpoints.
- **Other local programs** can connect to the port but do not know the token. Other user
  accounts cannot read the instance file in the per user data folder.
- **Navigation.** The window only shows its own origin, its `blob:` URLs and `about:blank`.
  `http`, `https` and `mailto` links open in the system's default handler; every other scheme
  (`file:`, custom protocol handlers, `javascript:`) is dropped. `window.open` never creates a
  second WebView.
- **Devtools** are on in debug builds and off in release builds.
- **Downloads** are allowed only from the app's origin and go to the user's Downloads folder,
  never overwriting a file (`orders.csv`, `orders (1).csv`, ...). Several downloads in a row are
  allowed without a prompt. Other permission requests (camera, microphone, clipboard) get the
  WebView's normal prompt.

Things the guard does not cover:

- A program running as the same user can read process memory or attach a debugger; no local
  app can defend against that.
- WebView2 honours the `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` environment variable, so whoever
  can set environment variables for the user can turn on remote debugging. Same user, same
  limit as above.
- The app's own authentication still applies inside the window. `DevAuth` in a desktop build
  gives full access to whoever sits at the machine.

## Behaviour

- **Single instance.** The first launch writes `instance` (port and a separate focus key) to
  the data folder. A second launch posts the key to `/_desktop/focus`; the first window comes
  to the front and the second process exits within about 150 ms. When two launches start in the
  same instant, both may open.
- **Window state.** Size, position and the maximized flag are saved to `window.json` on close.
  A saved position that is no longer on any monitor is ignored.
- **Shutdown.** Closing the window drops the WebView, stops the server gracefully (2 seconds at
  most) and ends the process. The WebView processes end with it.
- **Data folder.** `%LOCALAPPDATA%\<id>` on Windows, `~/Library/Application Support/<id>` on
  macOS, `~/.local/share/<id>` on Linux. The WebView profile (cache, local storage) lives in
  `webview` inside it, so the exe also works from a read only folder such as Program Files.
- **No tray icon.** Not built in.

## Size and memory

Measured on Windows 11 with `examples/adisyon`, release build, WebView2 runtime 156:

| | |
|---|---|
| `zeyn build-desktop` exe | 6.72 MB (7.25 MB with the plain release profile) |
| App process, idle | 4.3 MB private, 25.6 MB working set |
| App process and WebView2 processes, idle | 7 processes, about 180 MB private, 370 MB working set |
| Same with `--disable-gpu` | about 114 MB private, 330 MB working set |
| Second launch until it exits | about 130 ms |

The WebView dominates. Its GPU process alone holds about 88 MB. For simple screens on low memory
machines, start the exe with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu` (for example
from a shortcut or launcher script) to save about 65 MB, at the cost of software rendering.
