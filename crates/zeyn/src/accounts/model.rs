//! The records of the accounts system: users, roles, claims, external logins, tokens and
//! passkeys. They mirror the tables of ASP.NET Core Identity (`AspNetUsers`, `AspNetRoles`,
//! `AspNetUserRoles`, `AspNetUserClaims`, `AspNetRoleClaims`, `AspNetUserLogins`,
//! `AspNetUserTokens`, `AspNetUserPasskeys`) with neutral names.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};

/// A user account. The fields follow `IdentityUser`; `extra` holds the app's own fields.
///
/// Secrets (`password_hash`, `security_stamp`) never show in `Debug` output, and no screen, API
/// answer, export or AI tool of the default UI ever carries them.
///
/// Apps add their own fields with a plain struct stored in `extra`:
///
/// ```
/// use zeyn::accounts::User;
/// #[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
/// struct Profile { display_name: String, department: Option<String> }
///
/// let mut user = User::new("ada@example.com");
/// user.set_data(&Profile { display_name: "Ada".into(), department: None }).unwrap();
/// let p: Profile = user.data().unwrap();
/// assert_eq!(p.display_name, "Ada");
/// ```
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub user_name: String,
    pub normalized_user_name: String,
    pub email: Option<String>,
    pub normalized_email: Option<String>,
    pub email_confirmed: bool,
    /// Argon2id PHC string, or a migrated ASP.NET Core Identity v2/v3 hash until the next sign in.
    pub password_hash: Option<String>,
    /// Changes whenever credentials change; sessions and tokens issued before are no longer valid.
    pub security_stamp: String,
    /// Changes on every save; a save with a stale stamp is a conflict.
    pub concurrency_stamp: String,
    pub phone_number: Option<String>,
    pub phone_number_confirmed: bool,
    pub two_factor_enabled: bool,
    /// Unix seconds until which sign in is refused.
    pub lockout_end: Option<i64>,
    pub lockout_enabled: bool,
    pub access_failed_count: i32,
    /// The app's own fields, as a JSON object (see [`User::data`]).
    #[serde(default)]
    pub extra: Map<String, Json>,
}

impl std::fmt::Debug for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("User")
            .field("id", &self.id)
            .field("user_name", &self.user_name)
            .field("email_confirmed", &self.email_confirmed)
            .field("password_hash", &self.password_hash.as_ref().map(|_| "[redacted]"))
            .field("security_stamp", &"[redacted]")
            .field("two_factor_enabled", &self.two_factor_enabled)
            .field("lockout_end", &self.lockout_end)
            .field("access_failed_count", &self.access_failed_count)
            .finish_non_exhaustive()
    }
}

impl User {
    /// A new user with this user name. The manager fills the id, stamps and normalized names when
    /// it creates the user.
    pub fn new(user_name: &str) -> User {
        User { user_name: user_name.to_string(), ..User::default() }
    }

    /// With this e-mail address.
    pub fn with_email(mut self, email: &str) -> User {
        self.email = Some(email.to_string());
        self
    }

    /// The app's own fields read into `X`. Missing fields take their serde defaults.
    pub fn data<X: DeserializeOwned>(&self) -> crate::Result<X> {
        serde_json::from_value(Json::Object(self.extra.clone()))
            .map_err(|e| crate::Error::Internal(format!("user data: {e}")))
    }

    /// Stores the app's own fields. `X` must serialize to a JSON object.
    pub fn set_data<X: Serialize>(&mut self, data: &X) -> crate::Result<()> {
        match serde_json::to_value(data) {
            Ok(Json::Object(map)) => {
                self.extra = map;
                Ok(())
            }
            _ => Err(crate::Error::Internal("user data must serialize to a JSON object".into())),
        }
    }

    /// Whether the user is locked out at `now` (Unix seconds).
    pub fn is_locked_out(&self, now: i64) -> bool {
        self.lockout_enabled && self.lockout_end.is_some_and(|end| end > now)
    }
}

/// A role (`IdentityRole`). Roles map to permissions through [`super::RolePermissions`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub normalized_name: String,
    pub concurrency_stamp: String,
}

/// A claim of a user or a role (`IdentityUserClaim`, `IdentityRoleClaim`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Claim {
    pub kind: String,
    pub value: String,
}

impl Claim {
    pub fn new(kind: &str, value: &str) -> Claim {
        Claim { kind: kind.to_string(), value: value.to_string() }
    }
}

/// A sign in at an external provider linked to a local user (`IdentityUserLogin`,
/// `UserLoginInfo`). `provider_key` is the provider's subject for the user.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserLogin {
    pub provider: String,
    pub provider_key: String,
    pub display_name: Option<String>,
}

/// A WebAuthn credential (`IdentityUserPasskey`, `UserPasskeyInfo`).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Passkey {
    /// Credential id, at most 1023 bytes.
    pub credential_id: Vec<u8>,
    /// COSE public key as the authenticator sent it.
    pub public_key: Vec<u8>,
    pub name: Option<String>,
    /// Unix seconds.
    pub created_at: i64,
    pub sign_count: u32,
    pub transports: Vec<String>,
    pub user_verified: bool,
    pub backup_eligible: bool,
    pub backed_up: bool,
}

impl std::fmt::Debug for Passkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Passkey")
            .field("name", &self.name)
            .field("created_at", &self.created_at)
            .field("sign_count", &self.sign_count)
            .finish_non_exhaustive()
    }
}

/// Provider name of the values the accounts system keeps in the token table itself, like
/// `[AspNetUserStore]` in ASP.NET Core Identity.
pub const INTERNAL_PROVIDER: &str = "[zeyn]";
/// Token name of the sealed authenticator key.
pub const AUTHENTICATOR_KEY: &str = "authenticator_key";
/// Token name of the keyed hashes of the unused recovery codes, separated by `;`.
pub const RECOVERY_CODES: &str = "recovery_codes";
/// Token name of the last accepted authenticator time step (replay protection).
pub const AUTHENTICATOR_STEP: &str = "authenticator_step";
