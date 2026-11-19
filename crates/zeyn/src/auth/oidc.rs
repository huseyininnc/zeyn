//! OpenID Connect sign in for browser apps: the authorization code flow with PKCE, `state` and
//! `nonce`, and a sealed session cookie, so the server keeps no per user state.

use super::jwt::{Algorithm, ClaimMap, Issuer, JwtAuth, read_json};
use super::seal::Sealer;
use crate::ctx::Identity;
use crate::platform::Secret;
use crate::server::Authenticator;
use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

/// Name of the session cookie.
pub(crate) const SESSION: &str = "zeyn_session";
/// Name of the short lived cookie that carries `state`, `nonce` and the PKCE verifier from the
/// sign in redirect to the callback.
pub(crate) const FLOW: &str = "zeyn_oidc";
/// Largest session cookie written, in bytes. Browsers keep at most about 4096 per cookie.
pub(crate) const MAX_COOKIE: usize = 3800;
/// How long a sign in may take between the redirect and the callback.
pub(crate) const FLOW_TTL: i64 = 600;

/// What the session cookie holds. Short field names keep the cookie small.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Session {
    #[serde(rename = "u")]
    pub user: String,
    #[serde(rename = "t", default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    /// Further tenants when the tenant claim names several.
    #[serde(rename = "ts", default, skip_serializing_if = "Vec::is_empty")]
    pub tenants: Vec<String>,
    #[serde(rename = "z", default, skip_serializing_if = "Option::is_none")]
    pub time_zone: Option<String>,
    #[serde(rename = "r", default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(rename = "p", default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    #[serde(rename = "a", default, skip_serializing_if = "Option::is_none")]
    pub auth_time: Option<i64>,
    #[serde(rename = "m", default)]
    pub mfa: bool,
    /// When the session ends, in Unix seconds.
    #[serde(rename = "e", default)]
    pub expires: i64,
    /// When the tokens behind the session expire and a refresh is due.
    #[serde(rename = "x", default)]
    pub check: i64,
    #[serde(rename = "rt", default, skip_serializing_if = "Option::is_none")]
    pub refresh: Option<String>,
}

/// What the flow cookie holds between the sign in redirect and the callback.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Flow {
    #[serde(rename = "s")]
    pub state: String,
    #[serde(rename = "n")]
    pub nonce: String,
    #[serde(rename = "v")]
    pub verifier: String,
    #[serde(rename = "r")]
    pub return_to: String,
    #[serde(rename = "m", default)]
    pub mfa: bool,
    #[serde(rename = "e")]
    pub expires: i64,
}

/// Sign in with an OpenID Connect provider (Keycloak, Authentik and other compliant ones) for
/// browser apps. Users who are not signed in are sent to the provider; the callback checks
/// `state`, the PKCE verifier and the ID token (signature, `iss`, `aud`, `exp`, `nonce`) and
/// keeps the session in an encrypted cookie. No session is kept in server memory.
///
/// ```ignore
/// let oidc = OidcAuth::new(
///     "https://sso.example.com/realms/main",
///     "stock-web",
///     "https://stock.example.com",
///     config.secret("auth.session_key")?,
/// )
/// .client_secret(config.secret("auth.client_secret")?)
/// .claims(ClaimMap::new().roles("realm_access.roles").role("stock-admins", ["stock.product.write"]));
/// App::new("Stock").oidc(oidc)
/// ```
///
/// The app gets `/_zeyn/auth/login`, `/_zeyn/auth/callback` and `/_zeyn/auth/logout`; register
/// `<public url>/_zeyn/auth/callback` as the redirect URI and `<public url>/` as the post logout
/// redirect URI at the provider.
pub struct OidcAuth {
    pub(crate) issuer: Arc<Issuer>,
    pub(crate) client_id: String,
    client_secret: Option<Secret<String>>,
    pub(crate) sealer: Option<Sealer>,
    pub(crate) public_url: String,
    pub(crate) scopes: String,
    pub(crate) algorithms: Vec<Algorithm>,
    pub(crate) leeway: Duration,
    pub(crate) claims: ClaimMap,
    pub(crate) lifetime: Duration,
    pub(crate) refresh: bool,
    bearer: Option<String>,
    bearer_auth: Option<JwtAuth>,
}

impl OidcAuth {
    /// Sign in at `issuer` as client `client_id`. `public_url` is the app's address as users see
    /// it (scheme and host, such as `https://stock.example.com`); the redirect URI is built from it,
    /// never from the request's `Host` header. `session_key` seals the cookies; it must be at least
    /// 32 bytes, from configuration (`Config::secret`), never from code.
    pub fn new(
        issuer: &str,
        client_id: &str,
        public_url: &str,
        session_key: Secret<String>,
    ) -> Self {
        OidcAuth {
            issuer: Arc::new(Issuer::new(issuer)),
            client_id: client_id.to_string(),
            client_secret: None,
            sealer: Sealer::new(&session_key),
            public_url: public_url
                .trim_end_matches('/')
                .to_string(),
            scopes: "openid profile email".into(),
            algorithms: vec![Algorithm::RS256],
            leeway: Duration::from_secs(60),
            claims: ClaimMap::new(),
            lifetime: Duration::from_secs(8 * 3600),
            refresh: false,
            bearer: None,
            bearer_auth: None,
        }
    }

    /// Secret of a confidential client, sent with HTTP basic authentication. Public clients rely
    /// on PKCE alone.
    pub fn client_secret(mut self, secret: Secret<String>) -> Self {
        self.client_secret = Some(secret);
        self
    }

    /// Scopes asked for; `openid profile email` unless set. `openid` is always added.
    pub fn scopes(mut self, scopes: &str) -> Self {
        self.scopes = if scopes
            .split_whitespace()
            .any(|s| s == "openid")
        {
            scopes.to_string()
        } else {
            format!("openid {scopes}")
        };
        self
    }

    /// How claims become the identity.
    pub fn claims(mut self, claims: ClaimMap) -> Self {
        self.claims = claims;
        self
    }

    /// Signature algorithms accepted for ID tokens; only `RS256` unless set.
    pub fn algorithms<I: IntoIterator<Item = Algorithm>>(mut self, algorithms: I) -> Self {
        self.algorithms = algorithms.into_iter().collect();
        self
    }

    /// Clock difference allowed for `exp`, `nbf` and `iat`; one minute unless set, at most five.
    pub fn leeway(mut self, leeway: Duration) -> Self {
        self.leeway = leeway.min(Duration::from_secs(300));
        self
    }

    /// Longest a session lasts before the user signs in again; eight hours unless set.
    pub fn session_lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = lifetime;
        self
    }

    /// Keeps the refresh token in the session cookie and refreshes the tokens when they expire,
    /// so a user who is disabled or signed out at the provider loses the session within one token
    /// lifetime. Off unless set; without it the session lasts `session_lifetime`.
    pub fn refresh_tokens(mut self) -> Self {
        self.refresh = true;
        self
    }

    /// Also accepts `Authorization: Bearer` access tokens from the same issuer meant for
    /// `audience`, for API and MCP clients of the same app. Claims map the same way.
    pub fn accept_bearer(mut self, audience: &str) -> Self {
        self.bearer = Some(audience.to_string());
        self
    }

    /// Builds the bearer authenticator once the settings are final.
    pub(crate) fn finish(&mut self) {
        if let Some(audience) = &self.bearer {
            self.bearer_auth = Some(JwtAuth::uses(
                self.issuer.clone(),
                audience,
                self.algorithms.clone(),
                self.leeway,
                self.claims.clone(),
            ));
        }
    }

    /// The problems that stop the app from starting.
    pub(crate) fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.sealer.is_none() {
            problems.push(
                crate::Problem::new(
                    "Z0204",
                    format!("OidcAuth: the session key must be at least {} bytes", super::seal::MIN_SECRET),
                )
                .why("the key seals the session cookie, and a short key could be guessed, which would let anyone forge a session")
                .fix("read a random key of at least 32 bytes from configuration (never from code), such as `openssl rand -base64 48`")
                .docs("guides/sign-in#browser-sign-in")
                .to_string(),
            );
        }
        if !(self.public_url.starts_with("https://") || self.public_url.starts_with("http://")) {
            problems.push(
                crate::Problem::new(
                    "Z0205",
                    "OidcAuth: the public URL must start with https:// (or http:// for local development)",
                )
                .why("the redirect URI sent to the identity provider is built from it, and the provider rejects one without a scheme")
                .fix("pass the address users open, such as `https://app.example`, and register its callback at the provider")
                .docs("guides/sign-in#browser-sign-in")
                .to_string(),
            );
        }
        problems
    }

    /// Whether cookies get `Secure`: whenever the app is served over HTTPS.
    pub(crate) fn secure(&self) -> bool {
        self.public_url.starts_with("https://")
    }

    /// The session in a cookie value, when it opens and has not ended.
    pub(crate) fn open_session(&self, value: &str) -> Option<Session> {
        let s: Session = self
            .sealer
            .as_ref()?
            .open(SESSION, value)?;
        (s.expires > super::now_secs()).then_some(s)
    }

    /// The address of the provider's sign in page for a new flow.
    pub(crate) async fn authorize_url(
        &self,
        flow: &Flow,
        redirect_uri: &str,
        reauth: bool,
    ) -> Result<String, String> {
        let d = self.issuer.discovery().await?;
        let challenge = super::b64url(
            ring::digest::digest(&ring::digest::SHA256, flow.verifier.as_bytes()).as_ref(),
        );
        let mut url = format!(
            "{}{}response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&nonce={}&code_challenge={}&code_challenge_method=S256",
            d.authorization_endpoint,
            if d.authorization_endpoint.contains('?') {
                '&'
            } else {
                '?'
            },
            pct(&self.client_id),
            pct(redirect_uri),
            pct(&self.scopes),
            pct(&flow.state),
            pct(&flow.nonce),
            challenge,
        );
        if flow.mfa && !self.claims.mfa_acr_values().is_empty() {
            url.push_str(&format!(
                "&acr_values={}",
                pct(&self.claims.mfa_acr_values().join(" "))
            ));
        }
        if flow.mfa || reauth {
            url.push_str("&prompt=login&max_age=0");
        }
        Ok(url)
    }

    async fn token_call(&self, form: &[(&str, &str)]) -> Result<Json, String> {
        let d = self.issuer.discovery().await?;
        let mut body = String::new();
        for (k, v) in form {
            if !body.is_empty() {
                body.push('&');
            }
            body.push_str(&format!("{}={}", pct(k), pct(v)));
        }
        let mut req = self
            .issuer
            .http()
            .post(&d.token_endpoint)
            .header("content-type", "application/x-www-form-urlencoded")
            .header("accept", "application/json");
        match &self.client_secret {
            Some(secret) => req = req.basic_auth(pct(&self.client_id), Some(pct(secret.expose()))),
            None => {
                body.push_str("&client_id=");
                body.push_str(&pct(&self.client_id));
            }
        }
        read_json(
            req.body(body)
                .send()
                .await
                .map_err(|e| format!("token endpoint: {e}"))?,
        )
        .await
    }

    /// Exchanges the authorization code for tokens and builds the session. The ID token must carry
    /// the flow's nonce.
    pub(crate) async fn exchange(
        &self,
        code: &str,
        flow: &Flow,
        redirect_uri: &str,
    ) -> Result<Session, String> {
        let tokens = self
            .token_call(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("code_verifier", &flow.verifier),
            ])
            .await?;
        self.session(&tokens, Some(&flow.nonce), None)
            .await
    }

    /// Exchanges the authorization code and returns the checked ID token's claims, for linking
    /// the provider's users to local accounts (`zeyn::accounts`).
    #[cfg(feature = "accounts")]
    pub(crate) async fn exchange_claims(
        &self,
        code: &str,
        flow: &Flow,
        redirect_uri: &str,
    ) -> Result<serde_json::Map<String, Json>, String> {
        let tokens = self
            .token_call(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("code_verifier", &flow.verifier),
            ])
            .await?;
        let id_token = tokens
            .get("id_token")
            .and_then(Json::as_str)
            .ok_or("the token response has no ID token")?;
        let claims = self
            .issuer
            .validate_id_token(id_token, &self.client_id, &self.algorithms, self.leeway)
            .await
            .map_err(|why| format!("ID token refused: {why:?}"))?;
        if !claims
            .get("nonce")
            .and_then(Json::as_str)
            .is_some_and(|n| crate::server::same(n, &flow.nonce))
        {
            return Err(format!(
                "ID token refused: {:?}",
                super::jwt::Refusal::Nonce
            ));
        }
        Ok(claims)
    }

    /// Uses the session's refresh token for new tokens and a renewed session.
    pub(crate) async fn renew(&self, old: &Session) -> Result<Session, String> {
        let rt = old
            .refresh
            .as_deref()
            .ok_or("no refresh token")?;
        let tokens = self
            .token_call(&[("grant_type", "refresh_token"), ("refresh_token", rt)])
            .await?;
        self.session(&tokens, None, Some(old))
            .await
    }

    async fn session(
        &self,
        tokens: &Json,
        nonce: Option<&str>,
        old: Option<&Session>,
    ) -> Result<Session, String> {
        let id_token = tokens
            .get("id_token")
            .and_then(Json::as_str)
            .ok_or("the token response has no ID token")?;
        let claims = self
            .issuer
            .validate_id_token(id_token, &self.client_id, &self.algorithms, self.leeway)
            .await
            .map_err(|why| format!("ID token refused: {why:?}"))?;
        if let Some(nonce) = nonce
            && !claims
                .get("nonce")
                .and_then(Json::as_str)
                .is_some_and(|n| crate::server::same(n, nonce))
        {
            return Err(format!(
                "ID token refused: {:?}",
                super::jwt::Refusal::Nonce
            ));
        }
        let mut s = self
            .claims
            .extract(&claims)
            .ok_or("the ID token has no user claim")?;
        if let Some(old) = old {
            if s.user != old.user {
                return Err("refreshed ID token is for another user".into());
            }
            s.mfa |= old.mfa;
        }
        let now = super::now_secs();
        let exp = claims
            .get("exp")
            .and_then(Json::as_i64)
            .unwrap_or(now);
        let access = tokens
            .get("expires_in")
            .and_then(Json::as_i64)
            .map(|s| now + s)
            .unwrap_or(exp);
        s.expires = old
            .map(|o| o.expires)
            .unwrap_or(now + self.lifetime.as_secs() as i64);
        s.check = exp.min(access);
        if self.refresh {
            s.refresh = tokens
                .get("refresh_token")
                .and_then(Json::as_str)
                .map(str::to_string)
                .or_else(|| old.and_then(|o| o.refresh.clone()));
        }
        Ok(s)
    }

    /// Seals a session for the cookie. `Err` when it would be too large for a browser to keep.
    pub(crate) fn seal_session(&self, s: &Session) -> Result<String, String> {
        let sealer = self
            .sealer
            .as_ref()
            .ok_or("no session key")?;
        let value = sealer.seal(SESSION, s);
        if value.len() > MAX_COOKIE {
            return Err(format!(
                "the session cookie would be {} bytes (limit {MAX_COOKIE}); map fewer roles or turn off refresh tokens",
                value.len()
            ));
        }
        Ok(value)
    }

    /// Where a signed out user goes: the provider's end session endpoint, which comes back to
    /// `back`, or `back` itself when the provider has none.
    pub(crate) async fn logout_url(&self, back: &str) -> String {
        match self
            .issuer
            .discovery()
            .await
            .ok()
            .and_then(|d| d.end_session_endpoint.clone())
        {
            Some(end) => format!(
                "{end}{}client_id={}&post_logout_redirect_uri={}",
                if end.contains('?') { '&' } else { '?' },
                pct(&self.client_id),
                pct(back)
            ),
            None => back.to_string(),
        }
    }
}

impl Authenticator for OidcAuth {
    fn authenticate(&self, headers: &HeaderMap) -> Option<Identity> {
        if let Some(bearer) = &self.bearer_auth
            && JwtAuth::has_bearer(headers)
        {
            return bearer.authenticate(headers);
        }
        let session = self.open_session(crate::server::cookie(headers, SESSION)?)?;
        Some(self.claims.identity(&session))
    }

    fn prepare<'a>(
        &'a self,
        headers: &'a HeaderMap,
    ) -> Option<Pin<Box<dyn Future<Output = ()> + Send + 'a>>> {
        self.bearer_auth
            .as_ref()?
            .prepare(headers)
    }
}

/// Percent encodes everything but the unreserved characters of RFC 3986.
pub(crate) fn pct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
