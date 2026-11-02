//! Optional user accounts with the structure of ASP.NET Core Identity (Cargo feature
//! `accounts`): users and roles in an [`AccountStore`], a [`UserManager`], a [`RoleManager`] and
//! a [`SignInManager`], options like `IdentityOptions`, Argon2id passwords that also accept
//! migrated .NET hashes, purpose tokens, a sealed session cookie with security stamp validation,
//! two factor with authenticator apps, e-mail codes, recovery codes and passkeys, external logins
//! through [`crate::OidcAuth`] providers, JSON endpoints like `MapIdentityApi`, and a default UI
//! ([`feature`]). See `docs/accounts.md`.
//!
//! ```no_run
//! use zeyn::prelude::*;
//! use zeyn::accounts::{self, Accounts, MemoryAccountStore};
//!
//! # fn key() -> zeyn::Secret<String> { zeyn::Secret::new("0123456789abcdef0123456789abcdef".into()) }
//! let accounts = Accounts::new(MemoryAccountStore::new(), key())
//!     .options(|o| o.sign_in.require_confirmed_email = true)
//!     .role("admins", ["admin.users", "admin.roles"]);
//! let app = App::new("Shop").accounts(accounts).feature(accounts::feature());
//! ```

mod manager;
mod model;
pub(crate) mod passkey;
mod password;
pub(crate) mod scope;
pub(crate) mod signin;
#[cfg(any(feature = "postgres", feature = "oracle"))]
mod sql;
mod store;
mod tokens;
mod ui;

pub use manager::{RoleManager, UserManager};
pub use model::{AUTHENTICATOR_KEY, Claim, INTERNAL_PROVIDER, Passkey, RECOVERY_CODES, Role, User, UserLogin};
pub use passkey::PasskeyOptions;
pub use password::{BreachedPasswords, MAX_PASSWORD_BYTES, PasswordDenyList, PasswordHasher, PasswordOptions, Verification, dotnet};
pub use signin::{SignInManager, SignInResult, TwoFactorKind};
#[cfg(any(feature = "postgres", feature = "oracle"))]
pub use sql::{SqlAccountStore, create_tables};
pub use store::{AccountStore, MemoryAccountStore};
pub use tokens::Purpose;
pub use ui::{AccountsUi, ROLES_WRITE, USERS_READ, USERS_WRITE, feature, ui};


use crate::ctx::{BoxFuture, Ctx, Service};
use crate::i18n::Text;
use crate::model::{FieldError, Rule, ValidationErrors};
use crate::platform::Secret;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Lockout rules (`LockoutOptions`): five failed attempts lock an account for five minutes, and
/// new users can be locked out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LockoutOptions {
    pub allowed_for_new_users: bool,
    pub max_failed_access_attempts: i32,
    pub default_lockout: Duration,
}

impl Default for LockoutOptions {
    fn default() -> Self {
        LockoutOptions { allowed_for_new_users: true, max_failed_access_attempts: 5, default_lockout: Duration::from_secs(300) }
    }
}

/// User name and e-mail rules (`UserOptions`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserOptions {
    /// Characters a user name may contain; empty allows all. ASP.NET Core Identity's default set.
    pub allowed_user_name_characters: String,
    /// Each user needs a unique e-mail address. Off by default, like ASP.NET Core Identity; the
    /// default UI registers with the e-mail address as the user name, which is unique anyway.
    pub require_unique_email: bool,
}

impl Default for UserOptions {
    fn default() -> Self {
        UserOptions {
            allowed_user_name_characters: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._@+".into(),
            require_unique_email: false,
        }
    }
}

/// Conditions for signing in (`SignInOptions`). All off by default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SignInOptions {
    pub require_confirmed_email: bool,
    pub require_confirmed_phone_number: bool,
}

/// Lifetimes of tokens and codes (`DataProtectionTokenProviderOptions`, `TokenOptions`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenOptions {
    /// How long e-mail confirmation, password reset and e-mail change links work. One day.
    pub token_lifespan: Duration,
    /// Issuer shown in authenticator apps.
    pub authenticator_issuer: String,
}

impl Default for TokenOptions {
    fn default() -> Self {
        TokenOptions { token_lifespan: Duration::from_secs(24 * 3600), authenticator_issuer: "Zeyn".into() }
    }
}

/// Session cookie and bearer token settings (`CookieAuthenticationOptions`,
/// `SecurityStampValidatorOptions`, `BearerTokenOptions`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionOptions {
    /// How long a session lasts without use; renewed (sliding) when it is checked. Fourteen days.
    pub expire: Duration,
    /// How often the security stamp is checked against the store. Thirty minutes.
    pub validation_interval: Duration,
    /// How long a remembered device skips the second factor. Fourteen days.
    pub remember_two_factor: Duration,
    /// Lifetime of bearer access tokens. One hour.
    pub bearer_expiration: Duration,
    /// Lifetime of bearer refresh tokens. Fourteen days.
    pub refresh_expiration: Duration,
}

impl Default for SessionOptions {
    fn default() -> Self {
        SessionOptions {
            expire: Duration::from_secs(14 * 24 * 3600),
            validation_interval: Duration::from_secs(30 * 60),
            remember_two_factor: Duration::from_secs(14 * 24 * 3600),
            bearer_expiration: Duration::from_secs(3600),
            refresh_expiration: Duration::from_secs(14 * 24 * 3600),
        }
    }
}

/// Every setting of the accounts system (`IdentityOptions`). The defaults are ASP.NET Core
/// Identity's, except the password hash (Argon2id).
#[derive(Clone, Debug, PartialEq)]
pub struct AccountOptions {
    pub password: PasswordOptions,
    pub lockout: LockoutOptions,
    pub user: UserOptions,
    pub sign_in: SignInOptions,
    pub tokens: TokenOptions,
    pub session: SessionOptions,
    pub hasher: PasswordHasher,
    pub passkeys: PasskeyOptions,
    /// Attempts per window for one client address on sign in, registration, password reset and
    /// code checks, through the app's rate store. Thirty per five minutes.
    pub ip_attempts: (u32, Duration),
    /// Attempts per window for one account (by the name typed). Ten per five minutes.
    pub account_attempts: (u32, Duration),
    /// Self registration is open. On by default; `AccountsUi::registration(false)` also hides it.
    pub registration: bool,
}

impl Default for AccountOptions {
    fn default() -> Self {
        AccountOptions {
            password: PasswordOptions::default(),
            lockout: LockoutOptions::default(),
            user: UserOptions::default(),
            sign_in: SignInOptions::default(),
            tokens: TokenOptions::default(),
            session: SessionOptions::default(),
            hasher: PasswordHasher::default(),
            passkeys: PasskeyOptions::default(),
            ip_attempts: (30, Duration::from_secs(300)),
            account_attempts: (10, Duration::from_secs(300)),
            registration: true,
        }
    }
}

/// The culture invariant normal form of a user name, e-mail or role name used for lookups
/// (`UpperInvariantLookupNormalizer`): Unicode NFC, then upper case by the Unicode default
/// mapping, never a language's rules. The Turkish dotless `ı` and the ASCII `i` both become
/// `I`, so `ali` and `alı` are the same name and look alike names cannot both be registered; the
/// dotted capital `İ` stays `İ`.
///
/// ```
/// use zeyn::accounts::normalize;
/// assert_eq!(normalize("admin@example.com"), "ADMIN@EXAMPLE.COM");
/// assert_eq!(normalize("kılıç"), normalize("KILIÇ"));
/// assert_eq!(normalize("ali"), normalize("alı"));
/// assert_eq!(normalize("i\u{307}"), "İ");
/// ```
pub fn normalize(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    name.trim().nfc().collect::<String>().to_uppercase().nfc().collect()
}

/// Random bytes from the operating system.
pub(crate) fn random_bytes<const N: usize>() -> [u8; N] {
    crate::auth::random::<N>()
}

/// A new random id in the GUID text form ASP.NET Core Identity uses.
pub(crate) fn new_id() -> String {
    let b = random_bytes::<16>();
    let h = tokens::hex(&b);
    format!("{}-{}-4{}-{}-{}", &h[0..8], &h[8..12], &h[13..16], &h[16..20], &h[20..32])
}

/// A new security or concurrency stamp: 160 random bits in base32, like ASP.NET Core Identity.
pub(crate) fn new_stamp() -> String {
    crate::auth::totp::base32(&random_bytes::<20>())
}

pub(crate) fn now_secs() -> i64 {
    crate::auth::now_secs()
}

/// A validation error with a catalog message `zeyn.accounts.error.<code>` for `field`, as the
/// forms show it next to the field and the JSON endpoints report it by code.
pub(crate) fn invalid(field: &'static str, code: &str) -> crate::Error {
    invalid_n(field, code, None)
}

pub(crate) fn invalid_n(field: &'static str, code: &str, n: Option<usize>) -> crate::Error {
    let mut text = Text::key(format!("zeyn.accounts.error.{code}"));
    if let Some(n) = n {
        text = text.arg("n", n.to_string());
    }
    crate::Error::Validation(ValidationErrors { errors: vec![FieldError { field, rule: Rule::Custom, message: text }] })
}

/// A refusal shown as a message, from `zeyn.accounts.error.<code>`.
pub(crate) fn denied(code: &str) -> crate::Error {
    crate::Error::Denied(Text::key(format!("zeyn.accounts.error.{code}")))
}

/// Which permissions a role grants. Role claims of type `permission` in the store grant more;
/// see [`RoleManager::add_claim`].
#[derive(Clone, Debug, Default)]
pub struct RolePermissions(BTreeMap<String, BTreeSet<String>>);

impl RolePermissions {
    /// Permissions of holders of `role` (matched on its normalized name).
    pub fn role<I: IntoIterator<Item = S>, S: Into<String>>(mut self, role: &str, permissions: I) -> Self {
        self.0.entry(normalize(role)).or_default().extend(permissions.into_iter().map(Into::into));
        self
    }

    pub(crate) fn of(&self, roles: &[String]) -> impl Iterator<Item = &String> {
        roles.iter().filter_map(|r| self.0.get(&normalize(r))).flatten()
    }
}

/// A message the accounts system sends to a user. Links and codes are secrets: senders must
/// never log them.
#[derive(Clone, PartialEq, Eq)]
pub enum AccountMessage {
    /// Confirm this e-mail address by opening `link`.
    ConfirmEmail { link: String },
    /// Reset the password by opening `link`.
    ResetPassword { link: String },
    /// Confirm the new address by opening `link`.
    ChangeEmail { link: String },
    /// Someone tried to register with an address that has an account: sent instead of a new
    /// confirmation, so registration answers the same either way.
    AlreadyRegistered { link: String },
    /// A six digit sign in code.
    TwoFactorCode { code: String },
    /// A code that confirms a new phone number.
    PhoneCode { code: String },
}

impl std::fmt::Debug for AccountMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AccountMessage::ConfirmEmail { .. } => "ConfirmEmail([redacted])",
            AccountMessage::ResetPassword { .. } => "ResetPassword([redacted])",
            AccountMessage::ChangeEmail { .. } => "ChangeEmail([redacted])",
            AccountMessage::AlreadyRegistered { .. } => "AlreadyRegistered([redacted])",
            AccountMessage::TwoFactorCode { .. } => "TwoFactorCode([redacted])",
            AccountMessage::PhoneCode { .. } => "PhoneCode([redacted])",
        })
    }
}

/// Sends account messages by e-mail or text message (`IEmailSender`). `to` is the address or
/// number. Sending should only queue, so the time a request takes does not tell whether an
/// account exists.
pub trait AccountSender: Send + Sync + 'static {
    fn send(&self, ctx: &Ctx, to: &str, message: AccountMessage) -> BoxFuture<crate::Result<()>>;
}

/// Drops every message with a warning that names only its kind. The default when no sender is
/// set; e-mail confirmation and password reset do not work with it.
pub struct NoSender;

impl AccountSender for NoSender {
    fn send(&self, _ctx: &Ctx, _to: &str, message: AccountMessage) -> BoxFuture<crate::Result<()>> {
        tracing::warn!(?message, "accounts: no sender is set, the message was dropped");
        Box::pin(async { Ok(()) })
    }
}

/// Keeps messages in memory for tests: the last 100 with their recipient.
#[derive(Clone, Default)]
pub struct MemorySender(Arc<Mutex<Vec<(String, AccountMessage)>>>);

impl MemorySender {
    pub fn messages(&self) -> Vec<(String, AccountMessage)> {
        self.0.lock().map(|m| m.clone()).unwrap_or_default()
    }

    /// The last link or code sent to `to`.
    pub fn last(&self, to: &str) -> Option<String> {
        self.messages().into_iter().rev().find(|(t, _)| t == to).map(|(_, m)| match m {
            AccountMessage::ConfirmEmail { link } | AccountMessage::ResetPassword { link } | AccountMessage::ChangeEmail { link } | AccountMessage::AlreadyRegistered { link } => link,
            AccountMessage::TwoFactorCode { code } | AccountMessage::PhoneCode { code } => code,
        })
    }
}

impl AccountSender for MemorySender {
    fn send(&self, _ctx: &Ctx, to: &str, message: AccountMessage) -> BoxFuture<crate::Result<()>> {
        if let Ok(mut m) = self.0.lock() {
            if m.len() >= 100 {
                m.remove(0);
            }
            m.push((to.to_string(), message));
        }
        Box::pin(async { Ok(()) })
    }
}

/// Sends account e-mail through the app's [`crate::Mailer`] with the built in templates
/// `zeyn.accounts.mail.*` in the user's language. Text messages (phone codes) are dropped with a
/// warning; set a sender of your own for them.
#[cfg(feature = "email")]
pub struct MailSender(pub crate::Mailer);

#[cfg(feature = "email")]
impl AccountSender for MailSender {
    fn send(&self, ctx: &Ctx, to: &str, message: AccountMessage) -> BoxFuture<crate::Result<()>> {
        let (template, name, value) = match message {
            AccountMessage::ConfirmEmail { link } => ("confirm", "link", link),
            AccountMessage::ResetPassword { link } => ("reset", "link", link),
            AccountMessage::ChangeEmail { link } => ("change", "link", link),
            AccountMessage::AlreadyRegistered { link } => ("exists", "link", link),
            AccountMessage::TwoFactorCode { code } => ("code", "code", code),
            AccountMessage::PhoneCode { .. } => {
                tracing::warn!("accounts: MailSender does not send text messages");
                return Box::pin(async { Ok(()) });
            }
        };
        let mail = crate::Mail::to(to).template(format!("zeyn.accounts.mail.{template}")).arg(name, value);
        let sent = self.0.send(ctx, mail);
        Box::pin(async move { sent })
    }
}

/// What an [`AccountEvent`] records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountEventKind {
    SignedIn,
    SignInFailed,
    LockedOut,
    SignedOut,
    Registered,
    EmailConfirmed,
    PasswordChanged,
    PasswordReset,
    TwoFactorEnabled,
    TwoFactorDisabled,
    RecoveryCodesRenewed,
    RecoveryCodeUsed,
    PasskeyAdded,
    PasskeyRemoved,
    LoginLinked,
    LoginUnlinked,
    StampChanged,
    Deleted,
    RateLimited,
}

/// A security event for the audit trail. Never holds a password, hash, token or code.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct AccountEvent {
    pub at: i64,
    pub kind: AccountEventKind,
    /// The user's id, when one is known.
    pub user: Option<String>,
    /// The client address, when the event comes from a request.
    pub ip: Option<String>,
}

/// Where account events go. Without one they are written to the log (target `zeyn::accounts`).
pub trait AccountEvents: Send + Sync + 'static {
    fn record(&self, event: AccountEvent);
}

/// The last 1 000 events in memory, for tests and demos.
#[derive(Clone, Default)]
pub struct MemoryAccountEvents(Arc<Mutex<Vec<AccountEvent>>>);

impl MemoryAccountEvents {
    pub fn events(&self) -> Vec<AccountEvent> {
        self.0.lock().map(|e| e.clone()).unwrap_or_default()
    }
}

impl AccountEvents for MemoryAccountEvents {
    fn record(&self, event: AccountEvent) {
        if let Ok(mut e) = self.0.lock() {
            if e.len() >= 1000 {
                e.remove(0);
            }
            e.push(event);
        }
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

pub(crate) struct Core {
    #[cfg(feature = "oidc")]
    pub externals: Vec<(String, Arc<crate::auth::OidcAuth>)>,
    pub store: Arc<dyn AccountStore>,
    pub options: AccountOptions,
    pub keys: tokens::Keys,
    pub sealer: Option<crate::auth::seal::Sealer>,
    pub sender: Arc<dyn AccountSender>,
    pub events: Option<Arc<dyn AccountEvents>>,
    pub breached: Option<Arc<dyn BreachedPasswords>>,
    pub roles: RolePermissions,
    pub public_url: Option<String>,
    /// A hash of a random password, checked when no user matches so a missing account costs as
    /// much time as a wrong password.
    pub dummy_hash: String,
}

/// The accounts system of an app: its store, keys and options. Cheap to clone. Hand it to
/// `App::accounts`, which registers it and the managers as services.
#[derive(Clone)]
pub struct Accounts(pub(crate) Arc<Core>);

impl Service for Accounts {}

impl Accounts {
    /// Accounts in `store`. `key` (at least 32 bytes, from configuration) seals the session
    /// cookies and the stored authenticator keys and signs tokens. It must stay the same:
    /// changing it signs everyone out, voids every link and code and makes stored authenticator
    /// keys unreadable.
    pub fn new(store: impl AccountStore, key: Secret<String>) -> Accounts {
        let options = AccountOptions::default();
        let dummy = format!("dummy-{}", tokens::hex(&random_bytes::<16>()));
        let dummy_hash = options.hasher.hash(&dummy).unwrap_or_default();
        Accounts(Arc::new(Core {
            #[cfg(feature = "oidc")]
            externals: Vec::new(),
            store: Arc::new(store),
            keys: tokens::Keys::new(key.expose().as_bytes()),
            sealer: crate::auth::seal::Sealer::new(&key),
            options,
            sender: Arc::new(NoSender),
            events: None,
            breached: None,
            roles: RolePermissions::default(),
            public_url: None,
            dummy_hash,
        }))
    }

    fn change(self, f: impl FnOnce(&mut Core)) -> Accounts {
        let mut core = Arc::try_unwrap(self.0).unwrap_or_else(|_| panic!("configure Accounts before cloning it"));
        f(&mut core);
        Accounts(Arc::new(core))
    }

    /// Changes options, starting from the defaults.
    pub fn options(self, f: impl FnOnce(&mut AccountOptions)) -> Accounts {
        self.change(|c| {
            f(&mut c.options);
            c.dummy_hash = c.options.hasher.hash("zeyn-dummy-password").unwrap_or_default();
        })
    }

    /// Sends confirmation links, reset links and codes (see [`MailSender`]).
    pub fn sender(self, sender: impl AccountSender) -> Accounts {
        self.change(|c| c.sender = Arc::new(sender))
    }

    /// Records security events (sign in, failures, lockouts, password and two factor changes).
    pub fn events(self, events: impl AccountEvents) -> Accounts {
        self.change(|c| c.events = Some(Arc::new(events)))
    }

    /// Refuses passwords found in a breach list (see [`BreachedPasswords`]).
    pub fn breached_passwords(self, check: impl BreachedPasswords) -> Accounts {
        self.change(|c| c.breached = Some(Arc::new(check)))
    }

    /// Permissions of the holders of a role.
    pub fn role<I: IntoIterator<Item = S>, S: Into<String>>(self, role: &str, permissions: I) -> Accounts {
        self.change(|c| c.roles = std::mem::take(&mut c.roles).role(role, permissions))
    }

    /// The app's address as users see it (`https://shop.example.com`), for links in messages and
    /// the passkey relying party. Never taken from the request's `Host` header.
    pub fn public_url(self, url: &str) -> Accounts {
        self.change(|c| c.public_url = Some(url.trim_end_matches('/').to_string()))
    }

    /// Lets users sign in with an OpenID Connect provider and link it to their account
    /// (`AspNetUserLogins`). `name` (letters, digits, `-` and `_`) names the provider in the
    /// login table and the routes; register `<public url>/_zeyn/account/external/<name>/callback`
    /// as its redirect URI. The provider's `sub` is the login key.
    #[cfg(feature = "oidc")]
    pub fn external(self, name: &str, mut provider: crate::auth::OidcAuth) -> Accounts {
        provider.finish();
        let name = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(32).collect::<String>();
        self.change(|c| c.externals.push((name, Arc::new(provider))))
    }

    /// The external provider called `name`.
    #[cfg(feature = "oidc")]
    pub(crate) fn external_provider(&self, name: &str) -> Option<Arc<crate::auth::OidcAuth>> {
        self.0.externals.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
    }

    /// Names of the external providers.
    pub fn external_names(&self) -> Vec<String> {
        #[cfg(feature = "oidc")]
        return self.0.externals.iter().map(|(n, _)| n.clone()).collect();
        #[cfg(not(feature = "oidc"))]
        Vec::new()
    }

    pub fn store(&self) -> &dyn AccountStore {
        &*self.0.store
    }

    pub fn settings(&self) -> &AccountOptions {
        &self.0.options
    }

    /// The user manager.
    pub fn users(&self) -> UserManager {
        UserManager(self.clone())
    }

    /// The role manager.
    pub fn roles(&self) -> RoleManager {
        RoleManager(self.clone())
    }

    /// The sign in manager.
    pub fn sign_in(&self) -> SignInManager {
        SignInManager(self.clone())
    }

    pub(crate) fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.0.sealer.is_none() {
            problems.push(format!("Accounts: the key must be at least {} bytes", crate::auth::seal::MIN_SECRET));
        }
        if self.0.public_url.as_deref().is_some_and(|u| !(u.starts_with("https://") || u.starts_with("http://"))) {
            problems.push("Accounts: the public URL must start with https:// (or http:// for local development)".into());
        }
        problems
    }

    pub(crate) fn event(&self, kind: AccountEventKind, user: Option<&str>) {
        let event = AccountEvent { at: now_secs(), kind, user: user.map(str::to_string), ip: scope::ip().map(|i| i.to_string()) };
        match &self.0.events {
            Some(sink) => sink.record(event),
            None => tracing::info!(target: "zeyn::accounts", kind = ?event.kind, user = event.user.as_deref().unwrap_or(""), ip = event.ip.as_deref().unwrap_or(""), "account event"),
        }
    }

    /// An absolute link to an app path for messages: the public URL when set, else the path.
    pub(crate) fn link(&self, path: &str) -> String {
        format!("{}{}{path}", self.0.public_url.as_deref().unwrap_or(""), scope::prefix())
    }
}
