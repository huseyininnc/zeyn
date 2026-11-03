# WASM Islands

Zeyn 在服务器上渲染页面。有些界面的交互性太强，不适合这样做：画布编辑器、图表、重型图表。**Island（孤岛）** 是编译为 WebAssembly 的 Rust 组件，由服务器放置在一个其余部分仍由服务器渲染的页面上。它周围的一切仍是普通 HTML。

Islands 位于 `zeyn` 的 `islands` Cargo feature 之后。不启用它的应用不会编译其中任何内容；即使启用了它，没有 island 的页面也不会产生任何开销（见下文“性能”）。

## 1. 组成部分

| 部分 | 位置 | 作用 |
|---|---|---|
| `zeyn::ui::Island` | 服务器 | 占位节点：props、回退内容、绑定的 action |
| `App::islands(dir)` | 服务器 | 提供构建好的文件和加载器 |
| `zeyn-island` | 共享和浏览器 | `Remote`（可调用 action 的契约）、`Island::call`（客户端） |
| 共享 crate | 两端 | 以普通 serde 类型表示的 props、消息和 remote |
| island crate | 浏览器 | 导出 `mount(root)` 以及可选的 `unmount(root)` 的 `cdylib` |
| `zeyn build-islands` | CLI | 将 island crate 构建为带哈希的文件和清单 |

典型布局，如 `examples/showcase` 中所示：

```
examples/showcase/
  shared/              showcase-shared: props, Count, SaveCount (serde + zeyn-island only)
  islands/counter/     the island crate, [lib] name = "counter", crate-type = ["cdylib", "rlib"]
  src/islands.rs       the screen and the action
  dist/islands/        output of zeyn build-islands (not committed)
```

## 2. 共享类型

共享 crate 依赖 `serde` 和不带 feature 的 `zeyn-island`，因此服务器不会编译任何浏览器代码。remote 指明 island 可以调用的 action，并固定其类型：

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

## 3. 服务器端

```rust
#[action(permission = "showcase.islands.write")]
pub async fn save_count(count: Count, store: CounterStore) -> Result<Count> { ... }

Island::new("counter")
    .props(&CounterProps { start, texts })
    .label(t!("islands.counter"))
    .fallback(Alert::info(t!("islands.fallback")))
    .action::<SaveCount, _>(save_count)
```

只有当 action 的输入是 `R::Input`，且其输出是 `Result<R::Output>` 时，`action::<R, A>` 才能通过编译，因此两端不会出现偏差。island 只能调用其节点所绑定的 action。

`fallback` 接受任意节点：文本摘要、只读表格。它在 island 加载之前、JavaScript 关闭时以及加载失败时显示。其中的表格和其他组件照常工作。

加载方式：`.on_interaction()` 等待占位元素上的第一次指针、触摸或聚焦；`.on_idle()` 在浏览器空闲时加载；默认在占位元素进入视口 200 px 范围内时加载。加载从不阻塞页面。

只需注册一次输出目录：`App::islands(concat!(env!("CARGO_MANIFEST_DIR"), "/dist/islands"))`。如果该处没有清单，服务器会记录一条警告，island 显示其回退内容。

## 4. Island 端

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

`Island::call` 向 `POST /_zeyn/data` 发送请求，携带 island 的屏幕、参数和节点路径，把 remote 的名称作为 `key`，并带上输入。它在 `x-zeyn-csrf` 中发送页面的 CSRF 令牌，并带上会话 cookie。服务器与对待每个组件一样进行检查：CSRF、屏幕的访问控制、feature 的 guard、action 的权限，然后在时间限制内运行拦截器和 action。没有别的入口：island 无法访问其节点未绑定的 action，island 的任何机制都不会绕过任何检查。

错误以 `CallError { status, code, message }` 返回。对于本地失败（`network`、`props`、`input`、`answer`），`status` 为 `0`。

当 island 持有定时器、动画帧或套接字时，请导出 `unmount(root)`。当占位元素离开文档（导航、窗口被关闭）时，加载器会调用它。

翻译：把 island 要显示的文本放进它的 props，由服务器预先本地化，这样 island 无需携带文本目录。

## 5. 构建

```
cd examples/showcase
zeyn build-islands            # every crate in islands/, output in dist/islands
zeyn build-islands --out public/islands islands/counter
```

对每个 crate，它会运行：

1. `cargo build --lib --release --target wasm32-unknown-unknown`，并通过 `--config` 传入体积相关设置（`opt-level = "z"`、`lto = true`、`codegen-units = 1`、`panic = "abort"`，不含调试信息），因此应用自己的 release 配置不受影响；
2. `wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section`（`wasm-bindgen-cli` 的版本必须与 `wasm-bindgen` crate 一致，即 `0.2.130`）；
3. 当 binaryen 位于 `PATH` 中时执行 `wasm-opt -Oz`；
4. 写出 `{name}.{hash}.js` 和 `{name}_bg.{hash}.wasm`（内容的 64 位 FNV 哈希），并更新 `manifest.json`。

当应用有 `islands/` 文件夹时，`zeyn dev` 会在每次应用构建之前执行同样的操作，并且不会监视 `dist/`。

服务器只提供清单中列出的文件名，路径为 `/_zeyn/islands/{file}`，使用 `application/wasm` 或 `text/javascript`，并带有 `Cache-Control: public, max-age=31536000, immutable`。文件在每次请求时从磁盘读取，因此不占用服务器内存。

## 6. 内容安全策略

编译 WebAssembly 需要在 `script-src` 中加入 `'wasm-unsafe-eval'`。Zeyn **只**把它加到其屏幕包含 island 的页面上；其他所有页面都保持 `docs/runtime-protocol.md` 中的严格策略。`'wasm-unsafe-eval'` 只允许 `WebAssembly.compile` 和 `instantiate`，别无其他：`eval`、`new Function` 和内联脚本仍被阻止，模块也仍然只能来自应用自身的源。

策略属于文档，之后无法放宽。因此，当 `nav.js` 把一个包含 island 的屏幕（局部响应带有 `x-zeyn-islands: 1`）加载到一个在没有该策略的情况下加载的文档中时，它会回退为整页加载。一旦文档具有 island 策略，在 island 屏幕和普通屏幕之间切换就会就地进行。

服务器通过属性 `data-zeyn-island="` 找到 island；经过转义的用户数据永远无法产生该引号，因此用户输入的文本不能开启该策略。

## 7. 画布界面

`zeyn::ui::Canvas::new(name, description)` 是用于绘图的 island。description 是必填的：画布对辅助技术不可见，所以页面必须说明它显示什么以及用哪些键操作。服务器把它渲染为 island 上方的一个段落，并让占位元素的 `aria-describedby` 指向它。

在 island 端，启用 `canvas` feature 的 `zeyn-island` 提供 `canvas::Surface`：

```rust
let surface = Surface::new(&stage, Editor::new(..), Prefer::TwoD, &texts.canvas, &described_by, 420)?;
surface.update(|editor| editor.add_box());   // change the scene, draw on the next frame
```

| 关注点 | Surface 的做法 |
|---|---|
| 图形 | `Prefer::Gl` 先尝试 WebGL 2，失败则回退到 2D；对于自行在 `surface.canvas()` 上设置 WebGPU 的场景，`webgpu_available()` 会告知 `navigator.gpu` 是否存在 |
| 高 DPI | 后备存储跟随 `devicePixelRatio` 和元素尺寸（通过 `ResizeObserver`）；2D 绘制已预先缩放，因此场景以 CSS 像素工作 |
| 输入 | 指针按下、移动和抬起（带指针捕获）、滚轮（将行和页增量换算为像素）以及按键，以 CSS 像素表示的 `Input` 值传入 |
| 帧 | 只有在场景用到的输入、`update`、尺寸调整之后，或当 `Scene::animating` 返回 `true` 时才使用 `requestAnimationFrame`。静止的画面不消耗 CPU |
| 默认行为 | 场景用到的输入会被阻止默认行为（方向键不会滚动页面）；对于 Tab 请返回 `false`，以便焦点可以离开 |
| 清理 | `destroy()` 移除监听器、观察器和画布；请在 `unmount` 中调用它 |

画布会获得 `tabindex="0"`、`role="application"`、标签以及 `aria-describedby`。

无障碍模式，如 showcase 中的图表编辑器（`examples/showcase/islands/diagram`）所示：

- **画布上的键盘操作：** `]` 和 `[` 选择图形，方向键移动它们，Delete 删除，`+`、`-` 和 `0` 缩放，Escape 清除选择。按键只在画布有焦点时才起作用。
- **普通控件：** 一排按钮的工具栏可以在不使用画布的情况下完成同样的操作。
- **文字摘要：** 画布下方的列表描述每个图形并随每次更改更新；一行 `role="status"` 会播报选择和保存结果。
- **回退：** 服务器渲染同样的摘要，因此没有 WebAssembly 的用户仍然可以读到图表内容。

编辑器通过 `SaveDiagram` 保存，服务器会限制它收到的内容（`Diagram::clamped`：最多 200 个图形、唯一的 id、40 个字符的标签、有界的坐标），因为 island 永远不被信任。

## 8. 性能

| 项目 | 数值 |
|---|---|
| 普通页面的 HTML、请求、策略 | 不变：没有 meta，没有加载器，没有额外请求 |
| `nav.js` | +281 字节（整页加载回退） |
| `/_zeyn/islands.js` 加载器，仅 island 页面 | 3.9 KB，gzip 后 1.7 KB |
| 计数器 island 的 `.wasm`（未使用 wasm-opt） | 127.6 KB，gzip 后 52.8 KB |
| 计数器 island 的胶水 `.js` | 18.0 KB，gzip 后 4.2 KB |
| 图表编辑器 `.wasm`（画布 surface，未使用 wasm-opt） | 169.4 KB，gzip 后 67.8 KB |
| 带实时编辑的图表编辑器（加入 `yrs`，见 `docs/realtime.md`） | 408.4 KB，gzip 后 144.3 KB |
| 图表编辑器的胶水 `.js` | 30.3 KB，gzip 后 6.1 KB |
| 每个 island 的服务器内存 | 无；文件从磁盘流式读取，props 在每次请求时渲染 |

计数器的大部分体积来自 `serde_json` 和 `core` 的格式化代码。共享这些代码的 island 各自付出一次；wasm-opt 通常还能再减少 10% 到 20%。

## 9. 限制

- 窗口（`open_window`）内的 island 要求打开它的页面带有 island 策略；请把含 island 的窗口放在 island 页面上，或将它们作为屏幕打开。
- 用 `Closure::forget` 附加的闭包与页面同寿；当一个 island 可能被多次挂载时，请在 `unmount` 中释放它们。
- island 本身不在服务器上渲染任何内容；搜索引擎、加载前的屏幕阅读器以及没有 JavaScript 的用户得到的是回退内容。请让它有意义。
