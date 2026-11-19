//! Memory of idle page sockets. Ignored by default; run it with
//! `cargo test -p zeyn --features realtime --release --test realtime_load -- --ignored --nocapture`
//! and `ZEYN_LOAD_CONNECTIONS=10000` for more connections (the default is 2000). It counts every
//! byte the process holds, so the figure covers the server and the test's own clients.

use axum::http::{HeaderMap, HeaderValue};
use futures_util::{SinkExt, StreamExt};
use std::alloc::{GlobalAlloc, Layout, System};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicIsize, Ordering};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use zeyn::prelude::*;
use zeyn::realtime::{Limits, ZeynSocket};

/// Counts the bytes the process holds.
struct Counting;

static HELD: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        HELD.fetch_add(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        HELD.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        HELD.fetch_add(
            new_size as isize - layout.size() as isize,
            Ordering::Relaxed,
        );
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[screen(route = "/", permission = "load.read")]
pub fn home() -> Screen {
    Screen::new("Home")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "load test: run with --ignored --nocapture"]
async fn idle_page_sockets_memory() {
    let n: usize = std::env::var("ZEYN_LOAD_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);
    let live = ZeynSocket::new()
        .topic("orders", "load.read")
        .limits(Limits {
            per_user: usize::MAX,
            heartbeat: std::time::Duration::from_secs(3600),
            ..Limits::default()
        });
    let socket = live.socket();
    let router = App::new("Load")
        .auth(|h: &HeaderMap| {
            Some(Identity::user(h.get("x-user")?.to_str().ok()?).with_permissions(["load.read"]))
        })
        .feature(Feature::new("load").screen(home))
        .socket(live)
        .router()
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap()
    });
    let mut config = WebSocketConfig::default();
    config.read_buffer_size = 256;
    config.write_buffer_size = 0;
    let mut clients = Vec::with_capacity(n);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let before = HELD.load(Ordering::Relaxed);
    for i in 0..n {
        let mut req = format!("ws://{addr}/_zeyn/socket")
            .into_client_request()
            .unwrap();
        let h = req.headers_mut();
        h.insert(
            "origin",
            HeaderValue::from_str(&format!("http://{addr}")).unwrap(),
        );
        h.insert(
            "sec-websocket-protocol",
            HeaderValue::from_str(&format!("zeyn, csrf.{TOKEN}")).unwrap(),
        );
        h.insert(
            "cookie",
            HeaderValue::from_str(&format!("zeyn_csrf={TOKEN}")).unwrap(),
        );
        h.insert(
            "x-user",
            HeaderValue::from_str(&format!("u{}", i % 500)).unwrap(),
        );
        let (mut s, _) = tokio_tungstenite::connect_async_with_config(req, Some(config), false)
            .await
            .unwrap();
        s.send(Message::Text(r#"{"hello":{}}"#.into()))
            .await
            .unwrap();
        let _ = s.next().await;
        s.send(Message::Text(r#"{"sub":["orders"]}"#.into()))
            .await
            .unwrap();
        clients.push(s);
    }
    let mut waited = 0;
    while socket.stats().subscribers < n && waited < 500 {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        waited += 1;
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let after = HELD.load(Ordering::Relaxed);
    let per = (after - before) / n as isize;
    println!(
        "{n} idle page sockets: {} KB held in all, {per} bytes per connection (server and client)",
        (after - before) / 1024
    );
    assert_eq!(socket.stats().connections, n);
    assert!(
        per < 32 * 1024,
        "an idle page socket holds {per} bytes; keep it under 32 KB"
    );
}
