//! OTLP export end to end: the app serves a request, stops, and a stand in collector receives
//! the spans. Run with `cargo test -p zeyn --features otel --test otel`. Its own test binary,
//! because the app installs the process wide subscriber.
#![cfg(feature = "otel")]

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use zeyn::prelude::*;
use zeyn::{Config, StaticSource};

#[screen(route = "/hello", public)]
pub fn hello() -> Screen {
    Screen::new("Hello")
}

/// Accepts OTLP/HTTP posts and passes each body on.
async fn collector(
    listener: TcpListener,
    bodies: tokio::sync::mpsc::UnboundedSender<(String, Vec<u8>)>,
) {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let bodies = bodies.clone();
        tokio::spawn(async move {
            let mut data = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let Ok(n) = socket.read(&mut buf).await else {
                    return;
                };
                if n == 0 {
                    return;
                }
                data.extend_from_slice(&buf[..n]);
                let Some(end) = data
                    .windows(4)
                    .position(|w| w == b"\r\n\r\n")
                else {
                    continue;
                };
                let head = String::from_utf8_lossy(&data[..end]).to_ascii_lowercase();
                let length: usize = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
                if data.len() >= end + 4 + length {
                    let path = head
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or("")
                        .to_string();
                    let _ = bodies.send((path, data[end + 4..end + 4 + length].to_vec()));
                    let _ = socket
                        .write_all(
                            b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                        )
                        .await;
                    return;
                }
            }
        });
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn spans_reach_an_otlp_collector() {
    let otlp = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap();
    let endpoint = format!("http://{}", otlp.local_addr().unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(collector(otlp, tx));

    let app_port = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let values = [
        ("otel.endpoint", endpoint.as_str()),
        ("otel.service_name", "otel-test"),
        ("log.level", "info"),
    ];
    let config = Config::load(vec![Box::new(StaticSource(
        values
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    ))])
    .await
    .unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let app = App::new("Otel")
        .config(config)
        .feature(Feature::new("hello").screen(hello));
    let server = tokio::spawn(async move {
        app.serve_until(&format!("127.0.0.1:{app_port}"), async {
            let _ = stopped.await;
        })
        .await
    });

    let mut answered = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Ok(mut s) = tokio::net::TcpStream::connect(("127.0.0.1", app_port)).await {
            s.write_all(b"GET /hello?secret=s3cr3t-value HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n")
                .await
                .unwrap();
            let mut reply = Vec::new();
            s.read_to_end(&mut reply).await.unwrap();
            assert!(
                reply.starts_with(b"HTTP/1.1 200"),
                "{}",
                String::from_utf8_lossy(&reply)
            );
            answered = true;
            break;
        }
    }
    assert!(answered, "the app never answered");
    stop.send(()).unwrap();
    server.await.unwrap().unwrap();

    let (path, body) = tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("no spans were exported")
        .unwrap();
    assert_eq!(path, "/v1/traces");
    let has = |needle: &[u8]| {
        body.windows(needle.len())
            .any(|w| w == needle)
    };
    assert!(
        has(b"request") && has(b"/hello") && has(b"otel-test"),
        "{}",
        String::from_utf8_lossy(&body)
    );
    assert!(!has(b"s3cr3t-value"), "the query string reached the trace");
}
