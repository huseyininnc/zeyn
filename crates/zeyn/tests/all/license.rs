#![cfg(feature = "signed-licenses")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use ed25519_dalek::{Signer, SigningKey};
use http_body_util::BodyExt;
use std::time::Duration;
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{License, LicenseError, SignedLicenseFile};

const PAYLOAD: &str =
    r#"{"holder":"Acme","valid_until":"2026-12-31","modules":["pos"],"max_users":25}"#;

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn public(key: &SigningKey) -> &'static str {
    Box::leak(hex(key.verifying_key().as_bytes()).into_boxed_str())
}

/// A license file as `zeyn license sign` writes it.
fn signed(key: &SigningKey, payload: &str) -> String {
    let mut message = b"zeyn-license-v1\n".to_vec();
    message.extend_from_slice(payload.as_bytes());
    serde_json::json!({ "payload": payload, "signature": hex(&key.sign(&message).to_bytes()) })
        .to_string()
}

#[test]
fn a_signed_license_is_read_with_the_matching_key() {
    let k = key(7);
    let license = License::from_signed(&signed(&k, PAYLOAD), public(&k)).unwrap();
    assert_eq!(license.holder, "Acme");
    assert_eq!(license.valid_until, Date::new(2026, 12, 31).unwrap());
    assert!(license.allows("pos") && !license.allows("hr"));
    assert_eq!(license.max_users, Some(25));
}

#[test]
fn tampering_and_the_wrong_key_are_refused() {
    let k = key(7);
    let file = signed(&k, PAYLOAD);
    let longer = file.replace("2026-12-31", "2099-12-31");
    assert_eq!(
        License::from_signed(&longer, public(&k)),
        Err(LicenseError::BadSignature)
    );
    let more_users = file.replace("25", "2500");
    assert_eq!(
        License::from_signed(&more_users, public(&k)),
        Err(LicenseError::BadSignature)
    );
    assert_eq!(
        License::from_signed(&file, public(&key(8))),
        Err(LicenseError::BadSignature)
    );
    assert_eq!(
        License::from_signed(&file, "not a key"),
        Err(LicenseError::InvalidPublicKey)
    );
    assert!(matches!(
        License::from_signed("{}", public(&k)),
        Err(LicenseError::Malformed(_))
    ));
    let not_a_license = signed(&k, r#"{"holder":"Acme"}"#);
    assert!(matches!(
        License::from_signed(&not_a_license, public(&k)),
        Err(LicenseError::Malformed(_))
    ));
    let message = LicenseError::BadSignature.to_string();
    assert!(
        message.contains("changed after signing") && message.contains("another key"),
        "{message}"
    );
}

#[screen(route = "/pos", public)]
pub fn pos() -> Screen {
    Screen::new("Point of sale")
}

async fn open(file: &str, public_key: &'static str, today: &str) -> (StatusCode, String) {
    let dir = std::env::temp_dir().join(format!("zeyn-license-{}-{}", std::process::id(), today));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}.json", hex(&file.len().to_le_bytes())));
    std::fs::write(&path, file).unwrap();
    let router = App::new("POS")
        .fixed_now(DateTime::parse(&format!("{today}T10:00:00Z")).unwrap())
        .license(
            SignedLicenseFile::new(path, public_key),
            Duration::from_secs(60),
        )
        .feature(
            Feature::new("pos")
                .guard(Licensed::module("pos"))
                .screen(pos),
        )
        .router()
        .unwrap();
    let res = router
        .oneshot(
            Request::get("/pos")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    (
        status,
        String::from_utf8(
            res.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap(),
    )
}

#[tokio::test]
async fn the_app_is_locked_unless_the_license_is_genuine_and_current() {
    let k = key(7);
    let file = signed(&k, PAYLOAD);
    assert_eq!(
        open(&file, public(&k), "2026-11-02")
            .await
            .0,
        StatusCode::OK
    );

    let (status, body) = open(&file, public(&k), "2027-01-01").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("Your license has expired."), "{body}");

    let (status, body) = open(&file.replace("Acme", "Evil"), public(&k), "2026-11-03").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body.contains("Your license does not cover this."), "{body}");

    assert_eq!(
        open(&file, public(&key(9)), "2026-11-04")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}
