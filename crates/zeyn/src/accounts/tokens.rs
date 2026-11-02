//! Purpose tokens, one time codes, authenticator codes and recovery codes.
//!
//! - **Purpose tokens** (`DataProtectorTokenProvider`): e-mail confirmation, password reset and
//!   e-mail change links. A token is the creation time and an HMAC-SHA256 over the creation time,
//!   the user id, the purpose and the user's security stamp, keyed from the accounts key. Nothing
//!   secret is readable from it. It stops working when it expires (one day unless set), when the
//!   purpose or user differs, and as soon as the security stamp changes, so a password change
//!   kills every outstanding link.
//! - **Short codes** (`TotpSecurityStampBasedTokenProvider`, `EmailTokenProvider`,
//!   `PhoneNumberTokenProvider`): six digit codes for e-mail and phone two factor and phone
//!   number changes, RFC 6238 with three minute steps and two steps of tolerance either way, like
//!   ASP.NET Core Identity. The key is derived from the accounts key and the security stamp.
//! - **Authenticator codes** (`AuthenticatorTokenProvider`): RFC 6238, 30 second steps, two steps
//!   either way, a 160 bit key. A step is accepted once.
//! - **Recovery codes**: eleven characters `XXXXX-XXXXX` from the alphabet ASP.NET Core Identity
//!   uses (no `0 O 1 I L`), stored as keyed hashes only.

use super::random_bytes;
use crate::auth::{b64url, b64url_decode};
use ring::hmac;

/// What a purpose token is for. The names match ASP.NET Core Identity's purposes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// `EmailConfirmation`
    ConfirmEmail,
    /// `ResetPassword`
    ResetPassword,
    /// `ChangeEmail:<new address>`
    ChangeEmail(String),
    /// `ChangePhoneNumber:<new number>`, a short code.
    ChangePhone(String),
    /// `TwoFactor`, a short code sent by e-mail or text message.
    TwoFactor,
}

impl Purpose {
    pub fn name(&self) -> String {
        match self {
            Purpose::ConfirmEmail => "EmailConfirmation".into(),
            Purpose::ResetPassword => "ResetPassword".into(),
            Purpose::ChangeEmail(e) => format!("ChangeEmail:{e}"),
            Purpose::ChangePhone(p) => format!("ChangePhoneNumber:{p}"),
            Purpose::TwoFactor => "TwoFactor".into(),
        }
    }
}

/// Format version, the first byte of a purpose token.
const TOKEN_VERSION: u8 = 1;

/// Keys derived from the accounts key, one per use.
pub(crate) struct Keys {
    token: hmac::Key,
    code: hmac::Key,
    recovery: hmac::Key,
    stamp: hmac::Key,
}

impl Keys {
    pub(crate) fn new(secret: &[u8]) -> Keys {
        let root = hmac::Key::new(hmac::HMAC_SHA256, secret);
        let derive = |label: &[u8]| hmac::Key::new(hmac::HMAC_SHA256, hmac::sign(&root, label).as_ref());
        Keys {
            token: derive(b"zeyn accounts purpose token v1"),
            code: derive(b"zeyn accounts short code v1"),
            recovery: derive(b"zeyn accounts recovery code v1"),
            stamp: derive(b"zeyn accounts stamp check v1"),
        }
    }

    fn token_mac(&self, created: i64, user: &str, purpose: &str, stamp: &str) -> hmac::Tag {
        let mut ctx = hmac::Context::with_key(&self.token);
        ctx.update(&created.to_be_bytes());
        for part in [user, purpose, stamp] {
            ctx.update(&(part.len() as u32).to_be_bytes());
            ctx.update(part.as_bytes());
        }
        ctx.sign()
    }

    /// A purpose token created at `now` (Unix seconds).
    pub(crate) fn token(&self, now: i64, user: &str, purpose: &Purpose, stamp: &str) -> String {
        let mut out = Vec::with_capacity(41);
        out.push(TOKEN_VERSION);
        out.extend_from_slice(&now.to_be_bytes());
        out.extend_from_slice(self.token_mac(now, user, &purpose.name(), stamp).as_ref());
        b64url(&out)
    }

    /// Whether a purpose token is valid for this user, purpose and stamp at `now`. The MAC is
    /// compared in constant time.
    pub(crate) fn check_token(&self, token: &str, now: i64, lifetime: i64, user: &str, purpose: &Purpose, stamp: &str) -> bool {
        if token.len() > 128 {
            return false;
        }
        let Some(bytes) = b64url_decode(token.trim()) else { return false };
        if bytes.len() != 41 || bytes[0] != TOKEN_VERSION {
            return false;
        }
        let created = i64::from_be_bytes(bytes[1..9].try_into().unwrap_or_default());
        if created > now + 60 || created + lifetime < now {
            return false;
        }
        let mut ctx = hmac::Context::with_key(&self.token);
        ctx.update(&created.to_be_bytes());
        for part in [user, purpose.name().as_str(), stamp] {
            ctx.update(&(part.len() as u32).to_be_bytes());
            ctx.update(part.as_bytes());
        }
        let expected = ctx.sign();
        crate::server::same_bytes(expected.as_ref(), &bytes[9..])
    }

    /// The key of short codes for a user: depends on the security stamp, so a new stamp voids
    /// every code sent before.
    fn code_key(&self, stamp: &str) -> Vec<u8> {
        hmac::sign(&self.code, stamp.as_bytes()).as_ref().to_vec()
    }

    /// The six digit short code for `modifier` (such as `Email:TwoFactor:<address>`) at `now`.
    pub(crate) fn short_code(&self, now: i64, stamp: &str, modifier: &str) -> String {
        let step = (now.max(0) as u64) / SHORT_STEP;
        format!("{:06}", totp(&self.code_key(stamp), step, modifier.as_bytes()))
    }

    /// Whether `code` is a short code for `modifier` within two steps of `now`.
    pub(crate) fn check_short_code(&self, code: &str, now: i64, stamp: &str, modifier: &str) -> bool {
        let Some(code) = six_digits(code) else { return false };
        let key = self.code_key(stamp);
        let step = (now.max(0) as u64) / SHORT_STEP;
        let mut hit = false;
        for i in -2i64..=2 {
            let s = (step as i64 + i).max(0) as u64;
            hit |= totp(&key, s, modifier.as_bytes()) == code;
        }
        hit
    }

    /// Keyed hash of a recovery code, normalized first (upper case, no spaces).
    pub(crate) fn recovery_hash(&self, code: &str) -> String {
        let normal: String = code.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_uppercase).collect();
        hex(hmac::sign(&self.recovery, normal.as_bytes()).as_ref())
    }

    /// A short keyed fingerprint of a security stamp, kept in session cookies and bearer tokens so
    /// a changed stamp is noticed without putting the stamp itself in the browser.
    pub(crate) fn stamp_tag(&self, user: &str, stamp: &str) -> String {
        let mut ctx = hmac::Context::with_key(&self.stamp);
        ctx.update(&(user.len() as u32).to_be_bytes());
        ctx.update(user.as_bytes());
        ctx.update(stamp.as_bytes());
        b64url(&ctx.sign().as_ref()[..16])
    }
}

/// Seconds per step of short codes (three minutes, as in ASP.NET Core Identity).
const SHORT_STEP: u64 = 180;
/// Seconds per step of authenticator codes.
pub(crate) const AUTH_STEP: u64 = 30;

fn six_digits(code: &str) -> Option<u32> {
    let code: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    code.parse().ok()
}

/// RFC 4226 HOTP over the time step with an optional modifier, six digits.
fn totp(key: &[u8], step: u64, modifier: &[u8]) -> u32 {
    let k = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, key);
    let mut ctx = hmac::Context::with_key(&k);
    ctx.update(&step.to_be_bytes());
    ctx.update(modifier);
    let hash = ctx.sign();
    let h = hash.as_ref();
    let offset = (h[h.len() - 1] & 0x0f) as usize;
    let binary = (u32::from(h[offset]) & 0x7f) << 24 | u32::from(h[offset + 1]) << 16 | u32::from(h[offset + 2]) << 8 | u32::from(h[offset + 3]);
    binary % 1_000_000
}

/// The authenticator code for `key` at time step `step`.
pub(crate) fn authenticator_code(key: &[u8], step: u64) -> u32 {
    totp(key, step, &[])
}

/// The step within two of `now` that `code` matches, if any, and only when it is later than
/// `last` (a step is never accepted twice).
pub(crate) fn check_authenticator(key: &[u8], code: &str, now: i64, last: u64) -> Option<u64> {
    let code = six_digits(code)?;
    let step = (now.max(0) as u64) / AUTH_STEP;
    let mut found = None;
    for i in -2i64..=2 {
        let s = (step as i64 + i).max(0) as u64;
        if authenticator_code(key, s) == code && s > last && found.is_none() {
            found = Some(s);
        }
    }
    found
}

/// Alphabet of recovery codes: no characters that look alike.
const RECOVERY_ALPHABET: &[u8; 26] = b"23456789BCDFGHJKMNPQRTVWXY";

/// A new recovery code, `XXXXX-XXXXX`, drawn without bias.
pub(crate) fn recovery_code() -> String {
    let mut out = String::with_capacity(11);
    while out.len() < 11 {
        if out.len() == 5 {
            out.push('-');
            continue;
        }
        let [b] = random_bytes::<1>();
        if b < 26 * 9 {
            out.push(RECOVERY_ALPHABET[(b % 26) as usize] as char);
        }
    }
    out
}

/// A new 160 bit authenticator key in base32, as authenticator apps take it.
pub(crate) fn authenticator_key() -> String {
    crate::auth::totp::base32(&random_bytes::<20>())
}

/// Decodes RFC 4648 base32 (upper or lower case, spaces ignored).
pub(crate) fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 5 / 8);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for c in s.chars().filter(|c| !c.is_whitespace() && *c != '=') {
        let v = match c.to_ascii_uppercase() {
            c @ 'A'..='Z' => c as u32 - 'A' as u32,
            c @ '2'..='7' => c as u32 - '2' as u32 + 26,
            _ => return None,
        };
        buffer = (buffer << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc6238_sha1_vectors_match() {
        let key = b"12345678901234567890";
        assert_eq!(authenticator_code(key, 59 / 30), 287082);
        assert_eq!(authenticator_code(key, 1111111109 / 30), 81804);
        assert_eq!(authenticator_code(key, 2000000000 / 30), 279037);
    }

    #[test]
    fn base32_round_trips() {
        let bytes = random_bytes::<20>();
        assert_eq!(base32_decode(&crate::auth::totp::base32(&bytes)).unwrap(), bytes);
    }

    #[test]
    fn recovery_codes_have_the_dotnet_shape() {
        for _ in 0..50 {
            let c = recovery_code();
            assert_eq!(c.len(), 11);
            assert_eq!(&c[5..6], "-");
            assert!(c.chars().filter(|c| *c != '-').all(|c| RECOVERY_ALPHABET.contains(&(c as u8))));
        }
    }
}
