//! Password hashing, migration from ASP.NET Core Identity hashes, the password policy and the
//! optional breached password check.
//!
//! New hashes are Argon2id PHC strings (`$argon2id$v=19$m=19456,t=2,p=1$...`) with the OWASP
//! parameters. ASP.NET Core Identity hashes (`PasswordHasher` of
//! `src/Identity/Extensions.Core/src/PasswordHasher.cs`) are still verified, so users migrated
//! from a .NET app keep their passwords; a successful sign in rehashes them to Argon2id:
//!
//! - **v2**, marker `0x00`: PBKDF2 with HMAC-SHA1, 1000 iterations, 128 bit salt, 256 bit subkey,
//!   `{ 0x00, salt[16], subkey[32] }`.
//! - **v3**, marker `0x01`: PBKDF2 with the PRF, iteration count and salt length in the header,
//!   `{ 0x01, prf (u32), iterations (u32), salt length (u32), salt, subkey }`, all `u32` big
//!   endian, PRF 0 = HMAC-SHA1, 1 = HMAC-SHA256, 2 = HMAC-SHA512. .NET writes SHA512 and 100 000
//!   iterations today, SHA256 and 10 000 in older versions.
//!
//! Both are base64 (standard alphabet, padded) in the store.

use crate::ctx::BoxFuture;
use argon2::password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use ring::pbkdf2;
use std::num::NonZeroU32;
use std::sync::Arc;

/// Outcome of checking a password against a stored hash (`PasswordVerificationResult`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verification {
    Failed,
    Success,
    /// Correct, but the hash is in an older format or with weaker parameters; store a new one.
    SuccessRehashNeeded,
}

/// Longest password accepted, in bytes. Longer input is refused before any hashing, so huge
/// passwords cannot be used to burn CPU.
pub const MAX_PASSWORD_BYTES: usize = 1024;
/// Highest PBKDF2 iteration count accepted from a migrated hash. A stored hash with more is
/// refused rather than run, so a planted record cannot stall the server.
pub const MAX_PBKDF2_ITERATIONS: u32 = 2_000_000;

/// Argon2id parameters (`PasswordHasherOptions`). The defaults are the OWASP recommendation:
/// 19 MiB of memory, two passes, one lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PasswordHasher {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for PasswordHasher {
    fn default() -> Self {
        PasswordHasher { memory_kib: 19 * 1024, iterations: 2, parallelism: 1 }
    }
}

impl PasswordHasher {
    fn argon(&self) -> Option<Argon2<'static>> {
        let params = Params::new(self.memory_kib, self.iterations, self.parallelism, None).ok()?;
        Some(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }

    /// The hash of `password` as a PHC string.
    pub fn hash(&self, password: &str) -> crate::Result<String> {
        let salt = SaltString::encode_b64(&super::random_bytes::<16>())
            .map_err(|e| crate::Error::Internal(format!("salt: {e}")))?;
        let argon = self.argon().ok_or_else(|| crate::Error::Internal("invalid Argon2 parameters".into()))?;
        argon
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| crate::Error::Internal(format!("password hash: {e}")))
    }

    /// Checks `password` against a stored hash in any supported format. Malformed hashes fail.
    pub fn verify(&self, stored: &str, password: &str) -> Verification {
        if password.len() > MAX_PASSWORD_BYTES {
            return Verification::Failed;
        }
        if stored.starts_with("$argon2") {
            return self.verify_argon(stored, password);
        }
        match dotnet::verify(stored, password) {
            true => Verification::SuccessRehashNeeded,
            false => Verification::Failed,
        }
    }

    fn verify_argon(&self, stored: &str, password: &str) -> Verification {
        let Ok(parsed) = PasswordHash::new(stored) else { return Verification::Failed };
        let Ok(params) = Params::try_from(&parsed) else { return Verification::Failed };
        if params.m_cost() > 1024 * 1024 || params.t_cost() > 64 || params.p_cost() > 16 {
            return Verification::Failed;
        }
        let Some(argon) = self.argon() else { return Verification::Failed };
        if argon.verify_password(password.as_bytes(), &parsed).is_err() {
            return Verification::Failed;
        }
        let weaker = parsed.algorithm.as_str() != "argon2id"
            || params.m_cost() < self.memory_kib
            || params.t_cost() < self.iterations
            || params.p_cost() < self.parallelism;
        if weaker { Verification::SuccessRehashNeeded } else { Verification::Success }
    }
}

/// Verification of ASP.NET Core Identity password hashes, for users migrated from .NET apps.
/// Zeyn never writes these formats except in tests.
pub mod dotnet {
    use super::*;

    /// PRF of a v3 hash header.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Prf {
        Sha1 = 0,
        Sha256 = 1,
        Sha512 = 2,
    }

    impl Prf {
        fn algorithm(self) -> pbkdf2::Algorithm {
            match self {
                Prf::Sha1 => pbkdf2::PBKDF2_HMAC_SHA1,
                Prf::Sha256 => pbkdf2::PBKDF2_HMAC_SHA256,
                Prf::Sha512 => pbkdf2::PBKDF2_HMAC_SHA512,
            }
        }
    }

    fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
        Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
    }

    /// Whether `password` matches a v2 or v3 hash. Unknown markers, bad sizes and iteration
    /// counts above [`MAX_PBKDF2_ITERATIONS`] fail.
    pub fn verify(stored: &str, password: &str) -> bool {
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(stored.trim()) else { return false };
        match bytes.first() {
            Some(0x00) => {
                if bytes.len() != 1 + 16 + 32 {
                    return false;
                }
                let iterations = NonZeroU32::new(1000).expect("non zero");
                pbkdf2::verify(pbkdf2::PBKDF2_HMAC_SHA1, iterations, &bytes[1..17], password.as_bytes(), &bytes[17..])
                    .is_ok()
            }
            Some(0x01) => {
                let (Some(prf), Some(iterations), Some(salt_len)) =
                    (read_u32(&bytes, 1), read_u32(&bytes, 5), read_u32(&bytes, 9))
                else {
                    return false;
                };
                let prf = match prf {
                    0 => Prf::Sha1,
                    1 => Prf::Sha256,
                    2 => Prf::Sha512,
                    _ => return false,
                };
                let salt_len = salt_len as usize;
                if salt_len < 16
                    || iterations > MAX_PBKDF2_ITERATIONS
                    || bytes.len() < 13 + salt_len + 16
                    || bytes.len() > 13 + 1024
                {
                    return false;
                }
                let Some(iterations) = NonZeroU32::new(iterations) else { return false };
                let (salt, subkey) = bytes[13..].split_at(salt_len);
                pbkdf2::verify(prf.algorithm(), iterations, salt, password.as_bytes(), subkey).is_ok()
            }
            _ => false,
        }
    }

    /// A v2 hash, exactly as ASP.NET Core Identity in `IdentityV2` mode writes it. For tests and
    /// migration tooling.
    pub fn hash_v2(password: &str, salt: [u8; 16]) -> String {
        let mut subkey = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA1,
            NonZeroU32::new(1000).expect("non zero"),
            &salt,
            password.as_bytes(),
            &mut subkey,
        );
        let mut out = vec![0x00];
        out.extend_from_slice(&salt);
        out.extend_from_slice(&subkey);
        base64::engine::general_purpose::STANDARD.encode(out)
    }

    /// A v3 hash, exactly as ASP.NET Core Identity in `IdentityV3` mode writes it. For tests and
    /// migration tooling.
    pub fn hash_v3(password: &str, prf: Prf, iterations: u32, salt: &[u8]) -> String {
        let mut subkey = [0u8; 32];
        pbkdf2::derive(
            prf.algorithm(),
            NonZeroU32::new(iterations.max(1)).expect("non zero"),
            salt,
            password.as_bytes(),
            &mut subkey,
        );
        let mut out = vec![0x01];
        out.extend_from_slice(&(prf as u32).to_be_bytes());
        out.extend_from_slice(&iterations.to_be_bytes());
        out.extend_from_slice(&(salt.len() as u32).to_be_bytes());
        out.extend_from_slice(salt);
        out.extend_from_slice(&subkey);
        base64::engine::general_purpose::STANDARD.encode(out)
    }
}

/// Password rules (`PasswordOptions`), with the same defaults as ASP.NET Core Identity: at
/// least six characters, one unique character, and a digit, a lower case and an upper case ASCII
/// letter and a character that is neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PasswordOptions {
    pub required_length: usize,
    pub required_unique_chars: usize,
    pub require_non_alphanumeric: bool,
    pub require_lowercase: bool,
    pub require_uppercase: bool,
    pub require_digit: bool,
}

impl Default for PasswordOptions {
    fn default() -> Self {
        PasswordOptions {
            required_length: 6,
            required_unique_chars: 1,
            require_non_alphanumeric: true,
            require_lowercase: true,
            require_uppercase: true,
            require_digit: true,
        }
    }
}

impl PasswordOptions {
    /// The rules `password` breaks, as error codes (`password_too_short`, ...), in the order
    /// ASP.NET Core Identity's `PasswordValidator` reports them. Lengths count characters.
    pub fn check(&self, password: &str) -> Vec<(&'static str, Option<usize>)> {
        let mut errors = Vec::new();
        let chars = password.chars().count();
        if password.trim().is_empty() || chars < self.required_length {
            errors.push(("password_too_short", Some(self.required_length)));
        }
        if password.len() > MAX_PASSWORD_BYTES {
            errors.push(("password_too_long", Some(MAX_PASSWORD_BYTES)));
        }
        if self.require_non_alphanumeric && password.chars().all(|c| c.is_ascii_alphanumeric()) {
            errors.push(("password_requires_non_alphanumeric", None));
        }
        if self.require_digit && !password.chars().any(|c| c.is_ascii_digit()) {
            errors.push(("password_requires_digit", None));
        }
        if self.require_lowercase && !password.chars().any(|c| c.is_ascii_lowercase()) {
            errors.push(("password_requires_lower", None));
        }
        if self.require_uppercase && !password.chars().any(|c| c.is_ascii_uppercase()) {
            errors.push(("password_requires_upper", None));
        }
        if self.required_unique_chars >= 1 {
            let mut seen: Vec<char> = password.chars().collect();
            seen.sort_unstable();
            seen.dedup();
            if seen.len() < self.required_unique_chars {
                errors.push(("password_requires_unique_chars", Some(self.required_unique_chars)));
            }
        }
        errors
    }
}

/// An optional check against known breached passwords, such as a k-anonymity range query to a
/// breach corpus or an offline list. None is set by default, so nothing leaves the server unless
/// the app adds one. A check that fails (`Err`) does not block the user.
pub trait BreachedPasswords: Send + Sync + 'static {
    fn is_breached(&self, password: &str) -> BoxFuture<crate::Result<bool>>;
}

/// A fixed list of passwords to refuse, for tests and small offline lists. Compared case
/// sensitively.
#[derive(Clone, Default)]
pub struct PasswordDenyList(Arc<std::collections::HashSet<String>>);

impl PasswordDenyList {
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(passwords: I) -> Self {
        PasswordDenyList(Arc::new(passwords.into_iter().map(Into::into).collect()))
    }
}

impl BreachedPasswords for PasswordDenyList {
    fn is_breached(&self, password: &str) -> BoxFuture<crate::Result<bool>> {
        let hit = self.0.contains(password);
        Box::pin(async move { Ok(hit) })
    }
}
