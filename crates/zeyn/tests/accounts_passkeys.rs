//! Passkeys end to end against a software authenticator made in the test: registration with
//! attestation `none`, sign in with an ES256 signature, and the refusals for a reused challenge, a
//! counter that does not grow, a changed signature and a wrong origin.

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use base64::Engine;
use http_body_util::BodyExt;
use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_ASN1_SIGNING, EcdsaKeyPair, KeyPair};
use serde_json::{Value as Json, json};
use tower::ServiceExt;
use zeyn::accounts::{self, Accounts, MemoryAccountStore, PasswordHasher, User};
use zeyn::prelude::*;

const KEY: &str = "an accounts key for tests only, not used anywhere real";
const CSRF: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const ORIGIN: &str = "http://localhost:3071";

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn unb64(s: &str) -> Vec<u8> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s).unwrap()
}

/// CBOR head for a major type and length.
fn head(major: u8, n: usize, out: &mut Vec<u8>) {
    if n < 24 {
        out.push(major << 5 | n as u8);
    } else if n < 256 {
        out.extend([major << 5 | 24, n as u8]);
    } else {
        out.extend([major << 5 | 25, (n >> 8) as u8, n as u8]);
    }
}

fn cbor_bytes(b: &[u8], out: &mut Vec<u8>) {
    head(2, b.len(), out);
    out.extend_from_slice(b);
}

fn cbor_text(t: &str, out: &mut Vec<u8>) {
    head(3, t.len(), out);
    out.extend_from_slice(t.as_bytes());
}

struct Authenticator {
    key: EcdsaKeyPair,
    id: Vec<u8>,
    count: u32,
}

impl Authenticator {
    fn new() -> Self {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
        Authenticator { key: EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng).unwrap(), id: vec![7; 32], count: 0 }
    }

    fn cose(&self) -> Vec<u8> {
        let p = self.key.public_key().as_ref();
        let mut out = vec![0xa5, 0x01, 0x02, 0x03, 0x26, 0x20, 0x01, 0x21];
        cbor_bytes(&p[1..33], &mut out);
        out.push(0x22);
        cbor_bytes(&p[33..65], &mut out);
        out
    }

    fn client_data(kind: &str, challenge: &str, origin: &str) -> Vec<u8> {
        json!({ "type": kind, "challenge": challenge, "origin": origin, "crossOrigin": false }).to_string().into_bytes()
    }

    fn register(&self, options: &Json) -> Json {
        let rp = options["rp"]["id"].as_str().unwrap();
        let mut auth = ring::digest::digest(&ring::digest::SHA256, rp.as_bytes()).as_ref().to_vec();
        auth.push(0x45);
        auth.extend(0u32.to_be_bytes());
        auth.extend([0u8; 16]);
        auth.extend((self.id.len() as u16).to_be_bytes());
        auth.extend(&self.id);
        auth.extend(self.cose());
        let mut att = vec![0xa3];
        cbor_text("fmt", &mut att);
        cbor_text("none", &mut att);
        cbor_text("attStmt", &mut att);
        att.push(0xa0);
        cbor_text("authData", &mut att);
        cbor_bytes(&auth, &mut att);
        let cd = Self::client_data("webauthn.create", options["challenge"].as_str().unwrap(), ORIGIN);
        json!({ "id": b64(&self.id), "rawId": b64(&self.id), "type": "public-key", "response": { "clientDataJSON": b64(&cd), "attestationObject": b64(&att), "transports": ["internal"] } })
    }

    fn assert(&mut self, options: &Json, origin: &str) -> Json {
        self.count += 1;
        let mut auth = ring::digest::digest(&ring::digest::SHA256, options["rpId"].as_str().unwrap().as_bytes()).as_ref().to_vec();
        auth.push(0x05);
        auth.extend(self.count.to_be_bytes());
        let cd = Self::client_data("webauthn.get", options["challenge"].as_str().unwrap(), origin);
        let mut signed = auth.clone();
        signed.extend(ring::digest::digest(&ring::digest::SHA256, &cd).as_ref());
        let sig = self.key.sign(&SystemRandom::new(), &signed).unwrap();
        json!({ "id": b64(&self.id), "rawId": b64(&self.id), "type": "public-key", "response": { "clientDataJSON": b64(&cd), "authenticatorData": b64(&auth), "signature": b64(sig.as_ref()) } })
    }
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn cookie(&self, name: &str) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .find(|c| c.starts_with(&format!("{name}=")))
            .map(|c| c[name.len() + 1..].split(';').next().unwrap().to_string())
    }
}

async fn post(router: &Router, path: &str, body: Json, cookies: &str) -> Reply {
    let req = Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("zeyn_csrf={CSRF}; {cookies}"))
        .header("x-zeyn-csrf", CSRF)
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = router.clone().oneshot(req).await.unwrap();
    let (status, headers) = (res.status(), res.headers().clone());
    let body = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    Reply { status, headers, body }
}

#[tokio::test]
async fn passkeys_register_sign_in_and_refuse_replays_and_forgeries() {
    let a = Accounts::new(MemoryAccountStore::new(), Secret::new(KEY.into()))
        .options(|o| o.hasher = PasswordHasher { memory_kib: 256, iterations: 1, parallelism: 1 })
        .public_url(ORIGIN);
    a.users().create(User::new("pk@example.com").with_email("pk@example.com"), Some("Secret-123")).await.unwrap();
    let router = App::new("T").accounts(a.clone()).feature(accounts::feature()).router().unwrap();
    let login = post(&router, "/_zeyn/account/login?useCookies=true", json!({ "email": "pk@example.com", "password": "Secret-123" }), "").await;
    let session = format!("zeyn_account={}", login.cookie("zeyn_account").unwrap());

    let mut device = Authenticator::new();
    let options = post(&router, "/_zeyn/account/manage/passkeys/creationOptions", json!({}), &session).await;
    assert_eq!(options.status, StatusCode::OK, "{}", options.body);
    let opts: Json = serde_json::from_str(&options.body).unwrap();
    assert_eq!(opts["rp"]["id"], "localhost");
    assert_eq!(opts["authenticatorSelection"]["userVerification"], "required");
    assert_eq!(unb64(opts["challenge"].as_str().unwrap()).len(), 32);
    let ceremony = format!("zeyn_account_passkey={}", options.cookie("zeyn_account_passkey").unwrap());
    let cred = device.register(&opts);
    let added = post(&router, "/_zeyn/account/manage/passkeys", json!({ "credentialJson": cred.to_string(), "name": "Laptop" }), &format!("{session}; {ceremony}")).await;
    assert_eq!(added.status, StatusCode::OK, "{}", added.body);
    let again = post(&router, "/_zeyn/account/manage/passkeys", json!({ "credentialJson": cred.to_string(), "name": "Laptop" }), &session).await;
    assert_eq!(again.status, StatusCode::BAD_REQUEST, "a registration needs its own challenge");

    let request = post(&router, "/_zeyn/account/passkeys/requestOptions", json!({}), "").await;
    let ropts: Json = serde_json::from_str(&request.body).unwrap();
    let ceremony = format!("zeyn_account_passkey={}", request.cookie("zeyn_account_passkey").unwrap());
    let assertion = device.assert(&ropts, ORIGIN);
    let signed_in = post(&router, "/_zeyn/account/passkeys/login?useSessionCookies=true", json!({ "credentialJson": assertion.to_string() }), &ceremony).await;
    assert_eq!(signed_in.status, StatusCode::OK, "{}", signed_in.body);
    assert!(signed_in.cookie("zeyn_account").is_some_and(|c| !c.is_empty()));

    let replay = post(&router, "/_zeyn/account/passkeys/login?useSessionCookies=true", json!({ "credentialJson": assertion.to_string() }), "").await;
    assert_eq!(replay.status, StatusCode::BAD_REQUEST, "the challenge is used once");

    let fresh = |r: &Reply| (serde_json::from_str::<Json>(&r.body).unwrap(), format!("zeyn_account_passkey={}", r.cookie("zeyn_account_passkey").unwrap()));
    let (o, c) = fresh(&post(&router, "/_zeyn/account/passkeys/requestOptions", json!({}), "").await);
    device.count -= 1;
    let same_count = device.assert(&o, ORIGIN);
    assert_eq!(post(&router, "/_zeyn/account/passkeys/login", json!({ "credentialJson": same_count.to_string() }), &c).await.status, StatusCode::BAD_REQUEST, "a counter that does not grow is refused");

    let (o, c) = fresh(&post(&router, "/_zeyn/account/passkeys/requestOptions", json!({}), "").await);
    let mut stale = device.assert(&o, ORIGIN);
    let mut sig = unb64(stale["response"]["signature"].as_str().unwrap());
    let last = sig.len() - 1;
    sig[last] ^= 1;
    stale["response"]["signature"] = json!(b64(&sig));
    assert_eq!(post(&router, "/_zeyn/account/passkeys/login", json!({ "credentialJson": stale.to_string() }), &c).await.status, StatusCode::BAD_REQUEST, "a changed signature is refused");

    let (o, c) = fresh(&post(&router, "/_zeyn/account/passkeys/requestOptions", json!({}), "").await);
    let phishing = device.assert(&o, "http://localhost.evil.example");
    assert_eq!(post(&router, "/_zeyn/account/passkeys/login", json!({ "credentialJson": phishing.to_string() }), &c).await.status, StatusCode::BAD_REQUEST, "another origin is refused");

    let user = a.users().find_by_name("pk@example.com").await.unwrap().unwrap();
    let stored = a.users().passkeys(&user).await.unwrap();
    assert_eq!((stored.len(), stored[0].name.as_deref(), stored[0].sign_count), (1, Some("Laptop"), 1));
}
