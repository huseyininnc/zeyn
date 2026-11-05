//! Client preferences: small values the browser keeps for a component, such as a consent to
//! load embedded content or the tab a user opened last. The server never reads them, so cached
//! public pages stay the same for everyone; preferences that change the HTML (theme, language)
//! stay in cookies.
//!
//! A key is declared once as a constant with its kind, lifetime and purpose, registered on the
//! app with [`crate::App::client_prefs`], and bound to components with `.remember(KEY)`. The
//! browser module `zeyn.state` (`/_zeyn/state.js`) keeps each record under `zeyn.v1.{key}` as
//! `{v, exp, val}`, checks it on every read (shape, length, expiry no further than the lifetime
//! allows) and removes it when it does not pass. Other tabs follow through the `storage` event,
//! and when storage is blocked the values live in memory for the page. Without script the
//! components show their default state.
//!
//! ```
//! use std::time::Duration;
//! use zeyn::prelude::*;
//! use zeyn::{ClientPref, Consent, Purpose};
//!
//! pub const VIDEO_NOTICE: ClientPref<Consent> =
//!     ClientPref::consent("video-notice", Duration::from_secs(86_400), Purpose::Necessary);
//!
//! let app = App::new("Shop").client_prefs(|p| p.key(VIDEO_NOTICE));
//! assert_eq!(app.client_prefs_table()[0].name, "video-notice");
//! ```
//!
//! A component takes only a declared key of its kind; a plain string does not compile:
//!
//! ```compile_fail
//! use zeyn::ui::Embed;
//! use zeyn::Text;
//! let video = Embed::url("https://video.example.com/embed/1")
//!     .title(Text::from("Tour"))
//!     .remember("video-notice");
//! ```
//!
//! A choice key cannot hold a consent either:
//!
//! ```compile_fail
//! use std::time::Duration;
//! use zeyn::{ClientPref, Choice, Purpose, Text};
//! use zeyn::ui::Embed;
//! const TAB: ClientPref<Choice> = ClientPref::choice("tab", Duration::from_secs(60), Purpose::Preference);
//! let video = Embed::url("https://video.example.com/embed/1").title(Text::from("Tour")).remember(TAB);
//! ```
//!
//! A name that is not usable or a lifetime out of bounds fails when the constant is built:
//!
//! ```compile_fail
//! use std::time::Duration;
//! use zeyn::{ClientPref, Consent, Purpose};
//! const BAD: ClientPref<Consent> = ClientPref::consent("no spaces", Duration::from_secs(60), Purpose::Necessary);
//! let _ = BAD;
//! ```

use std::marker::PhantomData;
use std::time::Duration;

/// Longest lifetime of a client preference: 400 days, the longest browsers keep a cookie.
pub const MAX_PREF_LIFETIME: Duration = Duration::from_secs(400 * 24 * 60 * 60);

/// Why a preference is kept, for the privacy page and for consent.
///
/// The ePrivacy Directive (article 5(3)) lets a site store data in the browser without consent
/// only when it is strictly necessary for a service the user asked for. The EU data protection
/// authorities (Article 29 Working Party, opinion 04/2012) count a user's interface choices as
/// necessary only for the session or a few hours; kept longer they need consent. So
/// [`Purpose::Preference`] values are kept for the session until the app's consent hook
/// ([`ClientPrefs::allowed_when`]) says yes, and for their whole lifetime after that.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// Needed for something the user asked for, such as a confirmed notice before embedded
    /// content from another site loads. Kept for its lifetime without asking.
    Necessary,
    /// An interface choice, such as the tab opened last. Kept for the session unless the user
    /// consented to preferences.
    Preference,
}

impl Purpose {
    pub fn code(self) -> &'static str {
        match self {
            Purpose::Necessary => "necessary",
            Purpose::Preference => "preference",
        }
    }
}

/// The kind of a consent key: the user confirmed something, such as a notice.
pub struct Consent;

/// The kind of a choice key: a short text the user picked, such as a tab.
pub struct Choice;

/// What a preference holds, for the privacy table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefKind {
    Consent,
    Choice,
}

/// A declared client preference key of kind `K` ([`Consent`] or [`Choice`]). Build it as a
/// constant; an unusable name (1 to 40 ASCII letters, digits, `-` and `_`) or a lifetime under
/// one second or over [`MAX_PREF_LIFETIME`] does not compile.
pub struct ClientPref<K> {
    info: PrefInfo,
    kind: PhantomData<fn() -> K>,
}

impl<K> Clone for ClientPref<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for ClientPref<K> {}

/// One row of [`crate::App::client_prefs_table`]: a key as the privacy page lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PrefInfo {
    pub name: &'static str,
    pub kind: PrefKind,
    pub purpose: Purpose,
    #[serde(serialize_with = "seconds")]
    pub lifetime: Duration,
}

fn seconds<S: serde::Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_u64(d.as_secs())
}

const fn valid_name(name: &str) -> bool {
    let b = name.as_bytes();
    if b.is_empty() || b.len() > 40 {
        return false;
    }
    let mut i = 0;
    while i < b.len() {
        if !(b[i].is_ascii_alphanumeric() || b[i] == b'-' || b[i] == b'_') {
            return false;
        }
        i += 1;
    }
    true
}

const fn info(
    name: &'static str,
    kind: PrefKind,
    lifetime: Duration,
    purpose: Purpose,
) -> PrefInfo {
    assert!(
        valid_name(name),
        "a client preference name has 1 to 40 ASCII letters, digits, `-` or `_`, such as \"video-notice\""
    );
    assert!(
        lifetime.as_secs() >= 1 && lifetime.as_secs() <= MAX_PREF_LIFETIME.as_secs(),
        "a client preference lives 1 second to 400 days (zeyn::MAX_PREF_LIFETIME), such as Duration::from_secs(86_400)"
    );
    PrefInfo {
        name,
        kind,
        purpose,
        lifetime,
    }
}

impl ClientPref<Consent> {
    /// A consent key, such as a notice the user confirmed once.
    pub const fn consent(name: &'static str, lifetime: Duration, purpose: Purpose) -> Self {
        ClientPref {
            info: info(name, PrefKind::Consent, lifetime, purpose),
            kind: PhantomData,
        }
    }
}

impl ClientPref<Choice> {
    /// A choice key, such as the tab the user opened last.
    pub const fn choice(name: &'static str, lifetime: Duration, purpose: Purpose) -> Self {
        ClientPref {
            info: info(name, PrefKind::Choice, lifetime, purpose),
            kind: PhantomData,
        }
    }
}

impl<K> ClientPref<K> {
    pub fn info(&self) -> PrefInfo {
        self.info
    }

    /// The attributes that bind an element to the key, or nothing (with a warning in the log)
    /// when the app did not register it, so the privacy table always lists what is stored.
    pub(crate) fn attrs(&self, r: &crate::render::Render) -> String {
        if let Some(ctx) = r.ctx
            && !ctx
                .service::<ClientPrefs>()
                .is_some_and(|p| {
                    p.keys
                        .iter()
                        .any(|k| k.name == self.info.name)
                })
        {
            tracing::warn!(
                key = self.info.name,
                "client preference key is not registered with App::client_prefs; it is not remembered"
            );
            return String::new();
        }
        format!(
            r#" data-zeyn-remember="{}" data-zeyn-remember-for="{}" data-zeyn-remember-purpose="{}""#,
            self.info.name,
            self.info.lifetime.as_millis(),
            self.info.purpose.code()
        )
    }
}

#[cfg(feature = "server")]
type ConsentHook = std::sync::Arc<dyn Fn(&axum::http::HeaderMap) -> bool + Send + Sync>;

/// The client preference keys of an app and its consent hook, built with
/// [`crate::App::client_prefs`].
#[derive(Clone, Default)]
pub struct ClientPrefs {
    pub(crate) keys: Vec<PrefInfo>,
    #[cfg(feature = "server")]
    pub(crate) allowed: Option<ConsentHook>,
}

impl crate::ctx::Service for ClientPrefs {}

impl ClientPrefs {
    /// Registers a key.
    pub fn key<K>(mut self, key: ClientPref<K>) -> Self {
        self.keys.push(key.info);
        self
    }

    #[cfg(feature = "server")]
    /// Whether the user consented to keeping [`Purpose::Preference`] values beyond the session,
    /// read from the request (usually the cookie the app's consent banner sets). The browser asks
    /// `GET /_zeyn/prefs` the first time it would keep such a value; the answer is never cached.
    /// Without a hook they are kept for the session only.
    pub fn allowed_when(
        mut self,
        hook: impl Fn(&axum::http::HeaderMap) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.allowed = Some(std::sync::Arc::new(hook));
        self
    }

    /// Names registered twice.
    pub(crate) fn problems(&self) -> Vec<String> {
        self.keys
            .iter()
            .enumerate()
            .filter(|(i, k)| {
                self.keys[..*i]
                    .iter()
                    .any(|o| o.name == k.name)
            })
            .map(|(_, k)| {
                format!(
                    "client preference `{}` is registered twice\n  = help: register each key once with App::client_prefs",
                    k.name
                )
            })
            .collect()
    }
}
