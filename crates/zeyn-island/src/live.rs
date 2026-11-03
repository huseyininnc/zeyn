//! Live document channel (feature `realtime`): the island side of `/_zeyn/live/doc/{kind}/{doc}`.
//!
//! Binary frames carry Y updates (v1) behind a one byte tag ([`FRAME_UPDATE`]). On every
//! (re)connect the server sends the whole document and the island should send its own state
//! back (`on_open`), so edits made while offline are merged. The channel reconnects after 1 s,
//! doubling up to 30 s, until [`Channel::close`] or a policy close (1008) from the server.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, CloseEvent, MessageEvent, WebSocket};

/// Tag of a frame holding a Y update.
pub const FRAME_UPDATE: u8 = 1;

struct Shared {
    url: String,
    ws: RefCell<Option<WebSocket>>,
    on_open: RefCell<Box<dyn FnMut()>>,
    on_update: RefCell<Box<dyn FnMut(&[u8])>>,
    on_state: RefCell<Box<dyn FnMut(bool)>>,
    closed: Cell<bool>,
    wait: Cell<i32>,
    handlers: RefCell<Vec<Closure<dyn FnMut(JsValue)>>>,
    retry: RefCell<Option<Closure<dyn FnMut()>>>,
}

/// A live connection to one document.
pub struct Channel {
    shared: Rc<Shared>,
}

impl Channel {
    /// Connects to document `doc` of `kind`. `on_open` runs after each (re)connect, once the
    /// server's copy has arrived through `on_update`; `on_state` reports `true` when connected and
    /// `false` when the connection dropped.
    pub fn document(
        kind: &str,
        doc: &str,
        on_open: impl FnMut() + 'static,
        on_update: impl FnMut(&[u8]) + 'static,
        on_state: impl FnMut(bool) + 'static,
    ) -> Result<Channel, JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let meta = |name: &str| {
            document
                .query_selector(&format!("meta[name=\"{name}\"]"))
                .ok()
                .flatten()
                .and_then(|m| m.get_attribute("content"))
                .unwrap_or_default()
        };
        let location = window.location();
        let scheme = if location.protocol()? == "https:" {
            "wss"
        } else {
            "ws"
        };
        let url = format!(
            "{scheme}://{}{}/_zeyn/live/doc/{}/{}?csrf={}",
            location.host()?,
            meta("zeyn-base").trim_end_matches('/'),
            js_sys::encode_uri_component(kind),
            js_sys::encode_uri_component(doc),
            js_sys::encode_uri_component(&meta("zeyn-csrf")),
        );
        let shared = Rc::new(Shared {
            url,
            ws: RefCell::new(None),
            on_open: RefCell::new(Box::new(on_open)),
            on_update: RefCell::new(Box::new(on_update)),
            on_state: RefCell::new(Box::new(on_state)),
            closed: Cell::new(false),
            wait: Cell::new(1000),
            handlers: RefCell::new(Vec::new()),
            retry: RefCell::new(None),
        });
        connect(&shared)?;
        Ok(Channel { shared })
    }

    /// Sends an update. `false` while disconnected; the island's next `on_open` resends its state.
    pub fn send_update(&self, update: &[u8]) -> bool {
        let ws = self.shared.ws.borrow();
        let Some(ws) = ws
            .as_ref()
            .filter(|w| w.ready_state() == WebSocket::OPEN)
        else {
            return false;
        };
        let mut frame = Vec::with_capacity(update.len() + 1);
        frame.push(FRAME_UPDATE);
        frame.extend_from_slice(update);
        ws.send_with_u8_array(&frame).is_ok()
    }

    /// Whether the socket is open.
    pub fn is_open(&self) -> bool {
        self.shared
            .ws
            .borrow()
            .as_ref()
            .is_some_and(|w| w.ready_state() == WebSocket::OPEN)
    }

    /// Closes for good. Call it from the island's `unmount`.
    pub fn close(&self) {
        self.shared.closed.set(true);
        if let Some(ws) = self.shared.ws.borrow_mut().take() {
            detach(&ws);
            let _ = ws.close_with_code(1000);
        }
        self.shared
            .handlers
            .borrow_mut()
            .clear();
        self.shared.retry.borrow_mut().take();
    }
}

fn detach(ws: &WebSocket) {
    ws.set_onopen(None);
    ws.set_onmessage(None);
    ws.set_onclose(None);
}

fn connect(shared: &Rc<Shared>) -> Result<(), JsValue> {
    let ws = WebSocket::new(&shared.url)?;
    ws.set_binary_type(BinaryType::Arraybuffer);
    let weak = Rc::downgrade(shared);
    let opened = handler(&weak, |s, _| {
        s.wait.set(1000);
        (s.on_state.borrow_mut())(true);
    });
    let first = Rc::new(Cell::new(true));
    let message = handler(&weak, move |s, e| {
        let Some(e) = e.dyn_ref::<MessageEvent>() else {
            return;
        };
        let Ok(buf) = e
            .data()
            .dyn_into::<js_sys::ArrayBuffer>()
        else {
            return;
        };
        let bytes = js_sys::Uint8Array::new(&buf).to_vec();
        if bytes.first() == Some(&FRAME_UPDATE) {
            (s.on_update.borrow_mut())(&bytes[1..]);
            if first.replace(false) {
                (s.on_open.borrow_mut())();
            }
        }
    });
    let closed = handler(&weak, |s, e| {
        let code = e
            .dyn_ref::<CloseEvent>()
            .map(|c| c.code())
            .unwrap_or(0);
        (s.on_state.borrow_mut())(false);
        if s.closed.get() || code == 1008 {
            return;
        }
        let wait = s.wait.get();
        s.wait.set((wait * 2).min(30_000));
        let weak = Rc::downgrade(s);
        let retry = Closure::<dyn FnMut()>::new(move || {
            if let Some(s) = weak
                .upgrade()
                .filter(|s| !s.closed.get())
            {
                let _ = connect(&s);
            }
        });
        if let Some(w) = web_sys::window() {
            let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(
                retry.as_ref().unchecked_ref(),
                wait,
            );
        }
        *s.retry.borrow_mut() = Some(retry);
    });
    ws.set_onopen(Some(opened.as_ref().unchecked_ref()));
    ws.set_onmessage(Some(message.as_ref().unchecked_ref()));
    ws.set_onclose(Some(closed.as_ref().unchecked_ref()));
    if let Some(old) = shared.ws.borrow_mut().replace(ws) {
        detach(&old);
    }
    *shared.handlers.borrow_mut() = vec![opened, message, closed];
    Ok(())
}

fn handler(
    weak: &Weak<Shared>,
    mut f: impl FnMut(&Rc<Shared>, JsValue) + 'static,
) -> Closure<dyn FnMut(JsValue)> {
    let weak = weak.clone();
    Closure::new(move |e: JsValue| {
        if let Some(s) = weak.upgrade() {
            f(&s, e);
        }
    })
}
