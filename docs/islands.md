# WASM Islands

Zeyn renders pages on the server. Some surfaces are too interactive for that: a canvas editor, a
diagram, a heavy chart. An **island** is a Rust component compiled to WebAssembly that the server
places on an otherwise server rendered page. Everything around it stays plain HTML.

Islands are behind the `islands` Cargo feature of `zeyn`. Apps that do not turn it on compile none
of this, and pages without an island pay nothing even when it is on (see Performance below).

## 1. The pieces

| Piece | Where | Role |
|---|---|---|
| `zeyn::ui::Island` | server | Placeholder node: props, fallback content, bound actions |
| `App::islands(dir)` | server | Serves the built files and the loader |
| `zeyn-island` | shared and browser | `Remote` (the contract of a callable action), `Island::call` (client) |
| A shared crate | both | Props, messages and remotes as plain serde types |
| An island crate | browser | `cdylib` that exports `mount(root)` and optionally `unmount(root)` |
| `zeyn build-islands` | CLI | Builds island crates to hashed files and a manifest |

A typical layout, as in `examples/showcase`:

```
examples/showcase/
  shared/              showcase-shared: props, Count, SaveCount (serde + zeyn-island only)
  islands/counter/     the island crate, [lib] name = "counter", crate-type = ["cdylib", "rlib"]
  src/islands.rs       the screen and the action
  dist/islands/        output of zeyn build-islands (not committed)
```

## 2. Shared types

The shared crate depends on `serde` and on `zeyn-island` without features, so the server compiles
no browser code. A remote names an action an island may call and fixes its types:

```rust
#[derive(Serialize, Deserialize)]
pub struct Count { pub value: i64 }

pub struct SaveCount;

impl zeyn_island::Remote for SaveCount {
    const NAME: &'static str = "save_count";
    type Input = Count;
    type Output = Count;
}
```

## 3. Server side

```rust
#[action(permission = "showcase.islands.write")]
pub async fn save_count(count: Count, store: CounterStore) -> Result<Count> { ... }

Island::new("counter")
    .props(&CounterProps { start, texts })
    .label(t!("islands.counter"))
    .fallback(Alert::info(t!("islands.fallback")))
    .action::<SaveCount, _>(save_count)
```

`action::<R, A>` only compiles when the action's input is `R::Input` and its output is
`Result<R::Output>`, so the two sides cannot drift apart. The island can call only the actions its
node binds.

`fallback` takes any nodes: a text summary, a read only grid. It shows before the island loads,
when JavaScript is off and when loading fails. Its grids and other components work as usual.

Loading: `.on_interaction()` waits for the first pointer, touch or focus on the placeholder,
`.on_idle()` loads when the browser is idle, and the default loads when the placeholder comes
within 200 px of the viewport. Loading never blocks the page.

Register the output folder once: `App::islands(concat!(env!("CARGO_MANIFEST_DIR"), "/dist/islands"))`.
Without a manifest there the server logs a warning and islands show their fallback.

## 4. Island side

```rust
#[wasm_bindgen]
pub fn mount(root: web_sys::Element) -> Result<(), JsValue> {
    let island = zeyn_island::Island::new(root.clone());
    let props: CounterProps = island.props().map_err(|e| JsValue::from_str(&e.message))?;
    // build DOM under root, attach listeners
    zeyn_island::spawn(async move {
        match island.call::<SaveCount>(&Count { value: 3 }).await {
            Ok(saved) => { /* ... */ }
            Err(e) => { /* e.message is localized by the server */ }
        }
    });
    Ok(())
}
```

`Island::call` posts to `POST /_zeyn/data` with the island's screen, params and node path, the
remote's name as `key`, and the input. It sends the page's CSRF token in `x-zeyn-csrf` and the
session cookie. The server checks, as for every component: CSRF, the screen's access, the
feature's guards, the action's permission, then runs the interceptors and the action with the time
limit. There is no other way in: an island cannot reach an action that its node does not bind, and
nothing about islands bypasses a check.

Errors come back as `CallError { status, code, message }`. `status` is `0` for local failures
(`network`, `props`, `input`, `answer`).

Export `unmount(root)` when the island holds timers, animation frames or sockets. The loader calls
it when the placeholder leaves the document (navigation, a closed window).

Translations: pass the texts the island shows in its props, already localized by the server, so
the island ships no catalog.

## 5. Build

```
cd examples/showcase
zeyn build-islands            # every crate in islands/, output in dist/islands
zeyn build-islands --out public/islands islands/counter
```

For each crate it runs:

1. `cargo build --lib --release --target wasm32-unknown-unknown` with size settings passed as
   `--config` (`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, no debug
   info), so the app's own release profile is untouched;
2. `wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section`
   (the `wasm-bindgen-cli` version must match the `wasm-bindgen` crate, `0.2.130`);
3. `wasm-opt -Oz` when binaryen is on the `PATH`;
4. writes `{name}.{hash}.js` and `{name}_bg.{hash}.wasm` (64 bit FNV hash of the content) and
   updates `manifest.json`.

`zeyn dev` does the same before each app build when the app has an `islands/` folder, and does not
watch `dist/`.

The server serves only file names listed in the manifest, at `/_zeyn/islands/{file}`, with
`application/wasm` or `text/javascript` and `Cache-Control: public, max-age=31536000, immutable`.
Files are read from disk per request, so they cost no server memory.

## 6. Content Security Policy

Compiling WebAssembly needs `'wasm-unsafe-eval'` in `script-src`. Zeyn adds it **only** to pages
whose screen holds an island; every other page keeps the strict policy of `docs/runtime-protocol.md`.
`'wasm-unsafe-eval'` allows `WebAssembly.compile` and `instantiate` and nothing else: `eval`,
`new Function` and inline scripts stay blocked, and modules still come only from the app's origin.

A policy belongs to a document and cannot be relaxed later. So when `nav.js` loads a screen that
holds an island (the partial answer carries `x-zeyn-islands: 1`) into a document that was loaded
without it, it falls back to a full page load. Once a document has the island policy, moving
between island and plain screens stays in place.

The server finds islands by the attribute `data-zeyn-island="`; escaped user data can never
produce the quote, so text typed by a user cannot turn the policy on.

## 7. Canvas surfaces

`zeyn::ui::Canvas::new(name, description)` is an island for drawing. The description is required:
a canvas is invisible to assistive technology, so the page must say what it shows and which keys
operate it. The server renders it as a paragraph above the island and points the placeholder's
`aria-describedby` at it.

On the island side, `zeyn-island` with the `canvas` feature provides `canvas::Surface`:

```rust
let surface = Surface::new(&stage, Editor::new(..), Prefer::TwoD, &texts.canvas, &described_by, 420)?;
surface.update(|editor| editor.add_box());   // change the scene, draw on the next frame
```

| Concern | What the surface does |
|---|---|
| Graphics | `Prefer::Gl` tries WebGL 2 and falls back to 2D; `webgpu_available()` tells whether `navigator.gpu` exists for scenes that set up WebGPU on `surface.canvas()` themselves |
| High DPI | The backing store follows `devicePixelRatio` and the element size (a `ResizeObserver`); 2D drawing is pre scaled, so the scene works in CSS pixels |
| Input | Pointer down, move and up (with pointer capture), wheel (line and page deltas turned into pixels) and keys, as `Input` values in CSS pixels |
| Frames | `requestAnimationFrame` only after an input the scene used, an `update`, a resize, or while `Scene::animating` returns `true`. A still picture costs no CPU |
| Defaults | An input the scene uses has its default prevented (arrows do not scroll the page); return `false` for Tab so focus can leave |
| Clean up | `destroy()` removes listeners, the observer and the canvas; call it from `unmount` |

The canvas gets `tabindex="0"`, `role="application"`, the label and `aria-describedby`.

Accessibility pattern, as in the showcase diagram editor (`examples/showcase/islands/diagram`):

- **Keyboard on the canvas:** `]` and `[` select shapes, arrows move them, Delete removes, `+`,
  `-` and `0` zoom, Escape clears the selection. Keys only act while the canvas has focus.
- **Ordinary controls:** a toolbar of buttons does the same without the canvas.
- **Text summary:** a list under the canvas describes every shape and follows each change; a
  `role="status"` line announces the selection and save results.
- **Fallback:** the server renders the same summary, so users without WebAssembly still read the
  diagram.

The editor saves through `SaveDiagram`, and the server clamps what it receives
(`Diagram::clamped`: at most 200 shapes, unique ids, labels of 40 characters, bounded coordinates)
because the island is never trusted.

## 8. Performance

| What | Number |
|---|---|
| Plain page HTML, requests, policy | unchanged: no meta, no loader, no extra request |
| `nav.js` | +281 bytes (the full load fallback) |
| `/_zeyn/islands.js` loader, island pages only | 3.9 KB, 1.7 KB gzip |
| Counter island `.wasm` (no wasm-opt) | 127.6 KB, 52.8 KB gzip |
| Counter island glue `.js` | 18.0 KB, 4.2 KB gzip |
| Diagram editor `.wasm` (canvas surface, no wasm-opt) | 169.4 KB, 67.8 KB gzip |
| Diagram editor with live editing (adds `yrs`, see `docs/realtime.md`) | 408.4 KB, 144.3 KB gzip |
| Diagram editor glue `.js` | 30.3 KB, 6.1 KB gzip |
| Server memory per island | none; files stream from disk, props are rendered per request |

Most of the counter's size is `serde_json` and the formatting code of `core`. Islands that share
those pay for them once each; wasm-opt usually takes another 10 to 20 percent off.

## 9. Limits

- An island inside a window (`open_window`) needs the opener page to hold the island policy; place
  windows with islands on island pages or open them as screens.
- Closures attached with `Closure::forget` live as long as the page; release them in `unmount`
  when an island can be mounted many times.
- Islands render nothing on the server themselves; the fallback is what search engines, screen
  readers before load and users without JavaScript get. Make it meaningful.
