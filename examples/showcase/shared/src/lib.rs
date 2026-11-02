//! Types shared by the showcase server and its WASM islands.
//!
//! The server renders props with them and binds each [`Remote`] to an action of the same input
//! and output types; the islands read the props and call the remotes. A change here that one
//! side does not follow is a compile error, not a broken page.

use serde::{Deserialize, Serialize};
use zeyn_island::Remote;

/// Props of the `counter` island.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CounterProps {
    /// Value saved on the server.
    pub start: i64,
    /// Localized texts, so the island ships no translations.
    pub texts: CounterTexts,
}

/// Texts of the `counter` island.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CounterTexts {
    pub less: String,
    pub more: String,
    pub save: String,
    pub saved: String,
    pub value: String,
}

/// A counter value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Count {
    pub value: i64,
}

/// Saves the counter; the server answers with the value it stored.
pub struct SaveCount;

impl Remote for SaveCount {
    const NAME: &'static str = "save_count";
    type Input = Count;
    type Output = Count;
}
