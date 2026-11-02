//! Built in TOTP second factor (RFC 6238) for apps without an identity provider that does MFA:
//! enrollment with a QR code drawn as SVG on the server, verification with a rate limit and replay
//! protection, and recovery codes stored as keyed hashes.

use super::seal::Sealer;
use crate::ctx::{BoxFuture, Identity};
use crate::platform::Secret;
use crate::security::RateLimit;
use crate::time::DateTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How many recovery codes a user gets at enrollment.
pub const RECOVERY_CODES: usize = 10;
/// Name of the cookie that proves a verified second factor.
pub(crate) const PROOF: &str = "zeyn_mfa";
/// Name of the cookie that carries a secret being enrolled.
pub(crate) const ENROLL: &str = "zeyn_totp_enroll";
/// How long an enrollment may take, in seconds.
pub(crate) const ENROLL_TTL: i64 = 600;

/// A user's second factor as the store keeps it. `secret` is sealed with the app's key, so the
/// store alone cannot produce codes; `recovery` holds keyed hashes of the unused recovery codes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotpRecord {
    pub secret: String,
    pub recovery: Vec<String>,
    /// Last time step accepted; a code is never accepted twice.
    pub last_step: u64,
}

/// Where second factor records live, usually a table keyed by user.
pub trait TotpStore: Send + Sync + 'static {
    fn load(&self, user: &str) -> BoxFuture<crate::Result<Option<TotpRecord>>>;
    fn save(&self, user: &str, record: TotpRecord) -> BoxFuture<crate::Result<()>>;
}

/// Records in memory. For tests and development; they are gone when the process ends.
#[derive(Clone, Default)]
pub struct MemoryTotpStore(Arc<Mutex<HashMap<String, TotpRecord>>>);

impl TotpStore for MemoryTotpStore {
    fn load(&self, user: &str) -> BoxFuture<crate::Result<Option<TotpRecord>>> {
        let found = self.0.lock().ok().and_then(|m| m.get(user).cloned());
        Box::pin(async move { Ok(found) })
    }

    fn save(&self, user: &str, record: TotpRecord) -> BoxFuture<crate::Result<()>> {
        if let Ok(mut m) = self.0.lock() {
            m.insert(user.to_string(), record);
        }
        Box::pin(async { Ok(()) })
    }
}

/// What the proof cookie holds.
#[derive(Serialize, Deserialize)]
pub(crate) struct Proof {
    #[serde(rename = "u")]
    pub user: String,
    #[serde(rename = "a")]
    pub at: i64,
    #[serde(rename = "e")]
    pub expires: i64,
}

/// What the enrollment cookie holds.
#[derive(Serialize, Deserialize)]
pub(crate) struct Pending {
    #[serde(rename = "u")]
    pub user: String,
    #[serde(rename = "s")]
    pub secret: String,
    #[serde(rename = "e")]
    pub expires: i64,
}

/// Built in TOTP second factor. Users enroll at `/_zeyn/auth/totp` with any authenticator app,
/// and verify there when a [`super::RequireMfa`] guard or a `reauth` action asks for it. A verified
/// code marks the identity with `mfa` and counts as a fresh sign in for `reauth` actions.
///
/// ```ignore
/// App::new("Back office")
///     .auth(ForwardAuth::new("x-forwarded-user"))
///     .totp(Totp::new(TotpTable::new(pool), config.secret("auth.totp_key")?).issuer("Back office"))
/// ```
pub struct Totp {
    pub(crate) store: Arc<dyn TotpStore>,
    pub(crate) sealer: Option<Sealer>,
    pub(crate) issuer: String,
    pub(crate) lifetime: Duration,
    pub(crate) attempts: RateLimit,
}

impl Totp {
    /// Records in `store`; `key` seals the stored secrets and the cookies and keys the recovery
    /// code hashes. It must be at least 32 bytes, from configuration, and must not change: records
    /// sealed with another key no longer open.
    pub fn new(store: impl TotpStore, key: Secret<String>) -> Self {
        Totp {
            store: Arc::new(store),
            sealer: Sealer::new(&key),
            issuer: "Zeyn".into(),
            lifetime: Duration::from_secs(12 * 3600),
            attempts: RateLimit::per_user(5, Duration::from_secs(300)),
        }
    }

    /// Name shown in the authenticator app.
    pub fn issuer(mut self, name: &str) -> Self {
        self.issuer = name.replace(':', " ");
        self
    }

    /// How long a verified code counts as a second factor; twelve hours unless set.
    pub fn lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = lifetime;
        self
    }

    /// Most verification attempts per user in a window; five per five minutes unless set.
    pub fn attempts(mut self, attempts: u32, window: Duration) -> Self {
        self.attempts = RateLimit::per_user(attempts, window);
        self
    }

    pub(crate) fn problems(&self) -> Vec<String> {
        if self.sealer.is_none() {
            vec![format!("Totp: the key must be at least {} bytes", super::seal::MIN_SECRET)]
        } else {
            Vec::new()
        }
    }

    /// Marks the identity with `mfa` when the request carries a valid proof for the same user, and
    /// moves its sign in time to the proof's time when that is later.
    pub(crate) fn apply(&self, proof: Option<&str>, identity: &mut Identity) {
        let (Some(sealer), Some(value), Some(user)) = (&self.sealer, proof, identity.user.as_deref()) else { return };
        let Some(p) = sealer.open::<Proof>(PROOF, value) else { return };
        if p.user != user || p.expires <= super::now_secs() {
            return;
        }
        identity.mfa = true;
        let at = DateTime::from_unix_ms(p.at.saturating_mul(1000));
        if identity.authenticated_at.is_none_or(|old| old.unix_ms() < at.unix_ms()) {
            identity.authenticated_at = Some(at);
        }
    }

    /// The TOTP generator for a raw secret.
    pub(crate) fn generator(&self, secret: Vec<u8>, user: &str) -> Option<totp_rs::Totp> {
        totp_rs::Builder::new()
            .with_secret(secret)
            .with_account_name(user.replace(':', " "))
            .with_issuer(Some(self.issuer.clone()))
            .build()
            .ok()
    }

    /// The step a code matches now (one step of clock skew either way), if any.
    pub(crate) fn check(&self, secret: Vec<u8>, user: &str, code: &str) -> Option<u64> {
        let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        self.generator(secret, user)?.check(&code, super::now_secs().max(0) as u64)
    }

    /// New recovery codes in the form `xxxxx-xxxxx`, and their keyed hashes for the store.
    pub(crate) fn recovery_codes(&self) -> Option<(Vec<String>, Vec<String>)> {
        let sealer = self.sealer.as_ref()?;
        let codes: Vec<String> = (0..RECOVERY_CODES)
            .map(|_| {
                let b = base32(&super::random::<7>()).to_lowercase();
                format!("{}-{}", &b[..5], &b[5..10])
            })
            .collect();
        let hashes = codes.iter().map(|c| sealer.hash(c)).collect();
        Some((codes, hashes))
    }

    /// Normalizes a typed recovery code: lower case, without spaces.
    pub(crate) fn recovery_hash(&self, code: &str) -> Option<String> {
        let code: String = code.trim().to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
        (code.len() == 11).then(|| self.sealer.as_ref().map(|s| s.hash(&code))).flatten()
    }
}

/// RFC 4648 base32 without padding, as authenticator apps expect secrets typed by hand.
pub(crate) fn base32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &b in bytes {
        buffer = (buffer << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// A QR code of `text` as an SVG image with its quiet zone, black on white in every theme so
/// cameras read it.
pub(crate) fn qr_svg(text: &str, label: &str) -> Option<String> {
    let qr = qrcodegen::QrCode::encode_text(text, qrcodegen::QrCodeEcc::Medium).ok()?;
    let size = qr.size();
    let mut path = String::with_capacity((size * size) as usize * 4);
    for y in 0..size {
        for x in 0..size {
            if qr.get_module(x, y) {
                path.push_str(&format!("M{},{}h1v1h-1z", x + 4, y + 4));
            }
        }
    }
    let full = size + 8;
    Some(format!(
        r##"<svg class="z-totp-qr" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {full} {full}" width="220" height="220" role="img" aria-label="{}" shape-rendering="crispEdges"><rect width="{full}" height="{full}" fill="#fff"/><path d="{path}" fill="#000"/></svg>"##,
        crate::render::escape(label)
    ))
}
