//! Sealed cookie values: AES-256-GCM with a key derived from an app secret, so the browser can
//! hold session state that it can neither read nor change. The cookie name is bound in as
//! associated data, so a value sealed for one cookie never opens as another.

use crate::platform::Secret;
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use ring::hmac;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Format version, the first byte of every sealed value.
const VERSION: u8 = 1;

/// Shortest secret accepted for sealing, in bytes.
pub(crate) const MIN_SECRET: usize = 32;

/// A key for sealing cookie values and for keyed hashes.
#[cfg_attr(not(feature = "totp"), allow(dead_code))]
pub(crate) struct Sealer {
    aead: LessSafeKey,
    mac: hmac::Key,
}

impl Sealer {
    /// Derives the keys from the secret. `None` when the secret is shorter than [`MIN_SECRET`].
    pub(crate) fn new(secret: &Secret<String>) -> Option<Sealer> {
        let secret = secret.expose().as_bytes();
        if secret.len() < MIN_SECRET {
            return None;
        }
        let root = hmac::Key::new(hmac::HMAC_SHA256, secret);
        let derive = |label: &[u8]| hmac::sign(&root, label);
        let aead = LessSafeKey::new(
            UnboundKey::new(&AES_256_GCM, derive(b"zeyn cookie seal v1").as_ref()).ok()?,
        );
        let mac = hmac::Key::new(hmac::HMAC_SHA256, derive(b"zeyn keyed hash v1").as_ref());
        Some(Sealer { aead, mac })
    }

    /// Seals a value for the cookie `name`.
    pub(crate) fn seal<T: Serialize>(&self, name: &str, value: &T) -> String {
        let mut data = serde_json::to_vec(value).unwrap_or_default();
        let nonce: [u8; NONCE_LEN] = super::random();
        if self
            .aead
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(name.as_bytes()),
                &mut data,
            )
            .is_err()
        {
            return String::new();
        }
        let mut out = Vec::with_capacity(1 + NONCE_LEN + data.len());
        out.push(VERSION);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&data);
        super::b64url(&out)
    }

    /// Opens a value sealed for the cookie `name`. `None` when it was changed, sealed with another
    /// key or for another cookie.
    pub(crate) fn open<T: DeserializeOwned>(&self, name: &str, sealed: &str) -> Option<T> {
        if sealed.len() > 8192 {
            return None;
        }
        let bytes = super::b64url_decode(sealed)?;
        let (&version, rest) = bytes.split_first()?;
        if version != VERSION || rest.len() < NONCE_LEN {
            return None;
        }
        let (nonce, data) = rest.split_at(NONCE_LEN);
        let mut data = data.to_vec();
        let nonce = Nonce::try_assume_unique_for_key(nonce).ok()?;
        let plain = self
            .aead
            .open_in_place(nonce, Aad::from(name.as_bytes()), &mut data)
            .ok()?;
        serde_json::from_slice(plain).ok()
    }

    /// Keyed hash (HMAC-SHA256) as hex, for values stored outside the app such as recovery codes:
    /// a copy of the store alone cannot be brute forced without the app secret.
    #[cfg_attr(not(feature = "totp"), allow(dead_code))]
    pub(crate) fn hash(&self, value: &str) -> String {
        hmac::sign(&self.mac, value.as_bytes())
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}
