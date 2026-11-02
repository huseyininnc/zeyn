//! [`SignInManager`] (`SignInManager<TUser>`) and the stateless session.
//!
//! The session is one sealed cookie, `zeyn_account`, like [`crate::OidcAuth`]'s: AES-256-GCM
//! with a key derived from the accounts key, nothing kept in server memory. It holds the user's
//! id and name, roles, permission claims, how and when the user signed in, whether a second
//! factor was used, and a keyed fingerprint of the security stamp. Every `validation_interval`
//! (thirty minutes, like `SecurityStampValidator`) the server compares the fingerprint with the
//! stored stamp: a changed stamp ends the session, an unchanged one renews it with fresh roles.
//! Bearer access tokens for API clients are the same session sealed under another name.

use super::model::User;
use super::{AccountEventKind, Accounts, UserManager, denied, now_secs, scope};
use crate::ctx::{Identity, Service};
use crate::security::RateLimit;
use crate::time::DateTime;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Name of the session cookie.
pub(crate) const SESSION: &str = "zeyn_account";
/// Name of the cookie that carries a sign in waiting for its second factor (five minutes).
pub(crate) const TWO_FACTOR: &str = "zeyn_account_2fa";
/// Name of the cookie of a device that may skip the second factor.
pub(crate) const DEVICE: &str = "zeyn_account_device";
/// Seal name of bearer access tokens.
pub(crate) const BEARER: &str = "zeyn_account_bearer";
/// Seal name of bearer refresh tokens.
pub(crate) const REFRESH: &str = "zeyn_account_refresh";
/// How long a pending second factor may take.
const TWO_FACTOR_TTL: i64 = 300;
/// Largest session cookie written.
const MAX_COOKIE: usize = 3800;

/// What the session cookie and bearer tokens hold. Short names keep the cookie small.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Session {
    #[serde(rename = "i")]
    pub id: String,
    #[serde(rename = "u")]
    pub user: String,
    #[serde(rename = "r", default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    #[serde(rename = "p", default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<String>,
    /// Keyed fingerprint of the security stamp.
    #[serde(rename = "s")]
    pub stamp: String,
    /// When the user signed in, Unix seconds.
    #[serde(rename = "a")]
    pub auth_time: i64,
    #[serde(rename = "m", default)]
    pub mfa: bool,
    /// How the user signed in: `pwd`, `mfa`, `passkey` or an external provider.
    #[serde(rename = "w", default)]
    pub method: String,
    /// When the stamp was last checked.
    #[serde(rename = "v")]
    pub validated: i64,
    /// When this value stops working.
    #[serde(rename = "e")]
    pub expires: i64,
    /// A persistent ("remember me") cookie rather than a browser session cookie.
    #[serde(rename = "k", default)]
    pub persistent: bool,
}

#[derive(Serialize, Deserialize)]
struct Pending {
    #[serde(rename = "i")]
    id: String,
    #[serde(rename = "s")]
    stamp: String,
    #[serde(rename = "r")]
    remember: bool,
    #[serde(rename = "l", default)]
    provider: Option<String>,
    #[serde(rename = "e")]
    expires: i64,
}

#[derive(Serialize, Deserialize)]
struct Device {
    #[serde(rename = "i")]
    id: String,
    #[serde(rename = "s")]
    stamp: String,
    #[serde(rename = "e")]
    expires: i64,
}

#[derive(Serialize, Deserialize)]
struct Refresh {
    #[serde(rename = "i")]
    id: String,
    #[serde(rename = "s")]
    stamp: String,
    #[serde(rename = "w")]
    method: String,
    #[serde(rename = "m", default)]
    mfa: bool,
    #[serde(rename = "a")]
    auth_time: i64,
    #[serde(rename = "e")]
    expires: i64,
}

/// Outcome of a sign in attempt (`SignInResult`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignInResult {
    Succeeded,
    /// Locked out after too many failures; try again later.
    LockedOut,
    /// The user may not sign in yet, such as with an unconfirmed e-mail address.
    NotAllowed,
    /// The password was right; a second factor is needed next.
    RequiresTwoFactor,
    Failed,
}

/// A second factor (`TokenOptions` providers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TwoFactorKind {
    Authenticator,
    Email,
    Phone,
}

/// Bearer tokens for API clients (`AccessTokenResponse`).
#[derive(Clone, Serialize)]
pub struct AccessTokens {
    #[serde(rename = "tokenType")]
    pub token_type: &'static str,
    #[serde(rename = "accessToken")]
    pub access_token: String,
    #[serde(rename = "expiresIn")]
    pub expires_in: i64,
    #[serde(rename = "refreshToken")]
    pub refresh_token: String,
}

impl std::fmt::Debug for AccessTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccessTokens([redacted])")
    }
}

/// Signing in and out: passwords with lockout and rate limits, the two factor flow, remembered
/// devices, external logins, passkeys and bearer tokens (`SignInManager<TUser>`). Take it as an
/// action argument; `App::accounts` registers it.
#[derive(Clone)]
pub struct SignInManager(pub(crate) Accounts);

impl Service for SignInManager {}

impl SignInManager {
    fn users(&self) -> UserManager {
        self.0.users()
    }

    fn sealer(&self) -> Result<&crate::auth::seal::Sealer> {
        self.0.0.sealer.as_ref().ok_or_else(|| Error::Internal("accounts key too short".into()))
    }

    fn tag(&self, user: &User) -> String {
        self.0.0.keys.stamp_tag(&user.id, &user.security_stamp)
    }

    /// Whether the user may sign in at all (`CanSignInAsync`): a confirmed e-mail or phone when
    /// the options ask for one.
    pub fn can_sign_in(&self, user: &User) -> bool {
        let o = self.0.0.options.sign_in;
        !(o.require_confirmed_email && !user.email_confirmed) && !(o.require_confirmed_phone_number && !user.phone_number_confirmed)
    }

    /// Takes one attempt from the per address and per account allowances.
    async fn throttle(&self, account: &str) -> Result<()> {
        let o = &self.0.0.options;
        let ip = scope::ip().map(|i| i.to_string()).unwrap_or_else(|| "unknown".into());
        let by_ip = scope::limit(&format!("accounts:ip:{ip}"), RateLimit::per_ip(o.ip_attempts.0, o.ip_attempts.1)).await;
        let by_account = scope::limit(&format!("accounts:user:{}", super::normalize(account)), RateLimit::per_user(o.account_attempts.0, o.account_attempts.1)).await;
        if by_ip.is_err() || by_account.is_err() {
            self.0.event(AccountEventKind::RateLimited, None);
            return Err(denied("rate_limited"));
        }
        Ok(())
    }

    /// Takes one attempt from the per address allowance only, for forms without an account name.
    pub(crate) async fn throttle_ip(&self) -> Result<()> {
        let o = &self.0.0.options;
        let ip = scope::ip().map(|i| i.to_string()).unwrap_or_else(|| "unknown".into());
        if scope::limit(&format!("accounts:ip:{ip}"), RateLimit::per_ip(o.ip_attempts.0, o.ip_attempts.1)).await.is_err() {
            self.0.event(AccountEventKind::RateLimited, None);
            return Err(denied("rate_limited"));
        }
        Ok(())
    }

    /// Signs in with a user name (or e-mail) and password (`PasswordSignInAsync`). A missing
    /// user costs the same password hashing time as a wrong password and gives the same
    /// `Failed`. Rate limited per client address and per account name.
    pub async fn password_sign_in(&self, name: &str, password: &str, remember: bool, lockout_on_failure: bool) -> Result<SignInResult> {
        self.throttle(name).await?;
        let Some(mut user) = self.users().find_by_name_or_email(name).await? else {
            self.users().dummy_check(password).await;
            self.0.event(AccountEventKind::SignInFailed, None);
            return Ok(SignInResult::Failed);
        };
        let result = self.check_password_sign_in(&mut user, password, lockout_on_failure).await?;
        if result != SignInResult::Succeeded {
            return Ok(result);
        }
        self.sign_in_or_two_factor(&user, remember, None).await
    }

    /// Checks the password with lockout but does not sign in (`CheckPasswordSignInAsync`).
    pub async fn check_password_sign_in(&self, user: &mut User, password: &str, lockout_on_failure: bool) -> Result<SignInResult> {
        if let Some(early) = self.pre_sign_in(user) {
            self.users().dummy_check(password).await;
            return Ok(early);
        }
        if self.users().check_password(user, password).await? {
            if !user.two_factor_enabled || self.is_two_factor_client_remembered(user) {
                self.users().reset_access_failed(user).await?;
            }
            return Ok(SignInResult::Succeeded);
        }
        self.0.event(AccountEventKind::SignInFailed, Some(&user.id));
        if lockout_on_failure && self.users().access_failed(user).await? {
            return Ok(SignInResult::LockedOut);
        }
        Ok(SignInResult::Failed)
    }

    fn pre_sign_in(&self, user: &User) -> Option<SignInResult> {
        if !self.can_sign_in(user) {
            return Some(SignInResult::NotAllowed);
        }
        if user.is_locked_out(now_secs()) {
            return Some(SignInResult::LockedOut);
        }
        None
    }

    /// Whether the user has two factor on and a factor to use.
    pub async fn is_two_factor_enabled(&self, user: &User) -> Result<bool> {
        Ok(user.two_factor_enabled && !self.users().two_factor_kinds(user).await?.is_empty())
    }

    async fn sign_in_or_two_factor(&self, user: &User, remember: bool, provider: Option<&str>) -> Result<SignInResult> {
        if self.is_two_factor_enabled(user).await? && !self.is_two_factor_client_remembered(user) {
            let pending = Pending { id: user.id.clone(), stamp: self.tag(user), remember, provider: provider.map(str::to_string), expires: now_secs() + TWO_FACTOR_TTL };
            scope::set_cookie(TWO_FACTOR, Some(&self.sealer()?.seal(TWO_FACTOR, &pending)), Some(TWO_FACTOR_TTL));
            return Ok(SignInResult::RequiresTwoFactor);
        }
        self.sign_in(user, remember, provider.unwrap_or("pwd"), false).await?;
        Ok(SignInResult::Succeeded)
    }

    /// Builds the session of a user, with roles and permission claims from the store.
    pub(crate) async fn session(&self, user: &User, persistent: bool, method: &str, mfa: bool, auth_time: i64) -> Result<Session> {
        let store = &self.0.0.store;
        let roles = store.roles_of(&user.id).await?;
        let mut permissions: Vec<String> = store.claims_of(&user.id).await?.into_iter().filter(|c| c.kind == "permission").map(|c| c.value).collect();
        for role in &roles {
            permissions.extend(store.role_claims(&role.id).await?.into_iter().filter(|c| c.kind == "permission").map(|c| c.value));
        }
        permissions.sort();
        permissions.dedup();
        let now = now_secs();
        Ok(Session {
            id: user.id.clone(),
            user: user.user_name.clone(),
            roles: roles.into_iter().map(|r| r.name).collect(),
            permissions,
            stamp: self.tag(user),
            auth_time,
            mfa,
            method: method.to_string(),
            validated: now,
            expires: now + self.0.0.options.session.expire.as_secs() as i64,
            persistent,
        })
    }

    pub(crate) fn seal_session(&self, s: &Session) -> Result<String> {
        let value = self.sealer()?.seal(SESSION, s);
        if value.len() > MAX_COOKIE {
            return Err(Error::Internal(format!("the account session cookie would be {} bytes; give the user fewer roles or permission claims", value.len())));
        }
        Ok(value)
    }

    pub(crate) fn write_session(&self, s: &Session) -> Result<()> {
        let value = self.seal_session(s)?;
        let age = s.persistent.then(|| s.expires - now_secs());
        scope::set_cookie(SESSION, Some(&value), age);
        Ok(())
    }

    /// Signs the user in: sets the session cookie, persistent when `remember` (`SignInAsync`).
    /// `method` says how (`pwd`, `passkey`, a provider name).
    pub async fn sign_in(&self, user: &User, remember: bool, method: &str, mfa: bool) -> Result<()> {
        let s = self.session(user, remember, method, mfa, now_secs()).await?;
        self.write_session(&s)?;
        scope::set_cookie(TWO_FACTOR, None, None);
        self.0.event(AccountEventKind::SignedIn, Some(&user.id));
        Ok(())
    }

    /// Signs out of this browser (`SignOutAsync`). Other sessions stay; change the security
    /// stamp to end those too.
    pub fn sign_out(&self, user: Option<&str>) {
        scope::set_cookie(SESSION, None, None);
        scope::set_cookie(TWO_FACTOR, None, None);
        self.0.event(AccountEventKind::SignedOut, user);
    }

    /// The signed in user's session from the request, when there is a valid one.
    pub(crate) fn current(&self) -> Option<Session> {
        let value = scope::cookie(SESSION)?;
        self.open_session(&value)
    }

    pub(crate) fn open_session(&self, value: &str) -> Option<Session> {
        let s: Session = self.0.0.sealer.as_ref()?.open(SESSION, value)?;
        (s.expires > now_secs()).then_some(s)
    }

    /// Renews the session of the signed in user after a change to their roles, claims or stamp,
    /// keeping how and when they signed in (`RefreshSignInAsync`). Does nothing when `user` is not
    /// the signed in user.
    pub async fn refresh_sign_in(&self, user: &User) -> Result<()> {
        let Some(old) = self.current() else { return Ok(()) };
        if old.id != user.id {
            return Ok(());
        }
        let s = self.session(user, old.persistent, &old.method, old.mfa, old.auth_time).await?;
        self.write_session(&s)
    }

    /// The user behind a session fingerprint when the stamp still matches
    /// (`ValidateSecurityStampAsync`).
    pub async fn validate_security_stamp(&self, id: &str, tag: &str) -> Result<Option<User>> {
        let Some(user) = self.users().find_by_id(id).await? else { return Ok(None) };
        Ok(crate::server::same(&self.tag(&user), tag).then_some(user))
    }

    /// The session renewed with fresh roles when its stamp still matches, `None` when the stamp
    /// changed or the user is gone (`SecurityStampValidator`).
    pub(crate) async fn revalidate(&self, s: &Session) -> Result<Option<Session>> {
        let Some(user) = self.validate_security_stamp(&s.id, &s.stamp).await? else { return Ok(None) };
        Ok(Some(self.session(&user, s.persistent, &s.method, s.mfa, s.auth_time).await?))
    }

    /// The user waiting for a second factor, when the pending sign in is still valid and the
    /// stamp has not changed since (`GetTwoFactorAuthenticationUserAsync`).
    pub async fn two_factor_user(&self) -> Result<Option<User>> {
        Ok(self.pending().await?.map(|(u, _)| u))
    }

    async fn pending(&self) -> Result<Option<(User, Pending)>> {
        let Some(value) = scope::cookie(TWO_FACTOR) else { return Ok(None) };
        let Some(p) = self.sealer()?.open::<Pending>(TWO_FACTOR, &value) else { return Ok(None) };
        if p.expires <= now_secs() {
            return Ok(None);
        }
        Ok(self.validate_security_stamp(&p.id, &p.stamp).await?.map(|u| (u, p)))
    }

    /// Sends a sign in code by e-mail or text message to the user waiting for a second factor.
    pub async fn send_two_factor_code(&self, ctx: &crate::Ctx, kind: TwoFactorKind) -> Result<()> {
        self.throttle_ip().await?;
        let Some((user, _)) = self.pending().await? else { return Err(denied("two_factor_expired")) };
        let (to, code) = match kind {
            TwoFactorKind::Email if user.email_confirmed => (user.email.clone(), self.users().generate_email_code(&user)),
            TwoFactorKind::Phone if user.phone_number_confirmed => (user.phone_number.clone(), self.users().generate_phone_code(&user)),
            _ => (None, String::new()),
        };
        let Some(to) = to else { return Err(denied("two_factor_unavailable")) };
        self.0.0.sender.send(ctx, &to, super::AccountMessage::TwoFactorCode { code }).await
    }

    /// Completes a sign in with a second factor code (`TwoFactorAuthenticatorSignInAsync`,
    /// `TwoFactorSignInAsync`). Wrong codes count toward lockout. `remember_client` lets this
    /// browser skip the second factor for `remember_two_factor`.
    pub async fn two_factor_sign_in(&self, kind: TwoFactorKind, code: &str, remember_client: bool) -> Result<SignInResult> {
        self.throttle_ip().await?;
        let Some((mut user, pending)) = self.pending().await? else { return Ok(SignInResult::Failed) };
        if let Some(early) = self.pre_sign_in(&user) {
            return Ok(early);
        }
        let ok = match kind {
            TwoFactorKind::Authenticator => self.users().verify_authenticator_code(&user, code).await?,
            TwoFactorKind::Email => user.email_confirmed && self.users().verify_email_code(&user, code),
            TwoFactorKind::Phone => user.phone_number_confirmed && self.users().verify_phone_code(&user, code),
        };
        if !ok {
            self.0.event(AccountEventKind::SignInFailed, Some(&user.id));
            if self.users().access_failed(&mut user).await? {
                return Ok(SignInResult::LockedOut);
            }
            return Ok(SignInResult::Failed);
        }
        self.finish_two_factor(&mut user, &pending, remember_client).await
    }

    /// Completes a sign in with a recovery code, which is then used up
    /// (`TwoFactorRecoveryCodeSignInAsync`). The client is not remembered.
    pub async fn recovery_code_sign_in(&self, code: &str) -> Result<SignInResult> {
        self.throttle_ip().await?;
        let Some((mut user, pending)) = self.pending().await? else { return Ok(SignInResult::Failed) };
        if let Some(early) = self.pre_sign_in(&user) {
            return Ok(early);
        }
        if !self.users().redeem_recovery_code(&user, code).await? {
            self.0.event(AccountEventKind::SignInFailed, Some(&user.id));
            if self.users().access_failed(&mut user).await? {
                return Ok(SignInResult::LockedOut);
            }
            return Ok(SignInResult::Failed);
        }
        self.finish_two_factor(&mut user, &pending, false).await
    }

    async fn finish_two_factor(&self, user: &mut User, pending: &Pending, remember_client: bool) -> Result<SignInResult> {
        self.users().reset_access_failed(user).await?;
        if remember_client {
            self.remember_two_factor_client(user)?;
        }
        let method = pending.provider.clone().unwrap_or_else(|| "mfa".into());
        self.sign_in(user, pending.remember, &method, true).await?;
        Ok(SignInResult::Succeeded)
    }

    /// Whether this browser may skip the second factor for the user (`IsTwoFactorClientRememberedAsync`).
    pub fn is_two_factor_client_remembered(&self, user: &User) -> bool {
        let Some(value) = scope::cookie(DEVICE) else { return false };
        let Some(d) = self.0.0.sealer.as_ref().and_then(|s| s.open::<Device>(DEVICE, &value)) else { return false };
        d.id == user.id && d.expires > now_secs() && crate::server::same(&d.stamp, &self.tag(user))
    }

    /// Remembers this browser for the second factor (`RememberTwoFactorClientAsync`).
    pub fn remember_two_factor_client(&self, user: &User) -> Result<()> {
        let life = self.0.0.options.session.remember_two_factor.as_secs() as i64;
        let d = Device { id: user.id.clone(), stamp: self.tag(user), expires: now_secs() + life };
        scope::set_cookie(DEVICE, Some(&self.sealer()?.seal(DEVICE, &d)), Some(life));
        Ok(())
    }

    /// Forgets this browser (`ForgetTwoFactorClientAsync`).
    pub fn forget_two_factor_client(&self) {
        scope::set_cookie(DEVICE, None, None);
    }

    /// Signs in with a linked external login (`ExternalLoginSignInAsync`). `Failed` when no
    /// user has this login.
    pub async fn external_login_sign_in(&self, provider: &str, key: &str, remember: bool) -> Result<SignInResult> {
        let Some(user) = self.users().find_by_login(provider, key).await? else { return Ok(SignInResult::Failed) };
        if let Some(early) = self.pre_sign_in(&user) {
            return Ok(early);
        }
        self.sign_in_or_two_factor(&user, remember, Some(provider)).await
    }

    /// Signs in a user who proved a passkey (`PasskeySignInAsync`). A passkey replaces the
    /// password and the second factor; with user verification it counts as multi factor.
    pub(crate) async fn passkey_sign_in(&self, user: &User, verified: bool, remember: bool) -> Result<SignInResult> {
        if let Some(early) = self.pre_sign_in(user) {
            return Ok(early);
        }
        self.sign_in(user, remember, "passkey", verified).await?;
        Ok(SignInResult::Succeeded)
    }

    /// Bearer tokens for the signed in user of this request, or for `user` (`/login` without
    /// cookies).
    pub(crate) async fn bearer(&self, user: &User, method: &str, mfa: bool) -> Result<AccessTokens> {
        let o = self.0.0.options.session;
        let now = now_secs();
        let mut s = self.session(user, false, method, mfa, now).await?;
        s.expires = now + o.bearer_expiration.as_secs() as i64;
        let refresh = Refresh { id: user.id.clone(), stamp: self.tag(user), method: method.to_string(), mfa, auth_time: now, expires: now + o.refresh_expiration.as_secs() as i64 };
        Ok(AccessTokens {
            token_type: "Bearer",
            access_token: self.sealer()?.seal(BEARER, &s),
            expires_in: o.bearer_expiration.as_secs() as i64,
            refresh_token: self.sealer()?.seal(REFRESH, &refresh),
        })
    }

    /// New bearer tokens from a refresh token whose user still has the same stamp (`/refresh`).
    /// Lockout is not checked, as in ASP.NET Core Identity.
    pub(crate) async fn refresh_bearer(&self, token: &str) -> Result<Option<AccessTokens>> {
        let Some(r) = self.sealer()?.open::<Refresh>(REFRESH, token) else { return Ok(None) };
        if r.expires <= now_secs() {
            return Ok(None);
        }
        let Some(user) = self.validate_security_stamp(&r.id, &r.stamp).await? else { return Ok(None) };
        let mut tokens = self.bearer(&user, &r.method, r.mfa).await?;
        if let Some(mut s) = self.sealer()?.open::<Session>(BEARER, &tokens.access_token) {
            s.auth_time = r.auth_time;
            tokens.access_token = self.sealer()?.seal(BEARER, &s);
        }
        Ok(Some(tokens))
    }

    /// The identity of a session: its user, the permissions of its roles and claims, and when and
    /// how it signed in.
    pub(crate) fn identity(&self, s: &Session) -> Identity {
        let mut identity = Identity::user(s.user.clone());
        identity.permissions.extend(s.permissions.iter().cloned());
        identity.permissions.extend(self.0.0.roles.of(&s.roles).cloned());
        identity.permissions.extend(self.0.0.everyone.iter().cloned());
        identity.authenticated_at = Some(DateTime::from_unix_ms(s.auth_time.saturating_mul(1000)));
        identity.mfa = s.mfa;
        identity
    }
}

/// The authenticator `App::accounts` installs: the session cookie, or a bearer access token.
pub(crate) struct AccountsAuth(pub Accounts);

impl crate::server::Authenticator for AccountsAuth {
    fn authenticate(&self, headers: &axum::http::HeaderMap) -> Option<Identity> {
        let manager = self.0.sign_in();
        if let Some(token) = bearer(headers) {
            let s: Session = self.0.0.sealer.as_ref()?.open(BEARER, token)?;
            return (s.expires > now_secs()).then(|| manager.identity(&s));
        }
        let s = manager.open_session(crate::server::cookie(headers, SESSION)?)?;
        Some(manager.identity(&s))
    }
}

/// The bearer token of an `Authorization` header.
pub(crate) fn bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    let v = headers.get(axum::http::header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = v.split_once(' ')?;
    scheme.eq_ignore_ascii_case("bearer").then_some(token.trim()).filter(|t| !t.is_empty() && t.len() <= 8192)
}
