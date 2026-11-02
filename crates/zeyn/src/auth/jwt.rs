//! JSON Web Tokens signed by an OpenID Connect issuer: signature checks against the issuer's
//! JWKS (cached, bounded, refreshed on an unknown key id at a limited rate), claim checks, and the
//! mapping of claims to an [`Identity`].

use crate::ctx::Identity;
use crate::server::Authenticator;
use crate::time::DateTime;
use axum::http::{HeaderMap, header};
use ring::signature::{self, RsaPublicKeyComponents, UnparsedPublicKey};
use serde::Deserialize;
use serde_json::{Map, Value as Json};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Longest token accepted, in bytes.
const MAX_TOKEN: usize = 16 * 1024;
/// Most keys kept from one JWKS document.
const MAX_KEYS: usize = 16;
/// Largest discovery, JWKS or token response read, in bytes.
pub(crate) const MAX_RESPONSE: usize = 256 * 1024;

/// Signature algorithms a token may use. `none` and the HMAC algorithms (`HS256` and friends) are
/// not here on purpose: a token signed with a shared secret, or with the issuer's public key used
/// as one, is always refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Algorithm {
    RS256,
    RS384,
    RS512,
    PS256,
    PS384,
    PS512,
    ES256,
    ES384,
    EdDSA,
}

impl Algorithm {
    fn parse(name: &str) -> Option<Algorithm> {
        Some(match name {
            "RS256" => Algorithm::RS256,
            "RS384" => Algorithm::RS384,
            "RS512" => Algorithm::RS512,
            "PS256" => Algorithm::PS256,
            "PS384" => Algorithm::PS384,
            "PS512" => Algorithm::PS512,
            "ES256" => Algorithm::ES256,
            "ES384" => Algorithm::ES384,
            "EdDSA" => Algorithm::EdDSA,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Algorithm::RS256 => "RS256",
            Algorithm::RS384 => "RS384",
            Algorithm::RS512 => "RS512",
            Algorithm::PS256 => "PS256",
            Algorithm::PS384 => "PS384",
            Algorithm::PS512 => "PS512",
            Algorithm::ES256 => "ES256",
            Algorithm::ES384 => "ES384",
            Algorithm::EdDSA => "EdDSA",
        }
    }
}

/// Public key material of one JWK.
#[derive(Debug)]
enum Material {
    Rsa { n: Vec<u8>, e: Vec<u8> },
    P256(Vec<u8>),
    P384(Vec<u8>),
    Ed25519(Vec<u8>),
}

/// One signing key of the issuer.
#[derive(Debug)]
pub(crate) struct Jwk {
    kid: Option<String>,
    alg: Option<String>,
    material: Material,
}

impl Jwk {
    /// Reads a JWK. Keys for encryption, private keys, symmetric keys and unknown types are skipped.
    fn parse(v: &Json) -> Option<Jwk> {
        let text = |k: &str| v.get(k).and_then(Json::as_str);
        let bytes = |k: &str| text(k).and_then(super::b64url_decode);
        if text("use").is_some_and(|u| u != "sig") || v.get("d").is_some() {
            return None;
        }
        let material = match (text("kty")?, text("crv")) {
            ("RSA", _) => {
                let n: Vec<u8> = bytes("n")?.into_iter().skip_while(|b| *b == 0).collect();
                let e: Vec<u8> = bytes("e")?.into_iter().skip_while(|b| *b == 0).collect();
                Material::Rsa { n, e }
            }
            ("EC", Some(crv @ ("P-256" | "P-384"))) => {
                let (x, y) = (bytes("x")?, bytes("y")?);
                let size = if crv == "P-256" { 32 } else { 48 };
                if x.len() != size || y.len() != size {
                    return None;
                }
                let mut point = Vec::with_capacity(1 + 2 * size);
                point.push(4);
                point.extend_from_slice(&x);
                point.extend_from_slice(&y);
                if crv == "P-256" { Material::P256(point) } else { Material::P384(point) }
            }
            ("OKP", Some("Ed25519")) => Material::Ed25519(bytes("x")?),
            _ => return None,
        };
        Some(Jwk { kid: text("kid").map(str::to_string), alg: text("alg").map(str::to_string), material })
    }

    /// Whether this key can check a signature made with `alg`.
    fn fits(&self, alg: Algorithm) -> bool {
        if self.alg.as_deref().is_some_and(|a| a != alg.name()) {
            return false;
        }
        use Algorithm::*;
        matches!(
            (&self.material, alg),
            (Material::Rsa { .. }, RS256 | RS384 | RS512 | PS256 | PS384 | PS512)
                | (Material::P256(_), ES256)
                | (Material::P384(_), ES384)
                | (Material::Ed25519(_), EdDSA)
        )
    }

    fn verify(&self, alg: Algorithm, message: &[u8], sig: &[u8]) -> bool {
        use Algorithm::*;
        let rsa = |params: &'static signature::RsaParameters| match &self.material {
            Material::Rsa { n, e } => RsaPublicKeyComponents { n, e }.verify(params, message, sig).is_ok(),
            _ => false,
        };
        match (alg, &self.material) {
            (RS256, _) => rsa(&signature::RSA_PKCS1_2048_8192_SHA256),
            (RS384, _) => rsa(&signature::RSA_PKCS1_2048_8192_SHA384),
            (RS512, _) => rsa(&signature::RSA_PKCS1_2048_8192_SHA512),
            (PS256, _) => rsa(&signature::RSA_PSS_2048_8192_SHA256),
            (PS384, _) => rsa(&signature::RSA_PSS_2048_8192_SHA384),
            (PS512, _) => rsa(&signature::RSA_PSS_2048_8192_SHA512),
            (ES256, Material::P256(p)) => {
                UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_FIXED, p).verify(message, sig).is_ok()
            }
            (ES384, Material::P384(p)) => {
                UnparsedPublicKey::new(&signature::ECDSA_P384_SHA384_FIXED, p).verify(message, sig).is_ok()
            }
            (EdDSA, Material::Ed25519(p)) => {
                UnparsedPublicKey::new(&signature::ED25519, p).verify(message, sig).is_ok()
            }
            _ => false,
        }
    }
}

/// Why a token was refused. Kept short; it goes to debug logs, never to the client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Malformed,
    Algorithm,
    UnknownKey,
    Signature,
    Issuer,
    Audience,
    Expired,
    NotYetValid,
    Nonce,
}

/// The parts of a token header that matter.
#[derive(Deserialize)]
struct Head {
    alg: String,
    kid: Option<String>,
    crit: Option<Json>,
}

fn head(token: &str) -> Result<Head, Refusal> {
    let first = token.split('.').next().ok_or(Refusal::Malformed)?;
    serde_json::from_slice(&super::b64url_decode(first).ok_or(Refusal::Malformed)?).map_err(|_| Refusal::Malformed)
}

/// Key id of a token, read without checking anything.
fn key_id(token: &str) -> Option<String> {
    head(token).ok()?.kid
}

/// Checks the signature of a compact JWS with one of `keys` and returns its claims. Nothing else
/// about the claims is checked here.
pub(crate) fn verify(token: &str, keys: &[Jwk], allowed: &[Algorithm]) -> Result<Map<String, Json>, Refusal> {
    if token.len() > MAX_TOKEN {
        return Err(Refusal::Malformed);
    }
    let mut parts = token.split('.');
    let (Some(h), Some(p), Some(s), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(Refusal::Malformed);
    };
    let head = head(token)?;
    if head.crit.is_some() {
        return Err(Refusal::Malformed);
    }
    let alg = Algorithm::parse(&head.alg).filter(|a| allowed.contains(a)).ok_or(Refusal::Algorithm)?;
    let sig = super::b64url_decode(s).ok_or(Refusal::Malformed)?;
    let message = &token.as_bytes()[..h.len() + 1 + p.len()];
    let mut candidates =
        keys.iter().filter(|k| k.fits(alg)).filter(|k| head.kid.is_none() || k.kid == head.kid).peekable();
    if candidates.peek().is_none() {
        return Err(Refusal::UnknownKey);
    }
    if !candidates.any(|k| k.verify(alg, message, &sig)) {
        return Err(Refusal::Signature);
    }
    let claims: Json =
        serde_json::from_slice(&super::b64url_decode(p).ok_or(Refusal::Malformed)?).map_err(|_| Refusal::Malformed)?;
    match claims {
        Json::Object(map) => Ok(map),
        _ => Err(Refusal::Malformed),
    }
}

/// What the registered claims must say.
pub(crate) struct Expect<'a> {
    pub issuer: &'a str,
    pub audience: &'a str,
    pub leeway: i64,
    pub now: i64,
    /// ID tokens also need `azp` to be the client when there are several audiences. Access
    /// tokens name the calling client there, not the API, so it is not checked for them.
    pub id_token: bool,
}

/// Checks `iss`, `aud` (and `azp` when there are several audiences), `exp`, `nbf` and `iat`.
/// `exp` is required. `iss` must be the configured issuer; only a trailing `/` may differ, since
/// some providers (Authentik) put one there.
pub(crate) fn check_claims(claims: &Map<String, Json>, expect: &Expect) -> Result<(), Refusal> {
    if claims.get("iss").and_then(Json::as_str).map(|i| i.trim_end_matches('/')) != Some(expect.issuer) {
        return Err(Refusal::Issuer);
    }
    let audiences: Vec<&str> = match claims.get("aud") {
        Some(Json::String(a)) => vec![a.as_str()],
        Some(Json::Array(list)) => list.iter().filter_map(Json::as_str).collect(),
        _ => Vec::new(),
    };
    if !audiences.contains(&expect.audience) {
        return Err(Refusal::Audience);
    }
    if expect.id_token
        && audiences.len() > 1
        && claims.get("azp").and_then(Json::as_str).is_some_and(|azp| azp != expect.audience)
    {
        return Err(Refusal::Audience);
    }
    let time = |k: &str| claims.get(k).and_then(Json::as_i64);
    match time("exp") {
        Some(exp) if exp + expect.leeway > expect.now => {}
        _ => return Err(Refusal::Expired),
    }
    if time("nbf").is_some_and(|nbf| nbf - expect.leeway > expect.now)
        || time("iat").is_some_and(|iat| iat - expect.leeway > expect.now)
    {
        return Err(Refusal::NotYetValid);
    }
    Ok(())
}

/// The value at a dotted path such as `realm_access.roles`.
fn at<'a>(claims: &'a Map<String, Json>, path: &str) -> Option<&'a Json> {
    let mut parts = path.split('.');
    let mut value = claims.get(parts.next()?)?;
    for part in parts {
        value = value.get(part)?;
    }
    Some(value)
}

/// Strings of a claim that is a list, or one space separated string (as `scope` is).
fn strings(value: Option<&Json>) -> Vec<String> {
    match value {
        Some(Json::Array(list)) => list.iter().filter_map(Json::as_str).map(str::to_string).collect(),
        Some(Json::String(s)) => s.split_whitespace().map(str::to_string).collect(),
        _ => Vec::new(),
    }
}

/// How the claims of a token become an [`Identity`]. The claim names differ between providers:
///
/// ```ignore
/// ClaimMap::new()
///     .roles("realm_access.roles")
///     .role("stock-admins", ["stock.product.read", "stock.product.write"])
///     .tenant("tenant")
/// ```
#[derive(Clone, Debug)]
pub struct ClaimMap {
    user: Vec<String>,
    tenant: Option<String>,
    roles: Option<String>,
    permissions: Option<String>,
    time_zone: String,
    role_map: HashMap<String, Vec<String>>,
    mfa_acr: Vec<String>,
}

impl Default for ClaimMap {
    fn default() -> Self {
        ClaimMap {
            user: vec!["preferred_username".into(), "sub".into()],
            tenant: None,
            roles: None,
            permissions: None,
            time_zone: "zoneinfo".into(),
            role_map: HashMap::new(),
            mfa_acr: Vec::new(),
        }
    }
}

impl ClaimMap {
    /// User name from `preferred_username`, else `sub`; time zone from `zoneinfo`; no roles.
    pub fn new() -> Self {
        Self::default()
    }

    /// Claim with the user name, such as `email` or `sub`.
    pub fn user(mut self, claim: &str) -> Self {
        self.user = vec![claim.to_string()];
        self
    }

    /// Claim with the tenant.
    pub fn tenant(mut self, claim: &str) -> Self {
        self.tenant = Some(claim.to_string());
        self
    }

    /// Claim (a dotted path for nested claims) with the user's roles or groups:
    /// `realm_access.roles` for Keycloak realm roles, `groups` for Authentik groups.
    pub fn roles(mut self, path: &str) -> Self {
        self.roles = Some(path.to_string());
        self
    }

    /// Claim whose values are Zeyn permissions themselves, as a list or a space separated string
    /// such as `scope`.
    pub fn permissions(mut self, path: &str) -> Self {
        self.permissions = Some(path.to_string());
        self
    }

    /// Claim with the user's time zone; `zoneinfo` unless set.
    pub fn time_zone(mut self, claim: &str) -> Self {
        self.time_zone = claim.to_string();
        self
    }

    /// Permissions granted to holders of a role or members of a group.
    pub fn role<I, S>(mut self, role: &str, permissions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.role_map.entry(role.to_string()).or_default().extend(permissions.into_iter().map(Into::into));
        self
    }

    /// `acr` values that mean a second factor was used, such as Keycloak's level of
    /// authentication `"2"`. They are also asked for (`acr_values`) when a step up sign in starts.
    /// `amr` values such as `mfa` and `otp` count without this.
    pub fn mfa_acr<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.mfa_acr = values.into_iter().map(Into::into).collect();
        self
    }

    pub(crate) fn mfa_acr_values(&self) -> &[String] {
        &self.mfa_acr
    }

    /// The parts of the claims a session keeps: user, tenant, time zone, the roles this map knows,
    /// the direct permissions, the sign in time and whether a second factor was used.
    pub(crate) fn extract(&self, claims: &Map<String, Json>) -> Option<super::Session> {
        let user = self.user.iter().find_map(|c| claims.get(c).and_then(Json::as_str)).filter(|u| !u.is_empty())?;
        let text =
            |c: &Option<String>| c.as_deref().and_then(|c| at(claims, c)).and_then(Json::as_str).map(str::to_string);
        let roles = strings(self.roles.as_deref().and_then(|p| at(claims, p)))
            .into_iter()
            .filter(|r| self.role_map.contains_key(r))
            .collect();
        let amr = strings(claims.get("amr"));
        let amr: Vec<&str> = amr.iter().map(String::as_str).collect();
        let acr = claims.get("acr").and_then(Json::as_str);
        let at_time = claims.get("auth_time").or_else(|| claims.get("iat")).and_then(Json::as_i64);
        Some(super::Session {
            user: user.to_string(),
            tenant: text(&self.tenant),
            time_zone: claims.get(&self.time_zone).and_then(Json::as_str).map(str::to_string),
            roles,
            permissions: strings(self.permissions.as_deref().and_then(|p| at(claims, p))),
            auth_time: at_time,
            mfa: super::shows_mfa(&amr, acr, &self.mfa_acr),
            ..Default::default()
        })
    }

    /// The identity of a session.
    pub(crate) fn identity(&self, s: &super::Session) -> Identity {
        let mut identity = Identity::user(s.user.clone());
        identity.permissions.extend(s.permissions.iter().cloned());
        for role in &s.roles {
            if let Some(perms) = self.role_map.get(role) {
                identity.permissions.extend(perms.iter().cloned());
            }
        }
        identity.tenant = s.tenant.clone();
        identity.time_zone = s.time_zone.clone();
        identity.authenticated_at = s.auth_time.and_then(|t| t.checked_mul(1000)).map(DateTime::from_unix_ms);
        identity.mfa = s.mfa;
        identity
    }
}

/// Endpoints of an issuer from `/.well-known/openid-configuration`.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Discovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
    pub end_session_endpoint: Option<String>,
}

#[derive(Default)]
struct Cached {
    discovery: Option<(Instant, Arc<Discovery>)>,
    keys: Arc<Vec<Jwk>>,
    fetched: Option<Instant>,
    tried: Option<Instant>,
}

/// An issuer's discovery document and signing keys, fetched when needed and cached. Keys are
/// fetched again after `ttl`, or sooner when a token names a key id the cache does not have, but
/// never more often than `min_refresh`, so tokens with made up key ids cannot flood the issuer.
pub(crate) struct Issuer {
    pub issuer: String,
    jwks_url: Option<String>,
    pub ttl: Duration,
    pub min_refresh: Duration,
    http: reqwest::Client,
    cache: RwLock<Cached>,
    fetching: tokio::sync::Mutex<()>,
}

/// HTTP client for an issuer, with TLS from rustls and short timeouts.
pub(crate) fn client() -> reqwest::Client {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_default()
}

/// Reads a JSON response of at most [`MAX_RESPONSE`] bytes.
pub(crate) async fn read_json(res: reqwest::Response) -> Result<Json, String> {
    let status = res.status();
    if res.content_length().is_some_and(|n| n > MAX_RESPONSE as u64) {
        return Err("response too large".into());
    }
    let mut res = res;
    let mut body = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(|e| e.to_string())? {
        if body.len() + chunk.len() > MAX_RESPONSE {
            return Err("response too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    let json: Json = serde_json::from_slice(&body).map_err(|_| format!("status {status}, not JSON"))?;
    if !status.is_success() {
        let error = json.get("error").and_then(Json::as_str).unwrap_or("unknown");
        return Err(format!("status {status}, error `{error}`"));
    }
    Ok(json)
}

impl Issuer {
    pub(crate) fn new(issuer: &str) -> Issuer {
        Issuer {
            issuer: issuer.trim_end_matches('/').to_string(),
            jwks_url: None,
            ttl: Duration::from_secs(3600),
            min_refresh: Duration::from_secs(30),
            http: client(),
            cache: RwLock::default(),
            fetching: tokio::sync::Mutex::new(()),
        }
    }

    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// The discovery document, fetched once per `ttl`. Its `issuer` must be the configured one.
    pub(crate) async fn discovery(&self) -> Result<Arc<Discovery>, String> {
        if let Some((at, d)) = self.cache.read().ok().and_then(|c| c.discovery.clone())
            && at.elapsed() < self.ttl
        {
            return Ok(d);
        }
        let url = format!("{}/.well-known/openid-configuration", self.issuer);
        let json = read_json(self.http.get(&url).send().await.map_err(|e| format!("discovery: {e}"))?).await?;
        let d: Discovery = serde_json::from_value(json).map_err(|e| format!("discovery: {e}"))?;
        if d.issuer.trim_end_matches('/') != self.issuer {
            return Err(format!("discovery names issuer `{}`, expected `{}`", d.issuer, self.issuer));
        }
        let d = Arc::new(d);
        if let Ok(mut c) = self.cache.write() {
            c.discovery = Some((Instant::now(), d.clone()));
        }
        Ok(d)
    }

    pub(crate) fn keys(&self) -> Arc<Vec<Jwk>> {
        self.cache.read().map(|c| c.keys.clone()).unwrap_or_default()
    }

    /// Whether keys should be fetched before checking a token with this key id.
    pub(crate) fn wants_keys(&self, kid: Option<&str>) -> bool {
        let Ok(c) = self.cache.read() else { return false };
        let stale = c.fetched.is_none_or(|at| at.elapsed() >= self.ttl);
        let unknown = !c.keys.iter().any(|k| kid.is_none() || k.kid.as_deref() == kid);
        let may_try = c.tried.is_none_or(|at| at.elapsed() >= self.min_refresh);
        (stale || unknown) && may_try
    }

    /// Fetches the keys when [`Issuer::wants_keys`] says so. One fetch at a time; a failed fetch
    /// keeps the old keys.
    pub(crate) async fn refresh(&self, kid: Option<&str>) {
        let _one = self.fetching.lock().await;
        if !self.wants_keys(kid) {
            return;
        }
        if let Ok(mut c) = self.cache.write() {
            c.tried = Some(Instant::now());
        }
        match self.fetch_keys().await {
            Ok(keys) => {
                if let Ok(mut c) = self.cache.write() {
                    c.keys = Arc::new(keys);
                    c.fetched = Some(Instant::now());
                }
            }
            Err(e) => {
                tracing::warn!(issuer = %self.issuer, "signing keys could not be fetched, keeping the old ones: {e}")
            }
        }
    }

    async fn fetch_keys(&self) -> Result<Vec<Jwk>, String> {
        let url = match &self.jwks_url {
            Some(u) => u.clone(),
            None => self.discovery().await?.jwks_uri.clone(),
        };
        let json = read_json(self.http.get(&url).send().await.map_err(|e| format!("jwks: {e}"))?).await?;
        let keys = json.get("keys").and_then(Json::as_array).ok_or("jwks: no `keys`")?;
        Ok(keys.iter().filter_map(Jwk::parse).take(MAX_KEYS).collect())
    }

    /// Checks a token's signature with the cached keys and its registered claims.
    pub(crate) fn validate(
        &self,
        token: &str,
        audience: &str,
        algorithms: &[Algorithm],
        leeway: Duration,
        id_token: bool,
    ) -> Result<Map<String, Json>, Refusal> {
        let claims = verify(token, &self.keys(), algorithms)?;
        check_claims(
            &claims,
            &Expect {
                issuer: &self.issuer,
                audience,
                leeway: leeway.as_secs() as i64,
                now: super::now_secs(),
                id_token,
            },
        )?;
        Ok(claims)
    }

    /// Checks an ID token like [`Issuer::validate`], fetching keys first when it names an unknown
    /// key.
    pub(crate) async fn validate_id_token(
        &self,
        token: &str,
        audience: &str,
        algorithms: &[Algorithm],
        leeway: Duration,
    ) -> Result<Map<String, Json>, Refusal> {
        let kid = key_id(token);
        if self.wants_keys(kid.as_deref()) {
            self.refresh(kid.as_deref()).await;
        }
        self.validate(token, audience, algorithms, leeway, true)
    }
}

/// Bearer JWTs for APIs, MCP and service calls: `Authorization: Bearer <token>` with a token from
/// an OpenID Connect issuer. The signature is checked against the issuer's JWKS, found through
/// discovery; then `iss`, `aud`, `exp`, `nbf` and `iat` (with a minute of leeway).
///
/// ```ignore
/// App::new("Stock").auth(
///     JwtAuth::new("https://sso.example.com/realms/main", "stock-api")
///         .claims(ClaimMap::new().roles("realm_access.roles").role("stock-admins", ["stock.product.write"])),
/// )
/// ```
pub struct JwtAuth {
    issuer: Arc<Issuer>,
    audience: String,
    algorithms: Vec<Algorithm>,
    leeway: Duration,
    claims: ClaimMap,
}

impl JwtAuth {
    /// Tokens from `issuer` (exactly as in the tokens' `iss`) meant for `audience`. Only `RS256`
    /// is accepted unless [`JwtAuth::algorithms`] says more.
    pub fn new(issuer: &str, audience: &str) -> Self {
        JwtAuth {
            issuer: Arc::new(Issuer::new(issuer)),
            audience: audience.to_string(),
            algorithms: vec![Algorithm::RS256],
            leeway: Duration::from_secs(60),
            claims: ClaimMap::new(),
        }
    }

    /// Signature algorithms accepted.
    pub fn algorithms<I: IntoIterator<Item = Algorithm>>(mut self, algorithms: I) -> Self {
        self.algorithms = algorithms.into_iter().collect();
        self
    }

    /// Clock difference allowed for `exp`, `nbf` and `iat`; one minute unless set, at most five.
    pub fn leeway(mut self, leeway: Duration) -> Self {
        self.leeway = leeway.min(Duration::from_secs(300));
        self
    }

    /// JWKS address, when the issuer has no discovery document.
    pub fn jwks_url(mut self, url: &str) -> Self {
        if let Some(i) = Arc::get_mut(&mut self.issuer) {
            i.jwks_url = Some(url.to_string());
        }
        self
    }

    /// How long fetched keys are trusted before they are fetched again; one hour unless set.
    pub fn key_ttl(mut self, ttl: Duration) -> Self {
        if let Some(i) = Arc::get_mut(&mut self.issuer) {
            i.ttl = ttl;
        }
        self
    }

    /// Shortest time between two key fetches; 30 seconds unless set.
    pub fn min_key_refresh(mut self, every: Duration) -> Self {
        if let Some(i) = Arc::get_mut(&mut self.issuer) {
            i.min_refresh = every;
        }
        self
    }

    /// How claims become the identity.
    pub fn claims(mut self, claims: ClaimMap) -> Self {
        self.claims = claims;
        self
    }

    pub(crate) fn uses(
        issuer: Arc<Issuer>,
        audience: &str,
        algorithms: Vec<Algorithm>,
        leeway: Duration,
        claims: ClaimMap,
    ) -> Self {
        JwtAuth { issuer, audience: audience.to_string(), algorithms, leeway, claims }
    }

    fn bearer(headers: &HeaderMap) -> Option<&str> {
        let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
        let (scheme, token) = value.split_once(' ')?;
        scheme.eq_ignore_ascii_case("bearer").then(|| token.trim()).filter(|t| !t.is_empty())
    }

    /// Whether the request carries a bearer token.
    pub(crate) fn has_bearer(headers: &HeaderMap) -> bool {
        Self::bearer(headers).is_some()
    }
}

impl Authenticator for JwtAuth {
    fn authenticate(&self, headers: &HeaderMap) -> Option<Identity> {
        let token = Self::bearer(headers)?;
        match self.issuer.validate(token, &self.audience, &self.algorithms, self.leeway, false) {
            Ok(claims) => self.claims.extract(&claims).map(|s| self.claims.identity(&s)),
            Err(why) => {
                tracing::debug!(?why, "bearer token refused");
                None
            }
        }
    }

    fn prepare<'a>(&'a self, headers: &'a HeaderMap) -> Option<Pin<Box<dyn Future<Output = ()> + Send + 'a>>> {
        let kid = key_id(Self::bearer(headers)?);
        if !self.issuer.wants_keys(kid.as_deref()) {
            return None;
        }
        Some(Box::pin(async move { self.issuer.refresh(kid.as_deref()).await }))
    }
}
