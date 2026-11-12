//! A Rust client of the page socket (feature `socket-client`): it says hello, calls actions and
//! reads events and topic notifications the way the browser runtime does. Tests use
//! [`TestSocket::serve`] to run an app's router on a local port.

use super::Event;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value as Json;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;

type Stream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The CSRF token the client sends; any 32 byte hex value works, as for a browser.
const TOKEN: &str = "5a7e1d0c9b8a7f6e5d4c3b2a1908f7e6d5c4b3a29180f7e6d5c4b3a291807f6e";

/// One page socket opened from Rust.
///
/// ```ignore
/// let addr = TestSocket::serve(app.router()?).await;
/// let mut c = TestSocket::connect(addr, &[("authorization", "Bearer ...")]).await?;
/// c.call("join_room", json!({ "name": "r1" })).await?;
/// let m: MessageSent = c.next_event().await?;
/// ```
pub struct TestSocket {
    ws: Stream,
    next_call: u64,
    /// Frames that arrived while waiting for something else.
    held: VecDeque<Json>,
    wait: Duration,
}

impl TestSocket {
    /// Serves a router on a free local port and returns its address.
    pub async fn serve(router: axum::Router) -> SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a free local port");
        let addr = listener
            .local_addr()
            .expect("the bound address");
        tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await;
        });
        addr
    }

    /// Opens the page socket of the app at `addr` with extra request headers (how the app's
    /// authenticator knows the user) and says hello.
    pub async fn connect(addr: SocketAddr, headers: &[(&str, &str)]) -> crate::Result<Self> {
        let failed = |e: &dyn std::fmt::Display| crate::Error::Internal(format!("socket: {e}"));
        let mut req = format!("ws://{addr}/_zeyn/socket")
            .into_client_request()
            .map_err(|e| failed(&e))?;
        let h = req.headers_mut();
        let value = |v: &str| HeaderValue::from_str(v).map_err(|e| failed(&e));
        h.insert("origin", value(&format!("http://{addr}"))?);
        h.insert(
            "sec-websocket-protocol",
            value(&format!("zeyn, csrf.{TOKEN}"))?,
        );
        h.insert("cookie", value(&format!("zeyn_csrf={TOKEN}"))?);
        for (name, v) in headers {
            let name =
                tokio_tungstenite::tungstenite::http::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|e| failed(&e))?;
            h.insert(name, value(v)?);
        }
        let (ws, _) = tokio_tungstenite::connect_async(req)
            .await
            .map_err(|e| failed(&e))?;
        let mut socket = TestSocket {
            ws,
            next_call: 1,
            held: VecDeque::new(),
            wait: Duration::from_secs(5),
        };
        socket
            .send(serde_json::json!({ "hello": {} }))
            .await?;
        socket
            .next_matching(|f| f.get("welcome").is_some())
            .await?;
        Ok(socket)
    }

    /// How long reads wait before they fail (five seconds unless set).
    pub fn wait(mut self, wait: Duration) -> Self {
        self.wait = wait;
        self
    }

    /// Subscribes to topics; the server leaves out those the user may not read.
    pub async fn subscribe(&mut self, topics: &[&str]) -> crate::Result<()> {
        self.send(serde_json::json!({ "sub": topics }))
            .await
    }

    /// Calls a published action and returns its answer, or its problem as an error.
    pub async fn call(
        &mut self,
        action: &str,
        input: impl serde::Serialize,
    ) -> crate::Result<Json> {
        let c = self.next_call;
        self.next_call += 1;
        let d = serde_json::to_value(input).map_err(|e| crate::Error::Internal(e.to_string()))?;
        self.send(serde_json::json!({ "c": c, "a": action, "d": d }))
            .await?;
        let answer = self
            .next_matching(|f| f["r"] == c)
            .await?;
        match answer.get("err") {
            Some(problem) => Err(crate::Error::Internal(format!("{action}: {problem}"))),
            None => Ok(answer["ok"].clone()),
        }
    }

    /// The next event of type `E` this connection receives.
    pub async fn next_event<E: Event + serde::de::DeserializeOwned>(&mut self) -> crate::Result<E> {
        let frame = self
            .next_matching(|f| f["e"] == E::NAME)
            .await?;
        serde_json::from_value(frame["d"].clone())
            .map_err(|e| crate::Error::Internal(e.to_string()))
    }

    /// The next topic notification, as the topic name.
    pub async fn next_topic(&mut self) -> crate::Result<String> {
        let frame = self
            .next_matching(|f| f["t"].is_string() && f.get("typing").is_none())
            .await?;
        Ok(frame["t"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    async fn send(&mut self, frame: Json) -> crate::Result<()> {
        self.ws
            .send(Message::Text(frame.to_string().into()))
            .await
            .map_err(|e| crate::Error::Internal(format!("socket: {e}")))
    }

    /// The first held or arriving frame that `wanted` accepts; others are kept for later reads.
    async fn next_matching(&mut self, wanted: impl Fn(&Json) -> bool) -> crate::Result<Json> {
        if let Some(i) = self.held.iter().position(&wanted) {
            return Ok(self.held.remove(i).unwrap_or_default());
        }
        let deadline = tokio::time::Instant::now() + self.wait;
        loop {
            let next = tokio::time::timeout_at(deadline, self.ws.next())
                .await
                .map_err(|_| crate::Error::Internal("socket: nothing arrived in time".into()))?;
            match next {
                Some(Ok(Message::Text(text))) => {
                    let frame: Json = serde_json::from_str(&text).unwrap_or_default();
                    if wanted(&frame) {
                        return Ok(frame);
                    }
                    if self.held.len() < 256 {
                        self.held.push_back(frame);
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(crate::Error::Internal(format!("socket: {e}"))),
                None => return Err(crate::Error::Internal("socket: closed".into())),
            }
        }
    }
}
