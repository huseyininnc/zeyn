//! Passkeys (WebAuthn level 3) as a way to sign in and as a second factor, verified on the
//! server with `ring`, which the accounts feature already links.
//!
//! The ceremonies follow `PasskeyHandler` of ASP.NET Core Identity 10: ES256, RS256 and EdDSA
//! keys, user verification required, resident keys preferred, no attestation asked for and
//! attestation statements not verified (the authenticator's key is trusted on first use, as in
//! .NET's default). The challenge (32 random bytes) waits in a sealed cookie for five minutes,
//! so the server keeps no ceremony state. Checked on registration: type, challenge, origin, the
//! relying party id hash, user presence and verification, backup flags, the algorithm, the
//! credential id length and that the credential is new. On sign in also the signature over the
//! authenticator data and the client data hash, the user handle, and that the signature counter
//! grows (a counter that goes back hints at a cloned authenticator and is refused).

use super::model::Passkey;
use crate::auth::{b64url, b64url_decode};
use crate::{Error, Result};
use ring::signature::{self, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::time::Duration;

/// Passkey settings (`IdentityPasskeyOptions`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasskeyOptions {
    /// Passkeys are offered at all. On by default.
    pub enabled: bool,
    /// How long the browser waits for the authenticator. Five minutes.
    pub timeout: Duration,
    /// Challenge size in bytes. 32.
    pub challenge_size: usize,
    /// The relying party id (a domain such as `example.com`). Unset, the host of the accounts
    /// public URL; never the request's `Host` header.
    pub server_domain: Option<String>,
    /// `required` (default), `preferred` or `discouraged`.
    pub user_verification: &'static str,
    /// `preferred` (default), `required` or `discouraged`.
    pub resident_key: &'static str,
}

impl Default for PasskeyOptions {
    fn default() -> Self {
        PasskeyOptions {
            enabled: true,
            timeout: Duration::from_secs(300),
            challenge_size: 32,
            server_domain: None,
            user_verification: "required",
            resident_key: "preferred",
        }
    }
}

/// Name of the cookie that carries a ceremony's challenge.
pub(crate) const CEREMONY: &str = "zeyn_account_passkey";

/// What the ceremony cookie holds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PasskeyState {
    /// `create` or `get`.
    #[serde(rename = "o")]
    pub op: String,
    #[serde(rename = "c")]
    pub challenge: String,
    /// The user adding a passkey, for `create`.
    #[serde(rename = "u", default)]
    pub user: Option<String>,
    #[serde(rename = "e")]
    pub expires: i64,
}

/// COSE algorithm ids offered and accepted: ES256, RS256, EdDSA.
const ALGORITHMS: [i64; 3] = [-7, -257, -8];

/// The relying party: id and the exact origin the browser must report.
pub(crate) struct Party {
    pub id: String,
    pub origin: String,
    pub name: String,
}

impl Party {
    /// From the public URL (`https://host[:port]`), with an optional explicit relying party id
    /// that must be the host or a parent domain of it.
    pub(crate) fn from_url(public_url: &str, domain: Option<&str>, name: &str) -> Option<Party> {
        let rest = public_url.strip_prefix("https://").or_else(|| public_url.strip_prefix("http://"))?;
        let authority = rest.split('/').next()?;
        let host = authority.rsplit_once(':').map(|(h, _)| h).unwrap_or(authority).to_ascii_lowercase();
        if public_url.starts_with("http://") && host != "localhost" {
            return None;
        }
        let id = match domain {
            Some(d) if host == d || host.ends_with(&format!(".{d}")) => d.to_string(),
            Some(_) => return None,
            None => host,
        };
        let origin = public_url.trim_end_matches('/').split('/').take(3).collect::<Vec<_>>().join("/");
        Some(Party { id, origin, name: name.to_string() })
    }
}

/// Creation options for `navigator.credentials.create`, in the JSON form of WebAuthn level 3.
pub(crate) fn creation_options(
    o: &PasskeyOptions,
    party: &Party,
    user_id: &str,
    user_name: &str,
    challenge: &str,
    exclude: &[Passkey],
) -> Json {
    json!({
        "rp": { "id": party.id, "name": party.name },
        "user": { "id": b64url(user_id.as_bytes()), "name": user_name, "displayName": user_name },
        "challenge": challenge,
        "pubKeyCredParams": ALGORITHMS.iter().map(|a| json!({ "type": "public-key", "alg": a })).collect::<Vec<_>>(),
        "timeout": o.timeout.as_millis() as u64,
        "excludeCredentials": exclude.iter().map(|p| json!({ "type": "public-key", "id": b64url(&p.credential_id) })).collect::<Vec<_>>(),
        "authenticatorSelection": { "residentKey": o.resident_key, "requireResidentKey": o.resident_key == "required", "userVerification": o.user_verification },
        "attestation": "none",
    })
}

/// Request options for `navigator.credentials.get`. Empty `allow` lets the browser offer every
/// passkey of the site (discoverable credentials).
pub(crate) fn request_options(o: &PasskeyOptions, party: &Party, challenge: &str, allow: &[Passkey]) -> Json {
    json!({
        "challenge": challenge,
        "timeout": o.timeout.as_millis() as u64,
        "rpId": party.id,
        "allowCredentials": allow.iter().map(|p| json!({ "type": "public-key", "id": b64url(&p.credential_id), "transports": p.transports })).collect::<Vec<_>>(),
        "userVerification": o.user_verification,
    })
}

fn bad(why: &str) -> Error {
    tracing::debug!("passkey refused: {why}");
    super::denied("passkey_invalid")
}

fn field<'a>(v: &'a Json, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    cur.as_str()
}

fn bytes(v: &Json, path: &[&str]) -> Result<Vec<u8>> {
    let s = field(v, path).ok_or_else(|| bad("missing field"))?;
    if s.len() > 64 * 1024 {
        return Err(bad("field too large"));
    }
    b64url_decode(s.trim_end_matches('=')).ok_or_else(|| bad("not base64url"))
}

/// Checks the client data: type, challenge (constant time) and origin.
fn client_data(raw: &[u8], kind: &str, challenge: &str, party: &Party) -> Result<()> {
    let c: Json = serde_json::from_slice(raw).map_err(|_| bad("client data is not JSON"))?;
    if c.get("type").and_then(Json::as_str) != Some(kind) {
        return Err(bad("wrong client data type"));
    }
    let got = c.get("challenge").and_then(Json::as_str).unwrap_or("");
    if !crate::server::same(got.trim_end_matches('='), challenge) {
        return Err(bad("challenge mismatch"));
    }
    if c.get("origin").and_then(Json::as_str) != Some(party.origin.as_str()) {
        return Err(bad("origin mismatch"));
    }
    if c.get("crossOrigin").and_then(Json::as_bool) == Some(true) {
        return Err(bad("cross origin"));
    }
    Ok(())
}

/// Parsed authenticator data.
struct AuthData<'a> {
    flags: u8,
    sign_count: u32,
    attested: Option<(&'a [u8], &'a [u8])>,
}

const UP: u8 = 0x01;
const UV: u8 = 0x04;
const BE: u8 = 0x08;
const BS: u8 = 0x10;
const AT: u8 = 0x40;

fn auth_data<'a>(data: &'a [u8], party: &Party, uv_required: bool) -> Result<AuthData<'a>> {
    if data.len() < 37 {
        return Err(bad("authenticator data too short"));
    }
    let rp_hash = ring::digest::digest(&ring::digest::SHA256, party.id.as_bytes());
    if !crate::server::same_bytes(&data[..32], rp_hash.as_ref()) {
        return Err(bad("relying party id hash mismatch"));
    }
    let flags = data[32];
    if flags & UP == 0 {
        return Err(bad("user not present"));
    }
    if uv_required && flags & UV == 0 {
        return Err(bad("user not verified"));
    }
    if flags & BE == 0 && flags & BS != 0 {
        return Err(bad("backup state without backup eligibility"));
    }
    let sign_count = u32::from_be_bytes([data[33], data[34], data[35], data[36]]);
    let attested = if flags & AT != 0 {
        let rest = &data[37..];
        if rest.len() < 18 {
            return Err(bad("attested data too short"));
        }
        let len = u16::from_be_bytes([rest[16], rest[17]]) as usize;
        if len > 1023 || rest.len() < 18 + len {
            return Err(bad("credential id too long"));
        }
        let id = &rest[18..18 + len];
        let key_bytes = &rest[18 + len..];
        let (_, used) = cbor::item(key_bytes, 0)?;
        Some((id, &key_bytes[..used]))
    } else {
        None
    };
    Ok(AuthData { flags, sign_count, attested })
}

/// A verified new passkey and the user handle the browser was given.
pub(crate) fn verify_registration(
    o: &PasskeyOptions,
    party: &Party,
    state: &PasskeyState,
    credential: &Json,
    name: Option<String>,
    now: i64,
) -> Result<Passkey> {
    client_data(&bytes(credential, &["response", "clientDataJSON"])?, "webauthn.create", &state.challenge, party)?;
    let att = bytes(credential, &["response", "attestationObject"])?;
    let (obj, _) = cbor::item(&att, 0)?;
    let auth = obj.get_text("authData").and_then(cbor::Value::as_bytes).ok_or_else(|| bad("no authData"))?;
    let data = auth_data(auth, party, o.user_verification == "required")?;
    let (id, key) = data.attested.ok_or_else(|| bad("no attested credential"))?;
    let raw_id = bytes(credential, &["rawId"])?;
    if raw_id != id {
        return Err(bad("credential id mismatch"));
    }
    public_key(key)?;
    let transports = credential
        .get("response")
        .and_then(|r| r.get("transports"))
        .and_then(Json::as_array)
        .map(|a| a.iter().filter_map(Json::as_str).filter(|t| t.len() <= 16).take(8).map(str::to_string).collect())
        .unwrap_or_default();
    Ok(Passkey {
        credential_id: id.to_vec(),
        public_key: key.to_vec(),
        name: name.map(|n| n.chars().take(100).collect()),
        created_at: now,
        sign_count: data.sign_count,
        transports,
        user_verified: data.flags & UV != 0,
        backup_eligible: data.flags & BE != 0,
        backed_up: data.flags & BS != 0,
    })
}

/// The credential id of an assertion, to find its passkey.
pub(crate) fn assertion_id(credential: &Json) -> Result<Vec<u8>> {
    let id = bytes(credential, &["rawId"])?;
    if id.is_empty() || id.len() > 1023 {
        return Err(bad("bad credential id"));
    }
    Ok(id)
}

/// Verifies a sign in with `passkey` of the user `user_id`, and returns it with the new counter
/// and backup state.
pub(crate) fn verify_assertion(
    o: &PasskeyOptions,
    party: &Party,
    state: &PasskeyState,
    credential: &Json,
    user_id: &str,
    passkey: &Passkey,
) -> Result<Passkey> {
    if let Some(handle) = field(credential, &["response", "userHandle"]).filter(|h| !h.is_empty()) {
        let handle = b64url_decode(handle.trim_end_matches('=')).ok_or_else(|| bad("bad user handle"))?;
        if handle != user_id.as_bytes() {
            return Err(bad("user handle mismatch"));
        }
    }
    let cdata = bytes(credential, &["response", "clientDataJSON"])?;
    client_data(&cdata, "webauthn.get", &state.challenge, party)?;
    let auth = bytes(credential, &["response", "authenticatorData"])?;
    let data = auth_data(&auth, party, o.user_verification == "required")?;
    if passkey.backup_eligible != (data.flags & BE != 0) {
        return Err(bad("backup eligibility changed"));
    }
    let signature = bytes(credential, &["response", "signature"])?;
    let hash = ring::digest::digest(&ring::digest::SHA256, &cdata);
    let mut signed = auth.clone();
    signed.extend_from_slice(hash.as_ref());
    let (alg, key) = public_key(&passkey.public_key)?;
    UnparsedPublicKey::new(alg, &key).verify(&signed, &signature).map_err(|_| bad("signature"))?;
    if (data.sign_count != 0 || passkey.sign_count != 0) && data.sign_count <= passkey.sign_count {
        return Err(bad("signature counter did not grow"));
    }
    let mut updated = passkey.clone();
    updated.sign_count = data.sign_count;
    updated.backed_up = data.flags & BS != 0;
    Ok(updated)
}

/// The verification algorithm and key bytes of a COSE public key.
fn public_key(cose: &[u8]) -> Result<(&'static dyn signature::VerificationAlgorithm, Vec<u8>)> {
    let (key, _) = cbor::item(cose, 0)?;
    let int = |k: i64| key.get_int(k).and_then(cbor::Value::as_int);
    let bin = |k: i64| key.get_int(k).and_then(cbor::Value::as_bytes);
    let alg = int(3).ok_or_else(|| bad("no alg"))?;
    if !ALGORITHMS.contains(&alg) {
        return Err(bad("algorithm not allowed"));
    }
    match (int(1), alg) {
        (Some(2), -7) => {
            let (x, y) = (bin(-2).ok_or_else(|| bad("no x"))?, bin(-3).ok_or_else(|| bad("no y"))?);
            if int(-1) != Some(1) || x.len() != 32 || y.len() != 32 {
                return Err(bad("not a P-256 key"));
            }
            let mut point = vec![0x04];
            point.extend_from_slice(x);
            point.extend_from_slice(y);
            Ok((&signature::ECDSA_P256_SHA256_ASN1, point))
        }
        (Some(3), -257) => {
            let (n, e) = (bin(-1).ok_or_else(|| bad("no n"))?, bin(-2).ok_or_else(|| bad("no e"))?);
            if n.len() < 256 || n.len() > 1024 || e.is_empty() || e.len() > 8 {
                return Err(bad("RSA key size"));
            }
            Ok((&signature::RSA_PKCS1_2048_8192_SHA256, rsa_der(n, e)))
        }
        (Some(1), -8) => {
            let x = bin(-2).ok_or_else(|| bad("no x"))?;
            if int(-1) != Some(6) || x.len() != 32 {
                return Err(bad("not an Ed25519 key"));
            }
            Ok((&signature::ED25519, x.to_vec()))
        }
        _ => Err(bad("key type and algorithm do not match")),
    }
}

/// An RSA public key as the DER `RSAPublicKey` sequence `ring` reads.
fn rsa_der(n: &[u8], e: &[u8]) -> Vec<u8> {
    fn integer(bytes: &[u8]) -> Vec<u8> {
        let trimmed: &[u8] = {
            let mut b = bytes;
            while b.len() > 1 && b[0] == 0 {
                b = &b[1..];
            }
            b
        };
        let mut body = Vec::with_capacity(trimmed.len() + 1);
        if trimmed[0] & 0x80 != 0 {
            body.push(0);
        }
        body.extend_from_slice(trimmed);
        let mut out = vec![0x02];
        out.extend(length(body.len()));
        out.extend(body);
        out
    }
    fn length(n: usize) -> Vec<u8> {
        if n < 128 {
            vec![n as u8]
        } else if n < 256 {
            vec![0x81, n as u8]
        } else {
            vec![0x82, (n >> 8) as u8, n as u8]
        }
    }
    let mut body = integer(n);
    body.extend(integer(e));
    let mut out = vec![0x30];
    out.extend(length(body.len()));
    out.extend(body);
    out
}

/// The small part of CBOR (RFC 8949) that WebAuthn needs: definite lengths only, at most eight
/// levels deep, nothing larger than the input.
pub(crate) mod cbor {
    use super::{Result, bad};

    #[derive(Debug, Clone, PartialEq)]
    pub enum Value<'a> {
        Int(i64),
        Bytes(&'a [u8]),
        Text(&'a str),
        Array(Vec<Value<'a>>),
        Map(Vec<(Value<'a>, Value<'a>)>),
        Simple(u8),
    }

    impl<'a> Value<'a> {
        pub fn as_int(&self) -> Option<i64> {
            if let Value::Int(n) = self { Some(*n) } else { None }
        }

        pub fn as_bytes(&self) -> Option<&'a [u8]> {
            if let Value::Bytes(b) = self { Some(b) } else { None }
        }

        pub fn get_int(&self, key: i64) -> Option<&Value<'a>> {
            let Value::Map(m) = self else { return None };
            m.iter().find(|(k, _)| *k == Value::Int(key)).map(|(_, v)| v)
        }

        pub fn get_text(&self, key: &str) -> Option<&Value<'a>> {
            let Value::Map(m) = self else { return None };
            m.iter().find(|(k, _)| *k == Value::Text(key)).map(|(_, v)| v)
        }
    }

    fn head(data: &[u8]) -> Result<(u8, u64, usize)> {
        let first = *data.first().ok_or_else(|| bad("cbor: end of input"))?;
        let (major, info) = (first >> 5, first & 0x1f);
        let (arg, used) = match info {
            0..=23 => (u64::from(info), 1),
            24 => (u64::from(*data.get(1).ok_or_else(|| bad("cbor: short"))?), 2),
            25 => (
                u64::from(u16::from_be_bytes(
                    data.get(1..3).ok_or_else(|| bad("cbor: short"))?.try_into().map_err(|_| bad("cbor"))?,
                )),
                3,
            ),
            26 => (
                u64::from(u32::from_be_bytes(
                    data.get(1..5).ok_or_else(|| bad("cbor: short"))?.try_into().map_err(|_| bad("cbor"))?,
                )),
                5,
            ),
            27 => (
                u64::from_be_bytes(
                    data.get(1..9).ok_or_else(|| bad("cbor: short"))?.try_into().map_err(|_| bad("cbor"))?,
                ),
                9,
            ),
            _ => return Err(bad("cbor: indefinite or reserved length")),
        };
        Ok((major, arg, used))
    }

    /// One item at the start of `data` and the bytes it used.
    pub fn item(data: &[u8], depth: u8) -> Result<(Value<'_>, usize)> {
        if depth > 8 {
            return Err(bad("cbor: too deep"));
        }
        let (major, arg, mut used) = head(data)?;
        let take = |used: usize, n: u64| -> Result<&[u8]> {
            let n = usize::try_from(n).map_err(|_| bad("cbor: length"))?;
            data.get(used..used.checked_add(n).ok_or_else(|| bad("cbor: length"))?).ok_or_else(|| bad("cbor: short"))
        };
        let value = match major {
            0 => Value::Int(i64::try_from(arg).map_err(|_| bad("cbor: int"))?),
            1 => Value::Int(-1 - i64::try_from(arg).map_err(|_| bad("cbor: int"))?),
            2 => {
                let b = take(used, arg)?;
                used += b.len();
                Value::Bytes(b)
            }
            3 => {
                let b = take(used, arg)?;
                used += b.len();
                Value::Text(std::str::from_utf8(b).map_err(|_| bad("cbor: text"))?)
            }
            4 | 5 => {
                if arg > data.len() as u64 {
                    return Err(bad("cbor: count"));
                }
                let mut items = Vec::new();
                let mut pairs = Vec::new();
                for _ in 0..arg {
                    let (a, n) = item(&data[used..], depth + 1)?;
                    used += n;
                    if major == 5 {
                        let (b, n) = item(&data[used..], depth + 1)?;
                        used += n;
                        pairs.push((a, b));
                    } else {
                        items.push(a);
                    }
                }
                if major == 5 { Value::Map(pairs) } else { Value::Array(items) }
            }
            7 if arg <= 23 => Value::Simple(arg as u8),
            _ => return Err(bad("cbor: unsupported type")),
        };
        Ok((value, used))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cbor_reads_maps_with_negative_keys() {
        let data = [0xa2, 0x01, 0x02, 0x20, 0x43, 1, 2, 3];
        let (v, used) = cbor::item(&data, 0).unwrap();
        assert_eq!(used, data.len());
        assert_eq!(v.get_int(1).and_then(cbor::Value::as_int), Some(2));
        assert_eq!(v.get_int(-1).and_then(cbor::Value::as_bytes), Some(&[1u8, 2, 3][..]));
    }

    #[test]
    fn cbor_refuses_lengths_beyond_the_input() {
        assert!(cbor::item(&[0x5a, 0xff, 0xff, 0xff, 0xff], 0).is_err());
        assert!(cbor::item(&[0x9f], 0).is_err());
    }

    #[test]
    fn relying_party_comes_from_the_public_url() {
        let p = Party::from_url("https://shop.example.com", None, "Shop").unwrap();
        assert_eq!((p.id.as_str(), p.origin.as_str()), ("shop.example.com", "https://shop.example.com"));
        assert!(Party::from_url("https://shop.example.com", Some("example.com"), "Shop").is_some());
        assert!(Party::from_url("https://shop.example.com", Some("other.com"), "Shop").is_none());
        assert!(Party::from_url("http://shop.example.com", None, "Shop").is_none());
        assert_eq!(Party::from_url("http://localhost:3071", None, "Shop").unwrap().origin, "http://localhost:3071");
    }
}
