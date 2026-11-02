//! Client side of Zeyn islands.
//!
//! An island is a Rust component compiled to WebAssembly that the server places on a page with
//! `zeyn::ui::Island`. The server renders a placeholder with fallback content and the props as
//! JSON; `/_zeyn/islands.js` loads the island when it scrolls into view (or on first
//! interaction) and calls its exported `mount(root)`.
//!
//! The crate has two halves:
//!
//! - Always: [`Remote`], the contract of a server action an island may call, and [`CallError`].
//!   A crate shared by the server and the island declares its props, messages and remotes with
//!   these, and the server compiles none of the browser code.
//! - With the `client` feature: [`Island`], which reads the props and calls remotes with `fetch`
//!   through `POST /_zeyn/data`, with the page's CSRF token. The server runs the same checks as
//!   for every other component: CSRF, the screen's access, the feature's guards, the action's
//!   permission and the interceptors. An island can only reach the actions its `Island` node
//!   binds on the server.
//!
//! ```
//! use serde::{Deserialize, Serialize};
//! use zeyn_island::Remote;
//!
//! /// What the island sends.
//! #[derive(Serialize, Deserialize)]
//! pub struct Note { pub text: String }
//!
//! /// Saves a note. The server binds it with `Island::new("notes").action::<SaveNote, _>(save_note)`.
//! pub struct SaveNote;
//!
//! impl Remote for SaveNote {
//!     const NAME: &'static str = "save_note";
//!     type Input = Note;
//!     type Output = u32;
//! }
//!
//! let body = zeyn_island::call_body::<SaveNote>("notes", "null", "0", &Note { text: "Hi".into() }).unwrap();
//! assert_eq!(body, r#"{"screen":"notes","params":null,"node":"0","key":"save_note","input":{"text":"Hi"}}"#);
//! ```

use serde::de::{DeserializeOwned, IgnoredAny};
use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
pub use client::Island;
#[cfg(feature = "client")]
pub use wasm_bindgen_futures::spawn_local as spawn;
#[cfg(feature = "canvas")]
pub mod canvas;

/// A server action an island may call, declared once in a crate both sides share.
///
/// The server binds it to an action with the same input and output types, so a mismatch is a
/// compile error on the server, not a runtime surprise in the browser.
pub trait Remote: 'static {
    /// Key the island sends; unique among the remotes of one island.
    const NAME: &'static str;
    type Input: Serialize + DeserializeOwned + Send + 'static;
    type Output: Serialize + DeserializeOwned + Send + 'static;
}

/// Why a call or a props read failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallError {
    /// HTTP status; `0` when the request never got an answer or the island could not read it.
    pub status: u16,
    /// Machine readable code from the server (`forbidden`, `not_found`, `conflict`, `validation`,
    /// `timeout`, `internal`), or `network`, `props`, `input`, `answer` for local failures.
    pub code: String,
    /// Message for the user, already localized by the server.
    pub message: String,
}

impl CallError {
    fn local(code: &str, message: impl fmt::Display) -> Self {
        CallError { status: 0, code: code.to_string(), message: message.to_string() }
    }

    /// The server could not be reached: offline, aborted, or blocked.
    pub fn is_network(&self) -> bool {
        self.status == 0 && self.code == "network"
    }
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CallError {}

/// Reads the props the server rendered for an island.
///
/// ```
/// #[derive(Debug, serde::Deserialize)]
/// struct Props { start: i32 }
/// let p: Props = zeyn_island::props(r#"{"start":3}"#).unwrap();
/// assert_eq!(p.start, 3);
/// assert_eq!(zeyn_island::props::<Props>("{").unwrap_err().code, "props");
/// ```
pub fn props<P: DeserializeOwned>(json: &str) -> Result<P, CallError> {
    serde_json::from_str(json).map_err(|e| CallError::local("props", e))
}

/// Body of `POST /_zeyn/data` for a remote: the island's screen, params and node, the remote's
/// key and the input. `params` is the screen's `data-zeyn-params` JSON; anything that is not JSON
/// is sent as `null` (the server then refuses the call if the screen needs params).
pub fn call_body<R: Remote>(screen: &str, params: &str, node: &str, input: &R::Input) -> Result<String, CallError> {
    let params = if serde_json::from_str::<IgnoredAny>(params).is_ok() { params } else { "null" };
    let input = serde_json::to_string(input).map_err(|e| CallError::local("input", e))?;
    let text = |s: &str| serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    Ok(format!(
        r#"{{"screen":{},"params":{params},"node":{},"key":{},"input":{input}}}"#,
        text(screen),
        text(node),
        text(R::NAME)
    ))
}

/// Reads an error answer of the server (`{ "error": "...", "message": "..." }`).
///
/// ```
/// let e = zeyn_island::read_error(403, r#"{"error":"forbidden","message":"Not allowed."}"#);
/// assert_eq!((e.status, e.code.as_str(), e.message.as_str()), (403, "forbidden", "Not allowed."));
/// assert_eq!(zeyn_island::read_error(502, "<html>").code, "http");
/// ```
pub fn read_error(status: u16, body: &str) -> CallError {
    #[derive(Deserialize)]
    struct Problem {
        #[serde(default)]
        error: String,
        #[serde(default)]
        message: String,
    }
    match serde_json::from_str::<Problem>(body) {
        Ok(p) => CallError { status, code: p.error, message: p.message },
        Err(_) => CallError { status, code: "http".into(), message: format!("The server answered {status}.") },
    }
}

/// Reads a successful answer as the remote's output.
pub fn read_output<R: Remote>(body: &str) -> Result<R::Output, CallError> {
    serde_json::from_str(body).map_err(|e| CallError::local("answer", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;
    impl Remote for Echo {
        const NAME: &'static str = "echo \"x\"";
        type Input = Vec<u8>;
        type Output = Vec<u8>;
    }

    #[test]
    fn call_body_escapes_names_and_drops_broken_params() {
        let body = call_body::<Echo>("a\"b", "{broken", "1.2", &vec![1, 2]).unwrap();
        assert_eq!(body, r#"{"screen":"a\"b","params":null,"node":"1.2","key":"echo \"x\"","input":[1,2]}"#);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["input"][1], 2);
    }

    #[test]
    fn read_output_reports_answers_it_cannot_read() {
        assert_eq!(read_output::<Echo>("[3]").unwrap(), vec![3]);
        assert_eq!(read_output::<Echo>("{}").unwrap_err().code, "answer");
    }
}
