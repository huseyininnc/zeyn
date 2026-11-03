//! Sign in beyond a proxy in front of the app: bearer JWTs for APIs and MCP ([`JwtAuth`], Cargo
//! feature `oidc`), OpenID Connect sign in for browsers ([`OidcAuth`], feature `oidc`), a second
//! factor guard ([`RequireMfa`]) and a built in TOTP second factor ([`Totp`], feature `totp`).
//! See `docs/auth.md`.

use crate::ctx::Ctx;
use crate::feature::{Guard, GuardResult};
use crate::i18n::Text;
use crate::server::{Call, Interceptor};

#[cfg(feature = "oidc")]
pub(crate) mod jwt;
#[cfg(feature = "oidc")]
pub(crate) mod oidc;
#[cfg(any(feature = "oidc", feature = "totp"))]
pub(crate) mod seal;
#[cfg(feature = "totp")]
pub(crate) mod totp;

#[cfg(feature = "oidc")]
pub use jwt::{Algorithm, ClaimMap, JwtAuth};
#[cfg(feature = "oidc")]
pub use oidc::OidcAuth;
#[cfg(feature = "oidc")]
pub(crate) use oidc::{Flow, Session};
#[cfg(feature = "totp")]
pub use totp::{MemoryTotpStore, RECOVERY_CODES, Totp, TotpRecord, TotpStore};

/// Catalog key of the refusal [`RequireMfa`] gives. The server turns it into the `mfa_required`
/// answer that starts a step up sign in.
pub(crate) const MFA_REQUIRED: &str = "zeyn.auth.mfa_required";

/// Requires a second factor (`Identity::mfa`) for a whole feature, for some screens or for some
/// actions. Users without it are sent to sign in again with a second factor (step up): to the
/// identity provider with `acr_values` under [`OidcAuth`], or to the TOTP page under [`Totp`].
///
/// As a feature guard it covers every screen and call of the feature; the screen and action lists
/// are ignored there:
///
/// ```ignore
/// Feature::new("payroll").guard(RequireMfa::new())
/// ```
///
/// As an interceptor it covers the listed screens and actions of the whole app, or every call when
/// both lists are empty:
///
/// ```ignore
/// App::new("Back office").interceptor(RequireMfa::new().screen("bank_accounts").action("approve_payment"))
/// ```
#[derive(Clone, Debug, Default)]
pub struct RequireMfa {
    screens: Vec<&'static str>,
    actions: Vec<&'static str>,
}

impl RequireMfa {
    pub fn new() -> Self {
        Self::default()
    }

    /// A screen (by name) that needs a second factor to open and to call.
    pub fn screen(mut self, name: &'static str) -> Self {
        self.screens.push(name);
        self
    }

    /// An action (by name) that needs a second factor.
    pub fn action(mut self, name: &'static str) -> Self {
        self.actions.push(name);
        self
    }

    fn covers(&self, call: &Call) -> bool {
        (self.screens.is_empty() && self.actions.is_empty())
            || self.screens.contains(&call.screen)
            || call
                .action
                .as_ref()
                .is_some_and(|a| self.actions.contains(&a.name))
    }
}

impl Guard for RequireMfa {
    fn check(&self, ctx: &Ctx) -> GuardResult {
        if ctx.identity().mfa {
            GuardResult::Allow
        } else {
            GuardResult::Deny(Text::key(MFA_REQUIRED))
        }
    }
}

impl Interceptor for RequireMfa {
    fn name(&self) -> &'static str {
        "RequireMfa"
    }

    fn before(&self, ctx: &Ctx, call: &Call) -> crate::Result<()> {
        if !self.covers(call) || ctx.identity().mfa {
            return Ok(());
        }
        Err(crate::Error::Denied(Text::key(MFA_REQUIRED)))
    }
}

/// Whether an `amr` list (RFC 8176) or an `acr` value shows a second factor. `amr` counts when it
/// names `mfa`, a one time code, a hardware or software key, a smart card or a biometric; `acr`
/// counts when it is one of `mfa_acr`.
#[cfg(feature = "oidc")]
pub(crate) fn shows_mfa(amr: &[&str], acr: Option<&str>, mfa_acr: &[String]) -> bool {
    const SECOND: &[&str] = &[
        "mfa", "otp", "hwk", "swk", "sc", "sms", "fpt", "face", "iris", "retina", "vbm", "tel",
    ];
    amr.iter().any(|m| SECOND.contains(m)) || acr.is_some_and(|a| mfa_acr.iter().any(|m| m == a))
}

/// Accepts only a local path as a `return_to` target: it starts with one `/`, has no backslash and
/// no control characters, so a sign in can never send the user to another site.
#[cfg(any(feature = "oidc", feature = "totp"))]
pub(crate) fn safe_return(target: &str) -> Option<&str> {
    let ok = target.starts_with('/')
        && !target.starts_with("//")
        && !target.contains('\\')
        && !target.chars().any(char::is_control)
        && target.len() <= 2048;
    ok.then_some(target)
}

/// Seconds since the Unix epoch.
#[cfg(any(feature = "oidc", feature = "totp"))]
pub(crate) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Random bytes from the operating system.
#[cfg(any(feature = "oidc", feature = "totp"))]
pub(crate) fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    getrandom::getrandom(&mut bytes).expect("the operating system random source must work");
    bytes
}

/// Unpadded base64url (RFC 4648 section 5), as JWTs, PKCE and sealed cookies use it.
#[cfg(any(feature = "oidc", feature = "totp"))]
pub(crate) fn b64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        for i in 0..=chunk.len() {
            out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// Decodes unpadded base64url. Padding, other alphabets and stray bits are refused.
#[cfg(any(feature = "oidc", feature = "totp"))]
pub(crate) fn b64url_decode(s: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        } as u32)
    };
    if s.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.as_bytes().chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= value(*c)? << (18 - 6 * i);
        }
        let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
        let keep = chunk.len() - 1;
        if n & (0xFF_FFFF >> (8 * keep)) != 0 && keep < 3 {
            return None;
        }
        out.extend_from_slice(&bytes[..keep]);
    }
    Some(out)
}
