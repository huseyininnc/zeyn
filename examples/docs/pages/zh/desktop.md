# 桌面应用

Zeyn 应用可以作为单个桌面可执行文件发布。`zeyn-desktop` crate 在进程内启动应用自己的服务器，并通过系统 WebView 在原生窗口中显示它：

| 平台 | WebView | 状态 |
|----------|---------|--------|
| Windows 10 和 11 | WebView2 (Edge) | 已构建并手动检查 |
| macOS | WKWebView | 通过 `wry` 编译，未手动检查 |
| Linux | WebKitGTK 4.1 | 在安装了 `libwebkit2gtk-4.1-dev` 和 `libgtk-3-dev` 时通过 `wry` 编译，未手动检查 |

任何数据都不会离开本机。只要应用自己不调用远程服务，它就可以离线工作。

## 使用方法

把该 crate 放在一个 feature 之后添加，这样常规的服务器构建就不会编译 WebView：

```toml
[features]
desktop = ["dep:zeyn-desktop"]

[dependencies]
zeyn-desktop = { version = "0.0.8", optional = true }
```

用启动窗口代替 `App::serve`：

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

- `id` 为每个用户的数据文件夹命名，并保证每个 id 只有一个实例。默认为可执行文件名。
- `title` 在页面设置 `document.title` 之前显示；之后窗口标题跟随页面。
- `size` 是以逻辑像素表示的首次尺寸。之后的启动会沿用上次的尺寸和位置。

`run` 会一直阻塞到窗口关闭，然后停止服务器并结束进程。设置 `ZEYN_DESCRIBE=1` 时，它会打印应用描述并返回，因此 `zeyn check` 和 `zeyn routes` 仍可正常使用。

若要在 Windows 上隐藏 release 构建的控制台窗口，请把下面这行放在 `main.rs` 的顶部：

```rust
#![cfg_attr(all(feature = "desktop", not(debug_assertions)), windows_subsystem = "windows")]
```

`examples/adisyon` 是一个完整的示例，一个销售终端（POS）应用：

```
cargo run -p zeyn-example-adisyon --features desktop
```

## 构建可执行文件

```
cd my-app
zeyn build-desktop                    # same as --features desktop
zeyn build-desktop --package my-app --features desktop,postgres
```

它运行 `cargo build --release`，启用链接时优化、单个 codegen unit 并剥离符号，然后打印每个可执行文件的路径和大小。速度保持在 `opt-level = 3`。

### 图标、版本信息和清单（Windows）

Windows 从 exe 内部的资源中读取这些内容。添加一个构建脚本，只在桌面构建时嵌入它们：

```toml
[features]
desktop = ["dep:zeyn-desktop", "dep:winresource"]

[build-dependencies]
winresource = { version = "0.1", optional = true, default-features = false }
```

`build.rs`：

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

图标的资源 id 必须为 1（`set_icon` 的默认值）；窗口和任务栏会使用它。版本来自 `package.version`。关于带有按显示器 DPI 感知、UTF-8 和通用控件 6 的清单，请参见 `examples/adisyon/build.rs` 和 `examples/adisyon/assets`。MSVC 工具链会在 Windows SDK 中找到 `rc.exe`；GNU 工具链使用 `windres`。

## WebView2 运行时

Windows 11 和当前的 Windows 10 预装了 WebView2 运行时，并会自行更新（常青运行时）。exe 不捆绑它，从而保持小巧。

对于较旧或受限的机器：

- 随你的安装程序一起提供 Microsoft 的常青引导程序（`MicrosoftEdgeWebview2Setup.exe`，约 2 MB），并在缺少运行时时以 `/silent /install` 运行它。它只会下载一次运行时。
- 对于没有互联网的机器，使用常青独立安装程序（体积大，离线），或放在 exe 旁边的固定版本运行时。固定版本不会自行更新，所以其安全更新由你负责。

没有该运行时，窗口无法打开，`run` 会返回错误。

## 安全模型

服务器只能由应用自己的窗口访问。

- **仅回环地址。** 监听器绑定到 `127.0.0.1` 以及系统选取的端口。它绝不绑定 `0.0.0.0` 或局域网地址。
- **启动令牌。** 每次启动都会生成一个随机的 256 位令牌。窗口首先打开 `/_desktop/start?t=<token>`，它会把令牌设为 `HttpOnly`、`SameSite=Strict` 的 cookie，并重定向到 `/`。其他每个请求都必须在该 cookie 或 `x-zeyn-desktop` 请求头中携带令牌，否则返回 `403`。静态资源同样受到保护。令牌只存在于内存中，并以常量时间比较。
- **Host 和 Origin。** `Host` 请求头必须恰好是 `127.0.0.1:<port>`，这可以阻止 DNS 重绑定（名称解析到 127.0.0.1 的站点会把自己的名称作为 host 发送）。`Origin` 请求头若存在，必须恰好是 `http://127.0.0.1:<port>`。两者适用于每个路径，包括 start 和 focus 端点。
- **其他本地程序**可以连接到该端口，但不知道令牌。其他用户账户无法读取每用户数据文件夹中的实例文件。
- **导航。** 窗口只显示自己的源、它的 `blob:` URL 和 `about:blank`。`http`、`https` 和 `mailto` 链接使用系统默认处理程序打开；其他所有协议（`file:`、自定义协议处理程序、`javascript:`）都会被丢弃。`window.open` 从不创建第二个 WebView。
- **开发者工具**在 debug 构建中开启，在 release 构建中关闭。
- **下载**只允许来自应用的源，并保存到用户的“下载”文件夹，绝不覆盖已有文件（`orders.csv`、`orders (1).csv` 等）。连续多次下载无需提示即可进行。其他权限请求（摄像头、麦克风、剪贴板）会得到 WebView 的常规提示。

该防护不覆盖的情况：

- 以同一用户身份运行的程序可以读取进程内存或附加调试器；没有任何本地应用能防御这一点。
- WebView2 会遵循 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` 环境变量，因此任何能为该用户设置环境变量的人都可以开启远程调试。同一用户，同样受上述限制。
- 应用自己的认证在窗口内部仍然有效。桌面构建中的 `DevAuth` 会让坐在这台机器前的任何人获得完全访问权限。

## 行为

- **单实例。** 第一次启动会把 `instance`（端口和一个单独的 focus 密钥）写入数据文件夹。第二次启动会把该密钥发送到 `/_desktop/focus`；第一个窗口会被置于最前，第二个进程在约 150 毫秒内退出。当两次启动在同一瞬间开始时，两者都可能打开。
- **窗口状态。** 尺寸、位置和最大化标志在关闭时保存到 `window.json`。已不在任何显示器上的已保存位置会被忽略。
- **关闭。** 关闭窗口会释放 WebView，优雅地停止服务器（最多 2 秒）并结束进程。WebView 进程随之结束。
- **数据文件夹。** Windows 上为 `%LOCALAPPDATA%\<id>`，macOS 上为 `~/Library/Application Support/<id>`，Linux 上为 `~/.local/share/<id>`。WebView 配置文件（缓存、本地存储）位于其中的 `webview`，因此该 exe 在只读文件夹（如 Program Files）中也能工作。
- **没有托盘图标。** 未内置。

## 体积与内存

在 Windows 11 上使用 `examples/adisyon`、release 构建、WebView2 运行时 156 测得：

| | |
|---|---|
| `zeyn build-desktop` 生成的 exe | 6.72 MB（使用普通 release 配置为 7.25 MB） |
| 应用进程，空闲 | 4.3 MB 私有内存，25.6 MB 工作集 |
| 应用进程和 WebView2 进程，空闲 | 7 个进程，约 180 MB 私有内存，370 MB 工作集 |
| 同样条件加 `--disable-gpu` | 约 114 MB 私有内存，330 MB 工作集 |
| 第二次启动到其退出 | 约 130 毫秒 |

WebView 占据主要部分。仅其 GPU 进程就占用约 88 MB。对于低内存机器上的简单屏幕，请使用 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu` 启动 exe（例如通过快捷方式或启动脚本），可节省约 65 MB，代价是使用软件渲染。
