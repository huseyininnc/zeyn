//! Runs a Zeyn app as a desktop app.
//!
//! The app's own axum server starts inside the process on `127.0.0.1` and a random free port,
//! and a native window shows it in the system WebView (WebView2 on Windows, WKWebView on
//! macOS, WebKitGTK on Linux). Nothing leaves the machine, so the app works offline.
//!
//! ```no_run
//! # fn app() -> zeyn::App { unimplemented!() }
//! zeyn_desktop::Desktop::new(app()).title("Orders").size(1280, 800).run().unwrap();
//! ```
//!
//! Security: the server binds only to loopback and answers only requests that name its exact
//! host and origin and carry a token made fresh for every launch (see [`guard`]). The window
//! never navigates away from the app; other links open in the system browser. Devtools are
//! off in release builds. See `docs/desktop.md`.

pub mod guard;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Window, WindowBuilder};
use wry::{NewWindowResponse, PermissionKind, PermissionResponse, WebContext, WebView, WebViewBuilder};
use zeyn::App;

/// A Zeyn app in a native window.
pub struct Desktop {
    app: App,
    title: String,
    size: (u32, u32),
    id: Option<String>,
}

#[derive(Debug)]
enum UserEvent {
    Title(String),
    Focus,
    Navigate(String),
}

impl Desktop {
    /// Wraps an app. The window is 1280 x 800 logical pixels and titled after the executable
    /// until the page sets `document.title`.
    pub fn new(app: App) -> Self {
        Self { app, title: String::new(), size: (1280, 800), id: None }
    }

    /// The window title shown before the page sets its own `document.title`.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// The first window size in logical pixels; later launches reuse the last size.
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.size = (width.max(320), height.max(240));
        self
    }

    /// The app id: names the per user data folder (window state, WebView profile) and keeps
    /// one instance per id. Defaults to the executable name.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Starts the server, opens the window and blocks until the window closes, then stops the
    /// server and exits the process. A second launch focuses the running window and returns.
    /// With `ZEYN_DESCRIBE=1` it prints the app description and returns, like `App::serve`.
    pub fn run(self) -> std::io::Result<()> {
        if std::env::var_os("ZEYN_DESCRIBE").is_some() {
            println!("{}", serde_json::to_string_pretty(&self.app.describe()).unwrap_or_default());
            return Ok(());
        }
        let exe = std::env::current_exe().ok().and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()));
        let id = sanitize(self.id.as_deref().or(exe.as_deref()).unwrap_or("zeyn-app"));
        let title = if self.title.is_empty() { id.clone() } else { self.title };
        let dir = dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join(&id);
        std::fs::create_dir_all(&dir)?;
        if focus_running(&dir) {
            return Ok(());
        }

        let listener = bind_loopback()?;
        let port = listener.local_addr()?.port();
        let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
        let proxy = Mutex::new(event_loop.create_proxy());
        let guard = Arc::new(guard::Guard::new(
            port,
            Box::new(move || {
                if let Ok(p) = proxy.lock() {
                    let _ = p.send_event(UserEvent::Focus);
                }
            }),
        )?);

        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let router = {
            let _inside = rt.enter();
            self.app.router().map_err(|problems| std::io::Error::other(problems.join("\n")))?
        };
        let router = router.layer(axum::middleware::from_fn_with_state(guard.clone(), guard::layer));
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let (done_tx, done) = std::sync::mpsc::channel::<()>();
        std::thread::Builder::new().name("zeyn-server".into()).spawn(move || {
            let _ = rt.block_on(async move {
                listener.set_nonblocking(true)?;
                let listener = tokio::net::TcpListener::from_std(listener)?;
                axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>())
                    .with_graceful_shutdown(async move {
                        let _ = stopped.await;
                    })
                    .await
            });
            let _ = done_tx.send(());
        })?;
        let instance = dir.join("instance");
        write_private(&instance, &format!("{port} {}", guard.focus_key()));

        let saved = WindowState::load(&dir.join("window.json"));
        let mut builder = WindowBuilder::new()
            .with_title(&title)
            .with_min_inner_size(LogicalSize::new(480, 360))
            .with_inner_size(LogicalSize::new(self.size.0, self.size.1));
        if let Some(s) = &saved {
            builder = builder.with_inner_size(PhysicalSize::new(s.width, s.height)).with_maximized(s.maximized);
            if event_loop.available_monitors().any(|m| s.inside(m.position(), m.size())) {
                builder = builder.with_position(PhysicalPosition::new(s.x, s.y));
            }
        }
        #[cfg(windows)]
        {
            use tao::platform::windows::IconExtWindows;
            if let Ok(icon) = tao::window::Icon::from_resource(1, None) {
                builder = builder.with_window_icon(Some(icon));
            }
        }
        let window = builder.build(&event_loop).map_err(std::io::Error::other)?;

        let origin = guard.origin().to_string();
        let downloads = dirs::download_dir().unwrap_or_else(|| dir.clone());
        let mut context = WebContext::new(Some(dir.join("webview")));
        let (nav_origin, win_origin, dl_origin) = (origin.clone(), origin.clone(), origin);
        let (title_proxy, nav_proxy) = (event_loop.create_proxy(), event_loop.create_proxy());
        let builder = WebViewBuilder::new_with_web_context(&mut context)
            .with_url(guard.start_url())
            .with_devtools(cfg!(debug_assertions))
            .with_navigation_handler(move |url| {
                let ok = is_internal(&url, &nav_origin);
                if !ok {
                    open_external(&url);
                }
                ok
            })
            .with_new_window_req_handler(move |url, _| {
                if is_internal(&url, &win_origin) {
                    let _ = nav_proxy.send_event(UserEvent::Navigate(url));
                } else {
                    open_external(&url);
                }
                NewWindowResponse::Deny
            })
            .with_download_started_handler(move |url, path| {
                if !is_internal(&url, &dl_origin) {
                    return false;
                }
                if let Some(name) = path.file_name() {
                    *path = free_path(&downloads, &name.to_string_lossy());
                }
                true
            })
            .with_permission_handler(|kind| match kind {
                PermissionKind::AutomaticDownloads => PermissionResponse::Allow,
                _ => PermissionResponse::Default,
            })
            .with_document_title_changed_handler(move |t| {
                let _ = title_proxy.send_event(UserEvent::Title(t));
            });
        let webview = build(builder, &window).map_err(std::io::Error::other)?;

        let state_file = dir.join("window.json");
        let mut held = Some((webview, context, stop));
        event_loop.run(move |event, _, flow| {
            *flow = ControlFlow::Wait;
            match event {
                Event::NewEvents(StartCause::Init) => {}
                Event::UserEvent(UserEvent::Title(t)) => {
                    window.set_title(if t.trim().is_empty() { &title } else { &t })
                }
                Event::UserEvent(UserEvent::Focus) => {
                    window.set_minimized(false);
                    window.set_visible(true);
                    window.set_focus();
                }
                Event::UserEvent(UserEvent::Navigate(url)) => {
                    if let Some((w, _, _)) = &held {
                        let _ = w.load_url(&url);
                    }
                }
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                    WindowState::save(&window, saved.as_ref(), &state_file);
                    let _ = std::fs::remove_file(&instance);
                    if let Some((w, c, stop)) = held.take() {
                        drop(w);
                        drop(c);
                        let _ = stop.send(());
                        let _ = done.recv_timeout(Duration::from_secs(2));
                    }
                    *flow = ControlFlow::Exit;
                }
                _ => {}
            }
        })
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
fn build(builder: WebViewBuilder, window: &Window) -> wry::Result<WebView> {
    builder.build(window)
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
fn build(builder: WebViewBuilder, window: &Window) -> wry::Result<WebView> {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewBuilderExtUnix;
    builder.build_gtk(window.default_vbox().ok_or(wry::Error::UnsupportedWindowHandle)?)
}

/// Binds a listener to `127.0.0.1` on a free port the system picks. Never any other address.
pub fn bind_loopback() -> std::io::Result<TcpListener> {
    TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
}

/// Whether the window may show `url`: the app's own origin, its `blob:` URLs and `about:blank`.
pub fn is_internal(url: &str, origin: &str) -> bool {
    let own = |u: &str| u.strip_prefix(origin).is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '?', '#']));
    url == "about:blank" || own(url) || url.strip_prefix("blob:").is_some_and(own)
}

/// Opens `http`, `https` and `mailto` links in the system's default handler; ignores the rest.
fn open_external(url: &str) {
    if !(url.starts_with("https://") || url.starts_with("http://") || url.starts_with("mailto:")) {
        return;
    }
    #[cfg(windows)]
    let mut cmd = {
        let mut c = std::process::Command::new("rundll32.exe");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    let _ = cmd.arg(url).spawn();
}

/// `dir/name`, or `dir/stem (n).ext` when that file exists already.
fn free_path(dir: &Path, name: &str) -> PathBuf {
    let name =
        Path::new(name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "download".into());
    let first = dir.join(&name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.clone(), String::new()),
    };
    (1..10_000).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|p| !p.exists()).unwrap_or(first)
}

fn sanitize(id: &str) -> String {
    let s: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')).collect();
    if s.trim_matches('.').is_empty() { "zeyn-app".into() } else { s }
}

/// Asks a running instance to focus its window. True when one answered.
fn focus_running(dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(dir.join("instance")) else { return false };
    let Some((port, key)) = text.trim().split_once(' ') else { return false };
    let Ok(port) = port.parse::<u16>() else { return false };
    let Ok(mut s) = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], port)), Duration::from_millis(500))
    else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{}: {key}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        guard::FOCUS,
        guard::FOCUS_HEADER
    );
    let mut status = [0u8; 12];
    s.write_all(req.as_bytes()).is_ok() && s.read_exact(&mut status).is_ok() && &status == b"HTTP/1.1 204"
}

fn write_private(path: &Path, text: &str) {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    if let Ok(mut f) = opts.open(path) {
        let _ = f.write_all(text.as_bytes());
    }
}

/// Window position and size in physical pixels, kept between launches.
#[derive(Clone, Copy, Debug, PartialEq)]
struct WindowState {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    maximized: bool,
}

impl WindowState {
    fn load(path: &Path) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
        let int = |k: &str| v.get(k).and_then(serde_json::Value::as_i64);
        let s = Self {
            x: i32::try_from(int("x")?).ok()?,
            y: i32::try_from(int("y")?).ok()?,
            width: u32::try_from(int("width")?).ok()?.clamp(320, 16_384),
            height: u32::try_from(int("height")?).ok()?.clamp(240, 16_384),
            maximized: v.get("maximized").and_then(serde_json::Value::as_bool).unwrap_or(false),
        };
        Some(s)
    }

    fn save(window: &Window, before: Option<&Self>, path: &Path) {
        let maximized = window.is_maximized();
        let s = match (maximized, before, window.outer_position()) {
            (true, Some(b), _) => Self { maximized, ..*b },
            (_, _, Ok(p)) if !window.is_minimized() => {
                let size = window.inner_size();
                Self { x: p.x, y: p.y, width: size.width, height: size.height, maximized }
            }
            _ => return,
        };
        let json =
            serde_json::json!({ "x": s.x, "y": s.y, "width": s.width, "height": s.height, "maximized": s.maximized });
        let _ = std::fs::write(path, json.to_string());
    }

    /// Whether the window's top left corner lies on a monitor, so it never opens off screen.
    fn inside(&self, at: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> bool {
        let (x, y) = (self.x + 40, self.y + 10);
        x >= at.x
            && y >= at.y
            && i64::from(x) < i64::from(at.x) + i64::from(size.width)
            && i64::from(y) < i64::from(at.y) + i64::from(size.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_ports_are_free_distinct_and_never_public() {
        let a = bind_loopback().unwrap();
        let b = bind_loopback().unwrap();
        let (a, b) = (a.local_addr().unwrap(), b.local_addr().unwrap());
        assert!(a.ip().is_loopback() && b.ip().is_loopback());
        assert_eq!(a.ip().to_string(), "127.0.0.1");
        assert_ne!(a.port(), 0);
        assert_ne!(a.port(), b.port());
    }

    #[test]
    fn only_the_app_origin_stays_in_the_window() {
        let o = "http://127.0.0.1:4321";
        for url in [
            "http://127.0.0.1:4321",
            "http://127.0.0.1:4321/",
            "http://127.0.0.1:4321/orders?x=1",
            "blob:http://127.0.0.1:4321/3f2a",
            "about:blank",
        ] {
            assert!(is_internal(url, o), "{url}");
        }
        for url in [
            "http://127.0.0.1:43210/",
            "http://127.0.0.1:4321.evil.example/",
            "http://127.0.0.1:4321@evil.example/",
            "https://example.com/",
            "file:///C:/Windows",
            "blob:https://example.com/1",
            "data:text/html,x",
            "javascript:alert(1)",
        ] {
            assert!(!is_internal(url, o), "{url}");
        }
    }

    #[test]
    fn downloads_never_overwrite_and_never_leave_the_folder() {
        let dir = std::env::temp_dir().join(format!("zeyn-desktop-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(free_path(&dir, "orders.csv"), dir.join("orders.csv"));
        std::fs::write(dir.join("orders.csv"), "").unwrap();
        assert_eq!(free_path(&dir, "orders.csv"), dir.join("orders (1).csv"));
        assert_eq!(free_path(&dir, "../../evil.exe"), dir.join("evil.exe"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn app_ids_are_safe_folder_names() {
        assert_eq!(sanitize("adisyon"), "adisyon");
        assert_eq!(sanitize("../x/y"), "..xy");
        assert_eq!(sanitize(".."), "zeyn-app");
        assert_eq!(sanitize("a b:c"), "abc");
    }

    #[test]
    fn saved_windows_open_on_a_monitor() {
        let s = WindowState { x: 100, y: 100, width: 800, height: 600, maximized: false };
        assert!(s.inside(PhysicalPosition::new(0, 0), PhysicalSize::new(1920, 1080)));
        assert!(!s.inside(PhysicalPosition::new(1920, 0), PhysicalSize::new(1920, 1080)));
        let off = WindowState { x: -5000, ..s };
        assert!(!off.inside(PhysicalPosition::new(0, 0), PhysicalSize::new(1920, 1080)));
    }
}
