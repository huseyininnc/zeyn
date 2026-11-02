//! Configuration from any source, secrets, tenants and licenses.
//!
//! Nothing here talks to a network by itself. Apps plug in their own sources (a database, a vault,
//! a license server, an HTTP endpoint) by implementing [`ConfigSource`] and [`LicenseSource`].

use crate::action::{Error, Result};
use crate::ctx::{BoxFuture, Ctx, Service};
use crate::feature::{Guard, GuardResult};
use crate::i18n::Text;
use crate::time::Date;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex};

/// A value that must never appear in logs: `Debug` and `Display` print `[redacted]`.
///
/// ```
/// use zeyn::Secret;
/// let s = Secret::new("postgres://user:pass@db/app".to_string());
/// assert_eq!(format!("{s:?}"), "[redacted]");
/// assert!(s.expose().starts_with("postgres://"));
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    pub fn new(value: T) -> Self {
        Secret(value)
    }

    /// The real value. Every call is a place to review.
    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// Where configuration comes from: environment, a file, a database, a vault, a license server.
///
/// Keys are lower case and dotted, such as `db.url`.
///
/// ```ignore
/// struct VaultSource { client: MyVaultClient }
/// impl ConfigSource for VaultSource {
///     fn name(&self) -> &str { "vault" }
///     fn load(&self) -> BoxFuture<std::result::Result<HashMap<String, String>, String>> {
///         let client = self.client.clone();
///         Box::pin(async move { client.read("secret/app").await.map_err(|e| e.to_string()) })
///     }
/// }
/// ```
pub trait ConfigSource: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn load(&self) -> BoxFuture<std::result::Result<HashMap<String, String>, String>>;
}

/// Environment variables with a prefix: `APP_DB__URL` becomes `db.url` for prefix `APP`.
pub struct EnvSource(pub &'static str);

impl ConfigSource for EnvSource {
    fn name(&self) -> &str {
        "environment"
    }

    fn load(&self) -> BoxFuture<std::result::Result<HashMap<String, String>, String>> {
        let prefix = format!("{}_", self.0);
        let values = std::env::vars()
            .filter_map(|(k, v)| k.strip_prefix(&prefix).map(|rest| (rest.to_lowercase().replace("__", "."), v)))
            .collect();
        Box::pin(async move { Ok(values) })
    }
}

/// A JSON file of nested objects; nested keys are joined with dots. A missing file is an error.
pub struct JsonFileSource(pub std::path::PathBuf);

impl ConfigSource for JsonFileSource {
    fn name(&self) -> &str {
        "json file"
    }

    fn load(&self) -> BoxFuture<std::result::Result<HashMap<String, String>, String>> {
        let path = self.0.clone();
        Box::pin(async move {
            let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut out = HashMap::new();
            flatten("", &json, &mut out);
            Ok(out)
        })
    }
}

/// Fixed values, for defaults and tests.
pub struct StaticSource(pub HashMap<String, String>);

impl ConfigSource for StaticSource {
    fn name(&self) -> &str {
        "static"
    }

    fn load(&self) -> BoxFuture<std::result::Result<HashMap<String, String>, String>> {
        let values = self.0.clone();
        Box::pin(async move { Ok(values) })
    }
}

fn flatten(prefix: &str, value: &serde_json::Value, out: &mut HashMap<String, String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() { k.to_lowercase() } else { format!("{prefix}.{}", k.to_lowercase()) };
                flatten(&key, v, out);
            }
        }
        serde_json::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        serde_json::Value::Null => {}
        other => {
            out.insert(prefix.to_string(), other.to_string());
        }
    }
}

/// Merged configuration. Later sources override earlier ones.
///
/// ```
/// # tokio_test_block(async {
/// use zeyn::{Config, StaticSource};
/// let base = StaticSource([("db.url".to_string(), "local".to_string())].into());
/// let prod = StaticSource([("db.url".to_string(), "prod".to_string())].into());
/// let config = Config::load(vec![Box::new(base), Box::new(prod)]).await.unwrap();
/// assert_eq!(config.get("db.url"), Some("prod"));
/// # });
/// # fn tokio_test_block<F: std::future::Future>(f: F) { let mut f = std::pin::pin!(f); let w = std::task::Waker::noop(); let mut cx = std::task::Context::from_waker(&w); loop { if let std::task::Poll::Ready(_) = f.as_mut().poll(&mut cx) { break } } }
/// ```
#[derive(Clone, Default)]
pub struct Config {
    values: Arc<HashMap<String, String>>,
    origin: Arc<HashMap<String, String>>,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut keys: Vec<&String> = self.values.keys().collect();
        keys.sort();
        write!(f, "Config {{ keys: {keys:?} }}")
    }
}

impl Service for Config {}

impl Config {
    /// Loads every source in order. Fails with the source's name when one fails.
    pub async fn load(sources: Vec<Box<dyn ConfigSource>>) -> std::result::Result<Config, String> {
        let mut values = HashMap::new();
        let mut origin = HashMap::new();
        for source in sources {
            let loaded = source.load().await.map_err(|e| format!("config source `{}`: {e}", source.name()))?;
            for (k, v) in loaded {
                origin.insert(k.clone(), source.name().to_string());
                values.insert(k, v);
            }
        }
        Ok(Config { values: Arc::new(values), origin: Arc::new(origin) })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// A required value; the error names the key, never the value.
    pub fn require(&self, key: &str) -> Result<&str> {
        self.get(key).ok_or_else(|| Error::Internal(format!("configuration `{key}` is missing")))
    }

    /// A value that must not be logged, such as a connection string or an API key.
    pub fn secret(&self, key: &str) -> Result<Secret<String>> {
        self.require(key).map(|v| Secret::new(v.to_string()))
    }

    /// Which source a key came from. Useful when checking an installation.
    pub fn source_of(&self, key: &str) -> Option<&str> {
        self.origin.get(key).map(String::as_str)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }
}

/// The tenant a request belongs to in a multi tenant app.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Tenant {
    id: String,
}

impl Tenant {
    /// A tenant id: 1 to 63 lower case letters, digits, `-` or `_`. Anything else is refused, so
    /// an id can be used in file names, cache keys and schema names without escaping.
    pub fn new(id: &str) -> Option<Tenant> {
        let ok = (1..=63).contains(&id.len())
            && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
        ok.then(|| Tenant { id: id.to_string() })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Built in guard: the request must belong to a tenant.
#[derive(Clone, Copy, Debug, Default)]
pub struct TenantRequired;

impl Guard for TenantRequired {
    fn check(&self, ctx: &Ctx) -> GuardResult {
        match ctx.tenant() {
            Some(_) => GuardResult::Allow,
            None => GuardResult::Deny(Text::key("zeyn.error.tenant")),
        }
    }

    fn name(&self) -> &'static str {
        "TenantRequired"
    }
}

type Factory<T> = Arc<dyn Fn(Tenant, Config) -> BoxFuture<Result<T>> + Send + Sync>;

/// A service that is built once per tenant and reused, such as a database pool made from the
/// tenant's own connection string.
///
/// ```ignore
/// let pools = PerTenant::new(|tenant, config| Box::pin(async move {
///     let url = config.secret(&format!("tenants.{}.db_url", tenant.id()))?;
///     Pool::connect(url.expose()).await.map_err(|e| Error::Internal(e.to_string()))
/// }));
/// App::new("Saas").service(pools)
/// ```
pub struct PerTenant<T> {
    cache: Arc<Mutex<HashMap<String, T>>>,
    make: Factory<T>,
}

impl<T> Clone for PerTenant<T> {
    fn clone(&self) -> Self {
        PerTenant { cache: self.cache.clone(), make: self.make.clone() }
    }
}

impl<T: Clone + Send + Sync + 'static> Service for PerTenant<T> {}

impl<T: Clone + Send + Sync + 'static> PerTenant<T> {
    pub fn new(make: impl Fn(Tenant, Config) -> BoxFuture<Result<T>> + Send + Sync + 'static) -> Self {
        PerTenant { cache: Arc::new(Mutex::new(HashMap::new())), make: Arc::new(make) }
    }

    /// The value for the request's tenant, built on first use.
    pub async fn get(&self, ctx: &Ctx) -> Result<T> {
        let tenant = ctx.tenant().cloned().ok_or(Error::Denied(Text::key("zeyn.error.tenant")))?;
        if let Some(v) = self.cache.lock().map_err(|_| Error::Internal("tenant cache poisoned".into()))?.get(tenant.id()) {
            return Ok(v.clone());
        }
        let value = (self.make)(tenant.clone(), ctx.config().clone()).await?;
        self.cache
            .lock()
            .map_err(|_| Error::Internal("tenant cache poisoned".into()))?
            .entry(tenant.id().to_string())
            .or_insert(value.clone());
        Ok(value)
    }

    /// Drops the cached value of a tenant, for example after its connection string changed.
    pub fn forget(&self, tenant: &Tenant) {
        if let Ok(mut c) = self.cache.lock() {
            c.remove(tenant.id());
        }
    }
}

/// A license: who holds it, until when, and which modules it opens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct License {
    pub holder: String,
    /// Last day the license is valid, inclusive, in the user's time zone.
    pub valid_until: Date,
    /// Modules the license opens. Empty means every module.
    #[serde(default)]
    pub modules: BTreeSet<String>,
    #[serde(default)]
    pub max_users: Option<u32>,
}

impl License {
    pub fn is_valid_on(&self, day: Date) -> bool {
        day <= self.valid_until
    }

    pub fn allows(&self, module: &str) -> bool {
        self.modules.is_empty() || self.modules.contains(module)
    }
}

/// Where licenses come from: a file, a license server, a vault, a database. Called with the
/// request's tenant in multi tenant apps.
pub trait LicenseSource: Send + Sync + 'static {
    fn load(&self, tenant: Option<&Tenant>) -> BoxFuture<std::result::Result<Option<License>, String>>;
}

/// A fixed license, for development and tests.
pub struct FixedLicense(pub License);

impl LicenseSource for FixedLicense {
    fn load(&self, _tenant: Option<&Tenant>) -> BoxFuture<std::result::Result<Option<License>, String>> {
        let l = self.0.clone();
        Box::pin(async move { Ok(Some(l)) })
    }
}

/// Guard: the request must have a valid license, optionally for a module.
#[derive(Clone, Debug)]
pub struct Licensed(Option<&'static str>);

impl Licensed {
    /// Any valid license.
    pub fn any() -> Self {
        Licensed(None)
    }

    /// A valid license that opens this module.
    pub fn module(name: &'static str) -> Self {
        Licensed(Some(name))
    }
}

impl Guard for Licensed {
    fn check(&self, ctx: &Ctx) -> GuardResult {
        match ctx.license() {
            Some(l) if !l.is_valid_on(ctx.today()) => GuardResult::Deny(Text::key("zeyn.error.license_expired")),
            Some(l) if self.0.is_none_or(|m| l.allows(m)) => GuardResult::Allow,
            _ => GuardResult::Deny(Text::key("zeyn.error.license")),
        }
    }

    fn name(&self) -> &'static str {
        "Licensed"
    }
}

/// What a signed license signs: this prefix, then the payload text exactly as stored.
#[cfg(feature = "signed-licenses")]
const LICENSE_CONTEXT: &[u8] = b"zeyn-license-v1\n";

/// Why a signed license was refused. Every case leaves the app locked.
#[cfg(feature = "signed-licenses")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LicenseError {
    /// The public key in the app is not 64 hex digits of a valid ed25519 key.
    InvalidPublicKey,
    /// The file is not a signed license: not JSON, no `payload` or `signature`, or a payload
    /// that is not a license.
    Malformed(String),
    /// The signature does not match: the file was changed after signing, or it was signed with
    /// another key than the one the app trusts.
    BadSignature,
}

#[cfg(feature = "signed-licenses")]
impl fmt::Display for LicenseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LicenseError::InvalidPublicKey => f.write_str("the license public key in the app is not a valid ed25519 key (64 hex digits)"),
            LicenseError::Malformed(why) => write!(f, "the license file is not a signed license: {why}"),
            LicenseError::BadSignature => {
                f.write_str("the license signature does not match: the file was changed after signing or was signed with another key")
            }
        }
    }
}

#[cfg(feature = "signed-licenses")]
impl std::error::Error for LicenseError {}

#[cfg(feature = "signed-licenses")]
fn from_hex<const N: usize>(hex: &str) -> Option<[u8; N]> {
    let hex = hex.trim().as_bytes();
    if hex.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (i, pair) in hex.chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(out)
}

#[cfg(feature = "signed-licenses")]
impl License {
    /// Reads a license file made by `zeyn license sign` and checks its ed25519 signature against
    /// the public key built into the app. Expiry is not checked here: the [`Licensed`] guard
    /// compares `valid_until` with the user's today on every request.
    ///
    /// The file is JSON: `{"payload": "<license JSON as text>", "signature": "<128 hex digits>"}`.
    /// The payload stays text so the signed bytes are exactly the stored bytes.
    pub fn from_signed(file: &str, public_key: &str) -> std::result::Result<License, LicenseError> {
        let key = from_hex::<32>(public_key)
            .and_then(|k| ed25519_dalek::VerifyingKey::from_bytes(&k).ok())
            .ok_or(LicenseError::InvalidPublicKey)?;
        #[derive(Deserialize)]
        struct Signed {
            payload: String,
            signature: String,
        }
        let signed: Signed = serde_json::from_str(file).map_err(|e| LicenseError::Malformed(e.to_string()))?;
        let signature = from_hex::<64>(&signed.signature)
            .ok_or_else(|| LicenseError::Malformed("`signature` is not 128 hex digits".into()))?;
        let mut message = Vec::with_capacity(LICENSE_CONTEXT.len() + signed.payload.len());
        message.extend_from_slice(LICENSE_CONTEXT);
        message.extend_from_slice(signed.payload.as_bytes());
        key.verify_strict(&message, &ed25519_dalek::Signature::from_bytes(&signature))
            .map_err(|_| LicenseError::BadSignature)?;
        serde_json::from_str(&signed.payload).map_err(|e| LicenseError::Malformed(format!("payload: {e}")))
    }
}

/// A signed license file on disk, checked against the public key built into the app. Read on
/// every refresh, so a renewed file is picked up without a restart.
///
/// A file that fails the check (changed, signed with another key, not a license) locks the app
/// and logs why; a file that cannot be read keeps the last license, like any failing source.
///
/// ```ignore
/// const LICENSE_KEY: &str = "3b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29";
/// App::new("Shop").license(SignedLicenseFile::new("license.json", LICENSE_KEY), Duration::from_secs(3600))
/// ```
#[cfg(feature = "signed-licenses")]
pub struct SignedLicenseFile {
    path: std::path::PathBuf,
    public_key: &'static str,
}

#[cfg(feature = "signed-licenses")]
impl SignedLicenseFile {
    /// `public_key` is the 64 hex digits `zeyn license keygen` printed.
    pub fn new(path: impl Into<std::path::PathBuf>, public_key: &'static str) -> Self {
        SignedLicenseFile { path: path.into(), public_key }
    }
}

#[cfg(feature = "signed-licenses")]
impl LicenseSource for SignedLicenseFile {
    fn load(&self, _tenant: Option<&Tenant>) -> BoxFuture<std::result::Result<Option<License>, String>> {
        let result = std::fs::read_to_string(&self.path).map_err(|e| format!("{}: {e}", self.path.display())).map(|text| {
            License::from_signed(&text, self.public_key)
                .inspect_err(|e| tracing::error!("license {} refused, the app is locked: {e}", self.path.display()))
                .ok()
        });
        Box::pin(async move { result })
    }
}
