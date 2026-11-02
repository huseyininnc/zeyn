//! `zeyn license keygen` and `zeyn license sign`: the vendor side of signed licenses.
//!
//! The private key stays on the machine that signs. Apps only embed the public key and check
//! files with `zeyn::License::from_signed` or `zeyn::SignedLicenseFile`.

use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use std::path::Path;

/// What a signed license signs: this prefix, then the payload text. Must match the framework.
const CONTEXT: &[u8] = b"zeyn-license-v1\n";

/// Default private key file, next to where the command runs.
pub const DEFAULT_KEY: &str = "zeyn-license.key";

/// Environment variable that may hold the private key as hex instead of a file.
const KEY_ENV: &str = "ZEYN_LICENSE_KEY";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Option<[u8; 32]> {
    let text = text.trim().as_bytes();
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in text.chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(out)
}

/// Makes a new key pair, writes the private key to `out` (never over an existing file) and
/// returns the instructions with the public key to embed in the app.
pub fn keygen(out: &Path) -> Result<String, String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(out).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => format!("{} already exists; refusing to overwrite a private key", out.display()),
        _ => format!("{}: {e}", out.display()),
    })?;
    let mut secret = [0u8; 32];
    getrandom::getrandom(&mut secret).map_err(|e| format!("no system randomness: {e}"))?;
    let key = SigningKey::from_bytes(&secret);
    std::io::Write::write_all(&mut file, (hex(&secret) + "\n").as_bytes()).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(format!(
        "Private key written to {}. Keep it offline: never commit it, never ship it with an app.\n\
         Public key for the app:\n\n    const LICENSE_KEY: &str = \"{}\";\n",
        out.display(),
        hex(key.verifying_key().as_bytes())
    ))
}

/// Signs a license. `license` is a JSON file path or the JSON itself; it needs `holder` and
/// `valid_until` (`YYYY-MM-DD`) and may have `modules` and `max_users`. The key comes from
/// `key_file`, or from `ZEYN_LICENSE_KEY` when no file is given and the variable is set.
pub fn sign(license: &str, key_file: Option<&Path>) -> Result<String, String> {
    let text = if license.trim_start().starts_with('{') {
        license.to_string()
    } else {
        std::fs::read_to_string(license).map_err(|e| format!("{license}: {e}"))?
    };
    let payload: Value = serde_json::from_str(&text).map_err(|e| format!("license is not JSON: {e}"))?;
    check(&payload)?;
    let hex_key = match (key_file, std::env::var(KEY_ENV)) {
        (None, Ok(k)) => k,
        (file, _) => {
            let file = file.unwrap_or(Path::new(DEFAULT_KEY));
            std::fs::read_to_string(file).map_err(|e| format!("private key {}: {e}", file.display()))?
        }
    };
    let key = SigningKey::from_bytes(&from_hex(&hex_key).ok_or("the private key is not 64 hex digits")?);
    let payload = payload.to_string();
    let mut message = CONTEXT.to_vec();
    message.extend_from_slice(payload.as_bytes());
    let signed = json!({ "payload": payload, "signature": hex(&key.sign(&message).to_bytes()) });
    serde_json::to_string_pretty(&signed).map_err(|e| e.to_string())
}

/// The fields the framework needs, so a typo fails here and not in a customer's app.
fn check(payload: &Value) -> Result<(), String> {
    let holder = payload["holder"].as_str().unwrap_or("");
    if holder.trim().is_empty() {
        return Err("license needs a `holder`".into());
    }
    let until = payload["valid_until"].as_str().unwrap_or("");
    let digits = until.bytes().enumerate().all(|(i, b)| if i == 4 || i == 7 { b == b'-' } else { b.is_ascii_digit() });
    if until.len() != 10 || !digits {
        return Err("license needs `valid_until` as YYYY-MM-DD".into());
    }
    if !payload["modules"].is_null() && !payload["modules"].as_array().is_some_and(|m| m.iter().all(Value::is_string)) {
        return Err("`modules` must be a list of module names".into());
    }
    if !payload["max_users"].is_null() && !payload["max_users"].as_u64().is_some_and(|n| n <= u64::from(u32::MAX)) {
        return Err("`max_users` must be a whole number".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keygen_and_sign_make_a_license_the_framework_accepts() {
        let dir = std::env::temp_dir().join(format!("zeyn-cli-license-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key_file = dir.join("vendor.key");
        let _ = std::fs::remove_file(&key_file);
        let out = keygen(&key_file).unwrap();
        assert!(keygen(&key_file).unwrap_err().contains("refusing to overwrite"));
        let public = out.split('"').nth(1).unwrap().to_string();

        let file = sign(r#"{"holder":"Acme","valid_until":"2027-06-30","modules":["pos"]}"#, Some(&key_file)).unwrap();
        let license = zeyn::License::from_signed(&file, &public).unwrap();
        assert_eq!(license.holder, "Acme");
        assert!(license.allows("pos") && !license.allows("hr"));
        assert_eq!(zeyn::License::from_signed(&file.replace("Acme", "Acne"), &public), Err(zeyn::LicenseError::BadSignature));

        assert!(sign(r#"{"valid_until":"2027-06-30"}"#, Some(&key_file)).unwrap_err().contains("holder"));
        assert!(sign(r#"{"holder":"Acme","valid_until":"30.06.2027"}"#, Some(&key_file)).unwrap_err().contains("YYYY-MM-DD"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
