# Islas WASM

Zeyn renderiza las páginas en el servidor. Algunas superficies son demasiado interactivas para eso: un editor de canvas, un
diagrama, un gráfico pesado. Una **isla** (island) es un componente de Rust compilado a WebAssembly que el servidor
coloca en una página que por lo demás se renderiza en el servidor. Todo lo que la rodea sigue siendo HTML simple.

Las islas están tras la feature de Cargo `islands` de `zeyn`. Las apps que no la activan no compilan nada
de esto, y las páginas sin una isla no pagan nada ni siquiera cuando está activada (consulta Rendimiento más abajo).

## 1. Las piezas

| Pieza | Dónde | Función |
|---|---|---|
| `zeyn::ui::Island` | servidor | Nodo marcador de posición: props, contenido alternativo, actions enlazadas |
| `App::islands(dir)` | servidor | Sirve los archivos compilados y el cargador |
| `zeyn-island` | compartido y navegador | `Remote` (el contrato de una action invocable), `Island::call` (cliente) |
| Un crate compartido | ambos | Props, mensajes y remotes como tipos serde simples |
| Un crate de isla | navegador | `cdylib` que exporta `mount(root)` y, opcionalmente, `unmount(root)` |
| `zeyn build-islands` | CLI | Compila los crates de isla a archivos con hash y un manifiesto |

Una disposición típica, como en `examples/showcase`:

```
examples/showcase/
  shared/              showcase-shared: props, Count, SaveCount (serde + zeyn-island only)
  islands/counter/     the island crate, [lib] name = "counter", crate-type = ["cdylib", "rlib"]
  src/islands.rs       the screen and the action
  dist/islands/        output of zeyn build-islands (not committed)
```

## 2. Tipos compartidos

El crate compartido depende de `serde` y de `zeyn-island` sin features, de modo que el servidor no compila
código de navegador. Un remote nombra una action que una isla puede invocar y fija sus tipos:

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

## 3. Lado del servidor

```rust
#[action(permission = "showcase.islands.write")]
pub async fn save_count(count: Count, store: CounterStore) -> Result<Count> { ... }

Island::new("counter")
    .props(&CounterProps { start, texts })
    .label(t!("islands.counter"))
    .fallback(Alert::info(t!("islands.fallback")))
    .action::<SaveCount, _>(save_count)
```

`action::<R, A>` solo compila cuando la entrada de la action es `R::Input` y su salida es
`Result<R::Output>`, de modo que los dos lados no pueden desviarse. La isla solo puede invocar las actions que su
nodo enlaza.

`fallback` acepta cualquier nodo: un resumen de texto, un grid de solo lectura. Se muestra antes de que la isla cargue,
cuando JavaScript está desactivado y cuando la carga falla. Sus grids y demás componentes funcionan como de costumbre.

Carga: `.on_interaction()` espera al primer puntero, toque o foco sobre el marcador de posición,
`.on_idle()` carga cuando el navegador está inactivo, y el valor predeterminado carga cuando el marcador de posición se acerca
a menos de 200 px del viewport. La carga nunca bloquea la página.

Registra la carpeta de salida una vez: `App::islands(concat!(env!("CARGO_MANIFEST_DIR"), "/dist/islands"))`.
Sin un manifiesto allí, el servidor registra una advertencia y las islas muestran su contenido alternativo.

## 4. Lado de la isla

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

`Island::call` envía un POST a `POST /_zeyn/data` con la pantalla, los params y la ruta de nodo de la isla, el
nombre del remote como `key`, y la entrada. Envía el token CSRF de la página en `x-zeyn-csrf` y la
cookie de sesión. El servidor comprueba, como para cada componente: CSRF, el acceso de la pantalla, los
guards de la feature, el permiso de la action, y después ejecuta los interceptores y la action con el límite
de tiempo. No hay otra vía de entrada: una isla no puede alcanzar una action que su nodo no enlaza, y
nada relacionado con las islas se salta una comprobación.

Los errores vuelven como `CallError { status, code, message }`. `status` es `0` para los fallos locales
(`network`, `props`, `input`, `answer`).

Exporta `unmount(root)` cuando la isla mantiene temporizadores, fotogramas de animación o sockets. El cargador lo llama
cuando el marcador de posición abandona el documento (navegación, una ventana cerrada).

Traducciones: pasa los textos que muestra la isla en sus props, ya localizados por el servidor, de modo que
la isla no incluye ningún catálogo.

## 5. Compilación

```
cd examples/showcase
zeyn build-islands            # every crate in islands/, output in dist/islands
zeyn build-islands --out public/islands islands/counter
```

Para cada crate ejecuta:

1. `cargo build --lib --release --target wasm32-unknown-unknown` con los ajustes de tamaño pasados como
   `--config` (`opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, sin información
   de depuración), de modo que el perfil release propio de la app no se toca;
2. `wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section`
   (la versión de `wasm-bindgen-cli` debe coincidir con el crate `wasm-bindgen`, `0.2.130`);
3. `wasm-opt -Oz` cuando binaryen está en el `PATH`;
4. escribe `{name}.{hash}.js` y `{name}_bg.{hash}.wasm` (hash FNV de 64 bits del contenido) y
   actualiza `manifest.json`.

`zeyn dev` hace lo mismo antes de cada compilación de la app cuando la app tiene una carpeta `islands/`, y no
vigila `dist/`.

El servidor sirve solo los nombres de archivo listados en el manifiesto, en `/_zeyn/islands/{file}`, con
`application/wasm` o `text/javascript` y `Cache-Control: public, max-age=31536000, immutable`.
Los archivos se leen del disco en cada solicitud, de modo que no cuestan memoria del servidor.

## 6. Content Security Policy

Compilar WebAssembly necesita `'wasm-unsafe-eval'` en `script-src`. Zeyn lo añade **solo** a las páginas
cuya pantalla contiene una isla; todas las demás páginas mantienen la política estricta de `docs/runtime-protocol.md`.
`'wasm-unsafe-eval'` permite `WebAssembly.compile` e `instantiate` y nada más: `eval`,
`new Function` y los scripts en línea siguen bloqueados, y los módulos siguen proviniendo solo del origen de la app.

Una política pertenece a un documento y no se puede relajar después. Por eso, cuando `nav.js` carga una pantalla que
contiene una isla (la respuesta parcial lleva `x-zeyn-islands: 1`) en un documento que se cargó
sin ella, recurre a una carga de página completa. Una vez que un documento tiene la política de islas, moverse
entre pantallas con islas y pantallas simples se mantiene en el sitio.

El servidor encuentra las islas por el atributo `data-zeyn-island="`; los datos de usuario escapados nunca pueden
producir la comilla, de modo que el texto escrito por un usuario no puede activar la política.

## 7. Superficies de canvas

`zeyn::ui::Canvas::new(name, description)` es una isla para dibujar. La descripción es obligatoria:
un canvas es invisible para las tecnologías de apoyo, por lo que la página debe decir qué muestra y qué teclas
lo manejan. El servidor la renderiza como un párrafo sobre la isla y apunta el `aria-describedby` del marcador
de posición hacia él.

En el lado de la isla, `zeyn-island` con la feature `canvas` proporciona `canvas::Surface`:

```rust
let surface = Surface::new(&stage, Editor::new(..), Prefer::TwoD, &texts.canvas, &described_by, 420)?;
surface.update(|editor| editor.add_box());   // change the scene, draw on the next frame
```

| Aspecto | Qué hace la superficie |
|---|---|
| Gráficos | `Prefer::Gl` prueba WebGL 2 y recurre a 2D; `webgpu_available()` indica si existe `navigator.gpu` para escenas que configuran WebGPU por sí mismas en `surface.canvas()` |
| Alta densidad de píxeles (High DPI) | El almacén de respaldo sigue a `devicePixelRatio` y al tamaño del elemento (un `ResizeObserver`); el dibujo 2D está preescalado, de modo que la escena trabaja en píxeles CSS |
| Entrada | Puntero abajo, movimiento y arriba (con captura de puntero), rueda (los deltas de línea y de página se convierten a píxeles) y teclas, como valores `Input` en píxeles CSS |
| Fotogramas | `requestAnimationFrame` solo tras una entrada que usó la escena, un `update`, un redimensionado, o mientras `Scene::animating` devuelve `true`. Una imagen estática no cuesta CPU |
| Valores predeterminados | A una entrada que usa la escena se le impide su comportamiento predeterminado (las flechas no desplazan la página); devuelve `false` para Tab de modo que el foco pueda salir |
| Limpieza | `destroy()` elimina los listeners, el observer y el canvas; llámalo desde `unmount` |

El canvas recibe `tabindex="0"`, `role="application"`, la etiqueta y `aria-describedby`.

Patrón de accesibilidad, como en el editor de diagramas del showcase (`examples/showcase/islands/diagram`):

- **Teclado en el canvas:** `]` y `[` seleccionan formas, las flechas las mueven, Supr las elimina, `+`,
  `-` y `0` hacen zoom, Escape borra la selección. Las teclas solo actúan mientras el canvas tiene el foco.
- **Controles ordinarios:** una barra de herramientas de botones hace lo mismo sin el canvas.
- **Resumen de texto:** una lista bajo el canvas describe cada forma y sigue cada cambio; una
  línea `role="status"` anuncia la selección y los resultados de guardado.
- **Contenido alternativo:** el servidor renderiza el mismo resumen, de modo que los usuarios sin WebAssembly siguen leyendo el
  diagrama.

El editor guarda mediante `SaveDiagram`, y el servidor acota lo que recibe
(`Diagram::clamped`: como máximo 200 formas, ids únicos, etiquetas de 40 caracteres, coordenadas acotadas)
porque la isla nunca es de confianza.

### 7.1 Inspector de propiedades

`PropertyInspector::island(name)` edita el objeto seleccionado. La isla envía `zeyn:inspect` con `detail: { title, schema, values }` desde cualquiera de sus elementos; el inspector responde a cada cambio confirmado con `zeyn:property` (`detail: { name, value }`) en la raíz de la isla, ya comprobado. La isla sigue comprobando lo que guarda y el servidor comprueba lo guardado con `FormSchema::check`.

## 8. Rendimiento

| Qué | Cifra |
|---|---|
| HTML de página simple, solicitudes, política | sin cambios: sin meta, sin cargador, sin solicitud adicional |
| `nav.js` | +281 bytes (el respaldo de carga completa) |
| Cargador `/_zeyn/islands.js`, solo páginas con islas | 3.9 KB, 1.7 KB gzip |
| `.wasm` de la isla contador (sin wasm-opt) | 127.6 KB, 52.8 KB gzip |
| `.js` de enlace de la isla contador | 18.0 KB, 4.2 KB gzip |
| `.wasm` del editor de diagramas (superficie canvas, sin wasm-opt) | 169.4 KB, 67.8 KB gzip |
| Editor de diagramas con edición en vivo (añade `yrs`, consulta `docs/realtime.md`) | 408.4 KB, 144.3 KB gzip |
| `.js` de enlace del editor de diagramas | 30.3 KB, 6.1 KB gzip |
| Memoria del servidor por isla | ninguna; los archivos se transmiten desde el disco, las props se renderizan en cada solicitud |

La mayor parte del tamaño del contador es `serde_json` y el código de formato de `core`. Las islas que comparten
eso lo pagan una vez cada una; wasm-opt suele recortar otro 10 a 20 por ciento.

## 9. Límites

- Una isla dentro de una ventana (`open_window`) necesita que la página que la abre tenga la política de islas; coloca
  las ventanas con islas en páginas con islas o ábrelas como pantallas.
- Los closures adjuntados con `Closure::forget` viven tanto como la página; libéralos en `unmount`
  cuando una isla pueda montarse muchas veces.
- Las islas no renderizan nada en el servidor por sí mismas; el contenido alternativo es lo que reciben los motores de búsqueda, los
  lectores de pantalla antes de la carga y los usuarios sin JavaScript. Haz que tenga sentido.
