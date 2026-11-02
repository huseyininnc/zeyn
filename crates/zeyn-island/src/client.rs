//! Browser side: reading props and calling remotes with `fetch`.

use crate::{CallError, Remote, call_body, read_error, read_output};
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Element, Headers, RequestCredentials, RequestInit, Response};

/// The root element of a mounted island, and its way to the server.
///
/// ```ignore
/// #[wasm_bindgen]
/// pub fn mount(root: web_sys::Element) {
///     let island = zeyn_island::Island::new(root);
///     let props: CounterProps = island.props().unwrap_or_default();
///     zeyn_island::spawn(async move {
///         let saved = island.call::<SaveCount>(&props.start).await;
///     });
/// }
/// ```
#[derive(Clone)]
pub struct Island {
    root: Element,
}

impl Island {
    pub fn new(root: Element) -> Self {
        Island { root }
    }

    /// The placeholder element the server rendered (`div[data-zeyn-island]`).
    pub fn root(&self) -> &Element {
        &self.root
    }

    /// The props from `data-zeyn-props`.
    pub fn props<P: DeserializeOwned>(&self) -> Result<P, CallError> {
        crate::props(&self.root.get_attribute("data-zeyn-props").unwrap_or_else(|| "null".into()))
    }

    /// Runs the server action bound to `R` on this island's node and returns its output. Uses
    /// the page's CSRF token and session cookie; the server checks everything it checks for any
    /// other call.
    pub async fn call<R: Remote>(&self, input: &R::Input) -> Result<R::Output, CallError> {
        let attr = |el: Option<Element>, name: &str| el.and_then(|e| e.get_attribute(name));
        let screen = self.root.closest("[data-zeyn-screen]").ok().flatten();
        let node = attr(self.root.closest("[data-zeyn-node]").ok().flatten(), "data-zeyn-node").unwrap_or_default();
        let body = call_body::<R>(
            &attr(screen.clone(), "data-zeyn-screen").unwrap_or_default(),
            &attr(screen, "data-zeyn-params").unwrap_or_else(|| "null".into()),
            &node,
            input,
        )?;
        let (status, text) = post("/_zeyn/data", &body).await?;
        if !(200..300).contains(&status) {
            return Err(read_error(status, &text));
        }
        read_output::<R>(&text)
    }
}

fn network(e: JsValue) -> CallError {
    CallError {
        status: 0,
        code: "network".into(),
        message: e.as_string().unwrap_or_else(|| "The server could not be reached.".into()),
    }
}

/// Content of `meta[name=...]` in the page head.
fn meta(doc: &web_sys::Document, name: &str) -> String {
    doc.query_selector(&format!("meta[name=\"{name}\"]"))
        .ok()
        .flatten()
        .and_then(|m| m.get_attribute("content"))
        .unwrap_or_default()
}

/// Posts JSON below the app base with the CSRF header. Returns the status and the body text.
async fn post(path: &str, body: &str) -> Result<(u16, String), CallError> {
    let window = web_sys::window().ok_or_else(|| network(JsValue::NULL))?;
    let doc = window.document().ok_or_else(|| network(JsValue::NULL))?;
    let headers = Headers::new().map_err(network)?;
    headers.set("content-type", "application/json").map_err(network)?;
    headers.set("x-zeyn-csrf", &meta(&doc, "zeyn-csrf")).map_err(network)?;
    let init = RequestInit::new();
    init.set_method("POST");
    init.set_credentials(RequestCredentials::SameOrigin);
    init.set_headers(&headers);
    init.set_body(&JsValue::from_str(body));
    let url = format!("{}{path}", meta(&doc, "zeyn-base").trim_end_matches('/'));
    let res: Response = JsFuture::from(window.fetch_with_str_and_init(&url, &init))
        .await
        .map_err(network)?
        .dyn_into()
        .map_err(network)?;
    let text = JsFuture::from(res.text().map_err(network)?).await.map_err(network)?;
    Ok((res.status(), text.as_string().unwrap_or_default()))
}
