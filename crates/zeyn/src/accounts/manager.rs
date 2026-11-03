//! [`UserManager`] and [`RoleManager`]: the rules around the store (`UserManager<TUser>`,
//! `RoleManager<TRole>` with `UserValidator`, `PasswordValidator` and `RoleValidator`).

use super::model::{
    AUTHENTICATOR_KEY, AUTHENTICATOR_STEP, Claim, INTERNAL_PROVIDER, RECOVERY_CODES, Role, User,
    UserLogin,
};
use super::password::Verification;
use super::tokens::{self, Purpose};
use super::{
    AccountEventKind, Accounts, denied, invalid, invalid_n, new_id, new_stamp, normalize, now_secs,
};
use crate::ctx::Service;
use crate::{Error, Result};

/// Longest user name, e-mail, phone number or role name, like the 256 character columns of
/// ASP.NET Core Identity.
pub const MAX_NAME: usize = 256;

/// Most password hashes computed at once. With the default Argon2id parameters (19 MiB each)
/// hashing never holds more than about 76 MiB, whatever the number of sign ins.
pub const MAX_HASHING: usize = 4;

static HASHING: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(MAX_HASHING);

/// Users: creation and validation, passwords, e-mail and phone confirmation, roles, claims,
/// lockout, security stamps, tokens, two factor and external logins (`UserManager<TUser>`).
/// Take it as an action argument; `App::accounts` registers it.
#[derive(Clone)]
pub struct UserManager(pub(crate) Accounts);

impl Service for UserManager {}

/// Roles and their claims (`RoleManager<TRole>`).
#[derive(Clone)]
pub struct RoleManager(pub(crate) Accounts);

impl Service for RoleManager {}

fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.rsplit_once('@') else {
        return false;
    };
    !local.is_empty()
        && !domain.is_empty()
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && email.len() <= MAX_NAME
        && !email
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '<' || c == '>')
}

impl UserManager {
    fn store(&self) -> &dyn super::AccountStore {
        &*self.0.0.store
    }

    fn options(&self) -> &super::AccountOptions {
        &self.0.0.options
    }

    /// The accounts system this manager belongs to.
    pub fn accounts(&self) -> &Accounts {
        &self.0
    }

    /// Checks a user's name and e-mail (`UserValidator`): a user name that is not blank, uses only
    /// the allowed characters and is not taken; with `require_unique_email`, a valid e-mail that
    /// no other user has.
    pub async fn validate(&self, user: &User) -> Result<()> {
        let name = user.user_name.trim();
        let allowed = &self
            .options()
            .user
            .allowed_user_name_characters;
        if name.is_empty()
            || name.chars().count() > MAX_NAME
            || (!allowed.is_empty()
                && name
                    .chars()
                    .any(|c| !allowed.contains(c)))
        {
            return Err(invalid("user_name", "invalid_user_name"));
        }
        if let Some(owner) = self
            .store()
            .user_by_name(&normalize(name))
            .await?
            && owner.id != user.id
        {
            return Err(invalid("user_name", "duplicate_user_name"));
        }
        if let Some(email) = &user.email
            && !valid_email(email)
        {
            return Err(invalid("email", "invalid_email"));
        }
        if self.options().user.require_unique_email {
            let Some(email) = &user.email else {
                return Err(invalid("email", "invalid_email"));
            };
            if let Some(owner) = self
                .store()
                .user_by_email(&normalize(email))
                .await?
                && owner.id != user.id
            {
                return Err(invalid("email", "duplicate_email"));
            }
        }
        if user
            .phone_number
            .as_deref()
            .is_some_and(|p| p.len() > MAX_NAME)
        {
            return Err(invalid("phone_number", "invalid_phone"));
        }
        Ok(())
    }

    /// Checks a password against the policy and, when set, the breached password check
    /// (`PasswordValidator`). Errors name the field `password`.
    pub async fn validate_password(&self, password: &str) -> Result<()> {
        if let Some((code, n)) = self
            .options()
            .password
            .check(password)
            .into_iter()
            .next()
        {
            return Err(invalid_n("password", code, n));
        }
        if let Some(check) = &self.0.0.breached {
            match check.is_breached(password).await {
                Ok(true) => return Err(invalid("password", "password_breached")),
                Ok(false) => {}
                Err(e) => tracing::warn!("breached password check failed, allowing: {e}"),
            }
        }
        Ok(())
    }

    /// Creates a user, with a password when given. Fills the id, stamps and normalized names and
    /// turns lockout on when `allowed_for_new_users` (`CreateAsync`).
    pub async fn create(&self, mut user: User, password: Option<&str>) -> Result<User> {
        if user.id.is_empty() {
            user.id = new_id();
        }
        user.user_name = user.user_name.trim().to_string();
        user.email = user
            .email
            .map(|e| e.trim().to_string())
            .filter(|e| !e.is_empty());
        self.validate(&user).await?;
        if let Some(pw) = password {
            self.validate_password(pw).await?;
            user.password_hash = Some(self.hash_password(pw).await?);
        }
        user.security_stamp = new_stamp();
        user.concurrency_stamp = new_stamp();
        user.lockout_enabled = self
            .options()
            .lockout
            .allowed_for_new_users;
        user.normalized_user_name = normalize(&user.user_name);
        user.normalized_email = user.email.as_deref().map(normalize);
        self.store()
            .create_user(&user)
            .await
            .map_err(|e| match e {
                Error::Conflict => invalid("user_name", "duplicate_user_name"),
                e => e,
            })?;
        Ok(user)
    }

    /// Saves a changed user after validation, with a new concurrency stamp. [`Error::Conflict`]
    /// when someone else saved it since it was loaded (`UpdateAsync`).
    pub async fn update(&self, user: &mut User) -> Result<()> {
        self.validate(user).await?;
        self.save(user).await
    }

    /// Saves without validating names (for counters and stamps).
    pub(crate) async fn save(&self, user: &mut User) -> Result<()> {
        user.normalized_user_name = normalize(&user.user_name);
        user.normalized_email = user.email.as_deref().map(normalize);
        let expected = std::mem::replace(&mut user.concurrency_stamp, new_stamp());
        if let Err(e) = self
            .store()
            .update_user(user, &expected)
            .await
        {
            user.concurrency_stamp = expected;
            return Err(e);
        }
        Ok(())
    }

    /// Deletes a user with its roles, claims, logins, tokens and passkeys.
    pub async fn delete(&self, user: &User) -> Result<()> {
        self.store()
            .delete_user(&user.id)
            .await?;
        self.0
            .event(AccountEventKind::Deleted, Some(&user.id));
        Ok(())
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<User>> {
        if id.is_empty() || id.len() > MAX_NAME {
            return Ok(None);
        }
        self.store().user_by_id(id).await
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<User>> {
        if name.trim().is_empty() || name.len() > MAX_NAME {
            return Ok(None);
        }
        self.store()
            .user_by_name(&normalize(name))
            .await
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        if email.trim().is_empty() || email.len() > MAX_NAME {
            return Ok(None);
        }
        self.store()
            .user_by_email(&normalize(email))
            .await
    }

    /// By user name, else by e-mail: what a sign in form accepts.
    pub async fn find_by_name_or_email(&self, name: &str) -> Result<Option<User>> {
        match self.find_by_name(name).await? {
            Some(u) => Ok(Some(u)),
            None => self.find_by_email(name).await,
        }
    }

    pub async fn find_by_login(&self, provider: &str, key: &str) -> Result<Option<User>> {
        if key.len() > MAX_NAME * 4 {
            return Ok(None);
        }
        self.store()
            .user_by_login(provider, key)
            .await
    }

    /// A page of users for administration.
    pub async fn list(
        &self,
        search: Option<&str>,
        offset: u64,
        limit: u32,
    ) -> Result<(Vec<User>, u64)> {
        let search = search
            .map(normalize)
            .filter(|s| !s.is_empty());
        self.store()
            .users(search.as_deref(), offset, limit.min(crate::MAX_PAGE_SIZE))
            .await
    }

    /// Checks a password. A correct password in an older format (Argon2 with weaker parameters,
    /// or an ASP.NET Core Identity v2 or v3 hash) is rehashed with the current Argon2id
    /// parameters and saved (`CheckPasswordAsync`). Users without a password never match.
    pub async fn check_password(&self, user: &mut User, password: &str) -> Result<bool> {
        let Some(stored) = user.password_hash.clone() else {
            self.dummy_check(password).await;
            return Ok(false);
        };
        match self
            .verify_password(&stored, password)
            .await
        {
            Verification::Failed => Ok(false),
            Verification::Success => Ok(true),
            Verification::SuccessRehashNeeded => {
                user.password_hash = Some(self.hash_password(password).await?);
                if let Err(e) = self.save(user).await {
                    tracing::warn!("password rehash not saved: {e}");
                }
                Ok(true)
            }
        }
    }

    /// Spends the time of one password check without a user, so a missing account answers as
    /// slowly as a wrong password.
    pub(crate) async fn dummy_check(&self, password: &str) {
        let dummy = self.0.0.dummy_hash.clone();
        let _ = self
            .verify_password(&dummy, password)
            .await;
    }

    /// Hashes a password off the async threads, at most [`MAX_HASHING`] at once, so a burst of
    /// sign ins can neither stall the server nor take more than that many times the Argon2 memory.
    pub async fn hash_password(&self, password: &str) -> Result<String> {
        let (hasher, password) = (self.options().hasher, password.to_string());
        let _permit = HASHING
            .acquire()
            .await
            .map_err(|_| Error::Internal("hashing closed".into()))?;
        tokio::task::spawn_blocking(move || hasher.hash(&password))
            .await
            .map_err(|e| Error::Internal(format!("hashing: {e}")))?
    }

    /// Checks a password against a stored hash off the async threads (see [`Self::hash_password`]).
    pub async fn verify_password(&self, stored: &str, password: &str) -> Verification {
        let (hasher, stored, password) = (
            self.options().hasher,
            stored.to_string(),
            password.to_string(),
        );
        let Ok(_permit) = HASHING.acquire().await else {
            return Verification::Failed;
        };
        tokio::task::spawn_blocking(move || hasher.verify(&stored, &password))
            .await
            .unwrap_or(Verification::Failed)
    }

    pub fn has_password(&self, user: &User) -> bool {
        user.password_hash.is_some()
    }

    /// Sets a password for a user who has none (signed up with an external login or a passkey).
    pub async fn add_password(&self, user: &mut User, password: &str) -> Result<()> {
        if user.password_hash.is_some() {
            return Err(invalid("password", "user_already_has_password"));
        }
        self.set_password(user, password).await
    }

    /// Changes the password after checking the current one (`ChangePasswordAsync`). The security
    /// stamp changes, which signs out every other session and voids outstanding links.
    pub async fn change_password(&self, user: &mut User, current: &str, new: &str) -> Result<()> {
        if !self
            .check_password(user, current)
            .await?
        {
            return Err(invalid("old_password", "password_mismatch"));
        }
        self.set_password(user, new).await?;
        self.0
            .event(AccountEventKind::PasswordChanged, Some(&user.id));
        Ok(())
    }

    async fn set_password(&self, user: &mut User, password: &str) -> Result<()> {
        self.validate_password(password).await?;
        user.password_hash = Some(self.hash_password(password).await?);
        user.security_stamp = new_stamp();
        self.save(user).await
    }

    /// Removes the password; the user then signs in with an external login or a passkey only.
    pub async fn remove_password(&self, user: &mut User) -> Result<()> {
        user.password_hash = None;
        user.security_stamp = new_stamp();
        self.save(user).await
    }

    /// A new security stamp: every session, remembered device and outstanding link or code of
    /// the user stops working (sign out everywhere).
    pub async fn update_security_stamp(&self, user: &mut User) -> Result<()> {
        user.security_stamp = new_stamp();
        self.save(user).await?;
        self.0
            .event(AccountEventKind::StampChanged, Some(&user.id));
        Ok(())
    }

    /// A purpose token for the user (`GenerateUserTokenAsync` with the data protection provider).
    pub fn generate_token(&self, user: &User, purpose: &Purpose) -> String {
        self.0
            .0
            .keys
            .token(now_secs(), &user.id, purpose, &user.security_stamp)
    }

    /// Whether a purpose token is valid for the user now (`VerifyUserTokenAsync`).
    pub fn verify_token(&self, user: &User, purpose: &Purpose, token: &str) -> bool {
        let life = self
            .options()
            .tokens
            .token_lifespan
            .as_secs() as i64;
        self.0.0.keys.check_token(
            token,
            now_secs(),
            life,
            &user.id,
            purpose,
            &user.security_stamp,
        )
    }

    pub fn generate_email_confirmation_token(&self, user: &User) -> String {
        self.generate_token(user, &Purpose::ConfirmEmail)
    }

    /// Marks the e-mail address confirmed when the token is valid (`ConfirmEmailAsync`).
    pub async fn confirm_email(&self, user: &mut User, token: &str) -> Result<()> {
        if !self.verify_token(user, &Purpose::ConfirmEmail, token) {
            return Err(denied("invalid_token"));
        }
        user.email_confirmed = true;
        self.save(user).await?;
        self.0
            .event(AccountEventKind::EmailConfirmed, Some(&user.id));
        Ok(())
    }

    pub fn generate_password_reset_token(&self, user: &User) -> String {
        self.generate_token(user, &Purpose::ResetPassword)
    }

    /// Sets a new password when the reset token is valid (`ResetPasswordAsync`). The new stamp
    /// voids the token and signs out every session.
    pub async fn reset_password(&self, user: &mut User, token: &str, new: &str) -> Result<()> {
        if !self.verify_token(user, &Purpose::ResetPassword, token) {
            return Err(denied("invalid_token"));
        }
        self.set_password(user, new).await?;
        if user.lockout_end.is_some() {
            user.lockout_end = None;
            user.access_failed_count = 0;
            self.save(user).await?;
        }
        self.0
            .event(AccountEventKind::PasswordReset, Some(&user.id));
        Ok(())
    }

    pub fn generate_change_email_token(&self, user: &User, new_email: &str) -> String {
        self.generate_token(user, &Purpose::ChangeEmail(normalize(new_email)))
    }

    /// Changes the e-mail address when the token for that address is valid; the address counts
    /// as confirmed and the stamp changes (`ChangeEmailAsync`). With `user_name_is_email` the
    /// user name follows, as in the default UI where both are the same.
    pub async fn change_email(
        &self,
        user: &mut User,
        new_email: &str,
        token: &str,
        user_name_is_email: bool,
    ) -> Result<()> {
        if !self.verify_token(user, &Purpose::ChangeEmail(normalize(new_email)), token) {
            return Err(denied("invalid_token"));
        }
        user.email = Some(new_email.trim().to_string());
        user.email_confirmed = true;
        if user_name_is_email {
            user.user_name = new_email.trim().to_string();
        }
        user.security_stamp = new_stamp();
        self.update(user).await
    }

    /// A six digit code that confirms `phone` (`GenerateChangePhoneNumberTokenAsync`).
    pub fn generate_change_phone_code(&self, user: &User, phone: &str) -> String {
        self.0.0.keys.short_code(
            now_secs(),
            &user.security_stamp,
            &format!(
                "Totp:{}:{}",
                Purpose::ChangePhone(phone.to_string()).name(),
                user.id
            ),
        )
    }

    /// Sets and confirms the phone number when the code matches (`ChangePhoneNumberAsync`).
    pub async fn change_phone(&self, user: &mut User, phone: &str, code: &str) -> Result<()> {
        let modifier = format!(
            "Totp:{}:{}",
            Purpose::ChangePhone(phone.to_string()).name(),
            user.id
        );
        if !self
            .0
            .0
            .keys
            .check_short_code(code, now_secs(), &user.security_stamp, &modifier)
        {
            return Err(invalid("code", "invalid_code"));
        }
        user.phone_number = Some(phone.to_string());
        user.phone_number_confirmed = true;
        user.security_stamp = new_stamp();
        self.update(user).await
    }

    /// Removes the phone number.
    pub async fn remove_phone(&self, user: &mut User) -> Result<()> {
        user.phone_number = None;
        user.phone_number_confirmed = false;
        user.security_stamp = new_stamp();
        self.save(user).await
    }

    /// Adds the user to a role by name (`AddToRoleAsync`). [`Error::NotFound`] for an unknown
    /// role. The stamp changes so open sessions pick up the role at their next check.
    pub async fn add_to_role(&self, user: &mut User, role: &str) -> Result<()> {
        let role = self
            .store()
            .role_by_name(&normalize(role))
            .await?
            .ok_or(Error::NotFound)?;
        self.store()
            .add_to_role(&user.id, &role.id)
            .await?;
        self.update_security_stamp(user).await
    }

    pub async fn remove_from_role(&self, user: &mut User, role: &str) -> Result<()> {
        let role = self
            .store()
            .role_by_name(&normalize(role))
            .await?
            .ok_or(Error::NotFound)?;
        self.store()
            .remove_from_role(&user.id, &role.id)
            .await?;
        self.update_security_stamp(user).await
    }

    /// Names of the user's roles.
    pub async fn roles(&self, user: &User) -> Result<Vec<String>> {
        Ok(self
            .store()
            .roles_of(&user.id)
            .await?
            .into_iter()
            .map(|r| r.name)
            .collect())
    }

    pub async fn is_in_role(&self, user: &User, role: &str) -> Result<bool> {
        let n = normalize(role);
        Ok(self
            .store()
            .roles_of(&user.id)
            .await?
            .iter()
            .any(|r| r.normalized_name == n))
    }

    pub async fn users_in_role(&self, role: &str, limit: u32) -> Result<Vec<User>> {
        match self
            .store()
            .role_by_name(&normalize(role))
            .await?
        {
            Some(r) => {
                self.store()
                    .users_in_role(&r.id, limit.min(crate::MAX_PAGE_SIZE))
                    .await
            }
            None => Ok(Vec::new()),
        }
    }

    pub async fn claims(&self, user: &User) -> Result<Vec<Claim>> {
        self.store().claims_of(&user.id).await
    }

    pub async fn add_claim(&self, user: &mut User, claim: Claim) -> Result<()> {
        self.store()
            .add_claim(&user.id, &claim)
            .await?;
        self.update_security_stamp(user).await
    }

    pub async fn remove_claim(&self, user: &mut User, claim: &Claim) -> Result<()> {
        self.store()
            .remove_claim(&user.id, claim)
            .await?;
        self.update_security_stamp(user).await
    }

    /// Whether the user is locked out now (`IsLockedOutAsync`).
    pub fn is_locked_out(&self, user: &User) -> bool {
        user.is_locked_out(now_secs())
    }

    pub async fn set_lockout_enabled(&self, user: &mut User, enabled: bool) -> Result<()> {
        user.lockout_enabled = enabled;
        self.save(user).await
    }

    /// Locks the user out until `end` (Unix seconds), or unlocks with `None`
    /// (`SetLockoutEndDateAsync`). Fails when lockout is off for the user.
    pub async fn set_lockout_end(&self, user: &mut User, end: Option<i64>) -> Result<()> {
        if !user.lockout_enabled {
            return Err(denied("lockout_not_enabled"));
        }
        user.lockout_end = end;
        if end.is_none() {
            user.access_failed_count = 0;
        }
        self.save(user).await
    }

    /// Counts a failed attempt; at `max_failed_access_attempts` the user is locked out for
    /// `default_lockout` and the count starts again (`AccessFailedAsync`). Returns whether the
    /// user is now locked out.
    pub async fn access_failed(&self, user: &mut User) -> Result<bool> {
        if !user.lockout_enabled {
            return Ok(false);
        }
        user.access_failed_count += 1;
        let lockout = self.options().lockout;
        let locked = user.access_failed_count >= lockout.max_failed_access_attempts;
        if locked {
            user.lockout_end = Some(now_secs() + lockout.default_lockout.as_secs() as i64);
            user.access_failed_count = 0;
        }
        self.save(user).await?;
        if locked {
            self.0
                .event(AccountEventKind::LockedOut, Some(&user.id));
        }
        Ok(locked)
    }

    /// Starts the failed attempt count again (`ResetAccessFailedCountAsync`).
    pub async fn reset_access_failed(&self, user: &mut User) -> Result<()> {
        if user.access_failed_count == 0 {
            return Ok(());
        }
        user.access_failed_count = 0;
        self.save(user).await
    }

    /// Turns two factor sign in on or off (`SetTwoFactorEnabledAsync`). The stamp changes.
    pub async fn set_two_factor_enabled(&self, user: &mut User, enabled: bool) -> Result<()> {
        user.two_factor_enabled = enabled;
        user.security_stamp = new_stamp();
        self.save(user).await?;
        self.0.event(
            if enabled {
                AccountEventKind::TwoFactorEnabled
            } else {
                AccountEventKind::TwoFactorDisabled
            },
            Some(&user.id),
        );
        Ok(())
    }

    fn key_name(user: &User) -> String {
        format!("zeyn_accounts_authenticator:{}", user.id)
    }

    /// The user's authenticator key in base32, if one was set (`GetAuthenticatorKeyAsync`). The
    /// store holds it sealed with the accounts key and bound to the user.
    pub async fn authenticator_key(&self, user: &User) -> Result<Option<String>> {
        let Some(sealed) = self
            .store()
            .token(&user.id, INTERNAL_PROVIDER, AUTHENTICATOR_KEY)
            .await?
        else {
            return Ok(None);
        };
        let sealer = self
            .0
            .0
            .sealer
            .as_ref()
            .ok_or_else(|| Error::Internal("accounts key too short".into()))?;
        Ok(sealer.open::<String>(&Self::key_name(user), &sealed))
    }

    /// A new authenticator key (`ResetAuthenticatorKeyAsync`): the old one stops working and the
    /// stamp changes.
    pub async fn reset_authenticator_key(&self, user: &mut User) -> Result<String> {
        let key = tokens::authenticator_key();
        let sealer = self
            .0
            .0
            .sealer
            .as_ref()
            .ok_or_else(|| Error::Internal("accounts key too short".into()))?;
        self.store()
            .set_token(
                &user.id,
                INTERNAL_PROVIDER,
                AUTHENTICATOR_KEY,
                &sealer.seal(&Self::key_name(user), &key),
            )
            .await?;
        self.store()
            .remove_token(&user.id, INTERNAL_PROVIDER, AUTHENTICATOR_STEP)
            .await?;
        self.update_security_stamp(user).await?;
        Ok(key)
    }

    /// The `otpauth://` URI for the QR code of an authenticator key, in the format ASP.NET Core
    /// Identity's default UI uses.
    pub fn authenticator_uri(&self, user: &User, key: &str) -> String {
        let issuer = super::pct(
            &self
                .options()
                .tokens
                .authenticator_issuer,
        );
        let account = super::pct(
            user.email
                .as_deref()
                .unwrap_or(&user.user_name),
        );
        format!("otpauth://totp/{issuer}:{account}?secret={key}&issuer={issuer}&digits=6")
    }

    /// Checks an authenticator app code: 30 second steps, two steps either way, and a step is
    /// accepted only once (`VerifyTwoFactorTokenAsync` with the authenticator provider).
    pub async fn verify_authenticator_code(&self, user: &User, code: &str) -> Result<bool> {
        let Some(_turn) = self.0.0.in_flight.enter(&user.id) else {
            return Ok(false);
        };
        let Some(key) = self.authenticator_key(user).await? else {
            return Ok(false);
        };
        let Some(bytes) = tokens::base32_decode(&key) else {
            return Ok(false);
        };
        let last = self
            .store()
            .token(&user.id, INTERNAL_PROVIDER, AUTHENTICATOR_STEP)
            .await?
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        match tokens::check_authenticator(&bytes, code, now_secs(), last) {
            Some(step) => {
                self.store()
                    .set_token(
                        &user.id,
                        INTERNAL_PROVIDER,
                        AUTHENTICATOR_STEP,
                        &step.to_string(),
                    )
                    .await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// `count` new recovery codes; the old ones stop working. Only their keyed hashes are stored
    /// (`GenerateNewTwoFactorRecoveryCodesAsync`).
    pub async fn generate_recovery_codes(&self, user: &User, count: usize) -> Result<Vec<String>> {
        let mut codes: Vec<String> = Vec::with_capacity(count);
        while codes.len() < count.min(50) {
            let c = tokens::recovery_code();
            if !codes.contains(&c) {
                codes.push(c);
            }
        }
        let hashes: Vec<String> = codes
            .iter()
            .map(|c| self.0.0.keys.recovery_hash(c))
            .collect();
        self.store()
            .set_token(
                &user.id,
                INTERNAL_PROVIDER,
                RECOVERY_CODES,
                &hashes.join(";"),
            )
            .await?;
        self.0.event(
            super::AccountEventKind::RecoveryCodesRenewed,
            Some(&user.id),
        );
        Ok(codes)
    }

    /// Uses up a recovery code (`RedeemTwoFactorRecoveryCodeAsync`). Case and spaces do not
    /// matter; the comparison takes the same time wherever a code differs.
    pub async fn redeem_recovery_code(&self, user: &User, code: &str) -> Result<bool> {
        if code.len() > 64 {
            return Ok(false);
        }
        let Some(_turn) = self.0.0.in_flight.enter(&user.id) else {
            return Ok(false);
        };
        let stored = self
            .store()
            .token(&user.id, INTERNAL_PROVIDER, RECOVERY_CODES)
            .await?
            .unwrap_or_default();
        let hash = self.0.0.keys.recovery_hash(code);
        let mut found = false;
        let rest: Vec<&str> = stored
            .split(';')
            .filter(|h| !h.is_empty())
            .filter(|h| {
                let same = crate::server::same(h, &hash);
                found |= same;
                !same
            })
            .collect();
        if found {
            self.store()
                .set_token(&user.id, INTERNAL_PROVIDER, RECOVERY_CODES, &rest.join(";"))
                .await?;
            self.0
                .event(super::AccountEventKind::RecoveryCodeUsed, Some(&user.id));
        }
        Ok(found)
    }

    pub async fn count_recovery_codes(&self, user: &User) -> Result<usize> {
        Ok(self
            .store()
            .token(&user.id, INTERNAL_PROVIDER, RECOVERY_CODES)
            .await?
            .unwrap_or_default()
            .split(';')
            .filter(|h| !h.is_empty())
            .count())
    }

    /// A six digit sign in code for e-mail (`GenerateTwoFactorTokenAsync` with the e-mail
    /// provider). Valid for about nine minutes and until the stamp changes.
    pub fn generate_email_code(&self, user: &User) -> String {
        self.0.0.keys.short_code(
            now_secs(),
            &user.security_stamp,
            &format!("Email:TwoFactor:{}", user.email.as_deref().unwrap_or("")),
        )
    }

    pub fn verify_email_code(&self, user: &User, code: &str) -> bool {
        self.0.0.keys.check_short_code(
            code,
            now_secs(),
            &user.security_stamp,
            &format!("Email:TwoFactor:{}", user.email.as_deref().unwrap_or("")),
        )
    }

    /// A six digit sign in code for text messages (the phone number provider).
    pub fn generate_phone_code(&self, user: &User) -> String {
        self.0.0.keys.short_code(
            now_secs(),
            &user.security_stamp,
            &format!(
                "Phone:TwoFactor:{}",
                user.phone_number
                    .as_deref()
                    .unwrap_or("")
            ),
        )
    }

    pub fn verify_phone_code(&self, user: &User, code: &str) -> bool {
        self.0.0.keys.check_short_code(
            code,
            now_secs(),
            &user.security_stamp,
            &format!(
                "Phone:TwoFactor:{}",
                user.phone_number
                    .as_deref()
                    .unwrap_or("")
            ),
        )
    }

    /// The second factors the user can use now (`GetValidTwoFactorProvidersAsync`).
    pub async fn two_factor_kinds(&self, user: &User) -> Result<Vec<super::TwoFactorKind>> {
        let mut kinds = Vec::new();
        if self
            .authenticator_key(user)
            .await?
            .is_some()
        {
            kinds.push(super::TwoFactorKind::Authenticator);
        }
        if user.email_confirmed && user.email.is_some() {
            kinds.push(super::TwoFactorKind::Email);
        }
        if user.phone_number_confirmed && user.phone_number.is_some() {
            kinds.push(super::TwoFactorKind::Phone);
        }
        Ok(kinds)
    }

    /// Links an external login (`AddLoginAsync`). Refused when another user has it.
    pub async fn add_login(&self, user: &mut User, login: UserLogin) -> Result<()> {
        self.store()
            .add_login(&user.id, &login)
            .await
            .map_err(|e| match e {
                Error::Conflict => denied("login_already_associated"),
                e => e,
            })?;
        self.update_security_stamp(user).await?;
        self.0
            .event(AccountEventKind::LoginLinked, Some(&user.id));
        Ok(())
    }

    /// Unlinks an external login (`RemoveLoginAsync`). Refused when it is the user's last way
    /// to sign in (no password, no other login, no passkey).
    pub async fn remove_login(&self, user: &mut User, provider: &str, key: &str) -> Result<()> {
        let logins = self.store().logins_of(&user.id).await?;
        let passkeys = self
            .store()
            .passkeys_of(&user.id)
            .await?;
        if user.password_hash.is_none() && logins.len() <= 1 && passkeys.is_empty() {
            return Err(denied("last_login"));
        }
        self.store()
            .remove_login(&user.id, provider, key)
            .await?;
        self.update_security_stamp(user).await?;
        self.0
            .event(AccountEventKind::LoginUnlinked, Some(&user.id));
        Ok(())
    }

    pub async fn logins(&self, user: &User) -> Result<Vec<UserLogin>> {
        self.store().logins_of(&user.id).await
    }

    /// The user's passkeys.
    pub async fn passkeys(&self, user: &User) -> Result<Vec<super::Passkey>> {
        self.store().passkeys_of(&user.id).await
    }

    /// Removes a passkey (`RemovePasskeyAsync`). Refused when it is the user's last way to sign in.
    pub async fn remove_passkey(&self, user: &mut User, credential_id: &[u8]) -> Result<()> {
        let passkeys = self
            .store()
            .passkeys_of(&user.id)
            .await?;
        let logins = self.store().logins_of(&user.id).await?;
        if user.password_hash.is_none() && logins.is_empty() && passkeys.len() <= 1 {
            return Err(denied("last_login"));
        }
        self.store()
            .remove_passkey(&user.id, credential_id)
            .await?;
        self.update_security_stamp(user).await?;
        self.0
            .event(AccountEventKind::PasskeyRemoved, Some(&user.id));
        Ok(())
    }

    /// The user's personal data as JSON: profile fields, the app's own fields, roles, claims,
    /// external logins and passkey names, never hashes, stamps, keys or codes
    /// (`DownloadPersonalData` with `PersonalDataAttribute`).
    pub async fn personal_data(&self, user: &User) -> Result<serde_json::Value> {
        let logins = self.logins(user).await?;
        let passkeys = self.passkeys(user).await?;
        Ok(serde_json::json!({
            "id": user.id,
            "user_name": user.user_name,
            "email": user.email,
            "email_confirmed": user.email_confirmed,
            "phone_number": user.phone_number,
            "phone_number_confirmed": user.phone_number_confirmed,
            "two_factor_enabled": user.two_factor_enabled,
            "data": user.extra,
            "roles": self.roles(user).await?,
            "claims": self.claims(user).await?.into_iter().map(|c| serde_json::json!({ "type": c.kind, "value": c.value })).collect::<Vec<_>>(),
            "logins": logins.into_iter().map(|l| serde_json::json!({ "provider": l.provider, "key": l.provider_key })).collect::<Vec<_>>(),
            "passkeys": passkeys.into_iter().map(|p| serde_json::json!({ "name": p.name, "created_at": p.created_at })).collect::<Vec<_>>(),
        }))
    }
}

impl RoleManager {
    fn store(&self) -> &dyn super::AccountStore {
        &*self.0.0.store
    }

    /// Creates a role (`CreateAsync` with `RoleValidator`): a name that is not blank and not taken.
    pub async fn create(&self, name: &str) -> Result<Role> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_NAME {
            return Err(invalid("name", "invalid_role_name"));
        }
        let role = Role {
            id: new_id(),
            name: name.to_string(),
            normalized_name: normalize(name),
            concurrency_stamp: new_stamp(),
        };
        self.store()
            .create_role(&role)
            .await
            .map_err(|e| match e {
                Error::Conflict => invalid("name", "duplicate_role_name"),
                e => e,
            })?;
        Ok(role)
    }

    /// Renames a role. [`Error::Conflict`] when it changed since it was loaded.
    pub async fn rename(&self, role: &mut Role, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_NAME {
            return Err(invalid("name", "invalid_role_name"));
        }
        if let Some(other) = self.find_by_name(name).await?
            && other.id != role.id
        {
            return Err(invalid("name", "duplicate_role_name"));
        }
        let expected = std::mem::replace(&mut role.concurrency_stamp, new_stamp());
        role.name = name.to_string();
        role.normalized_name = normalize(name);
        self.store()
            .update_role(role, &expected)
            .await
    }

    pub async fn delete(&self, role: &Role) -> Result<()> {
        self.store().delete_role(&role.id).await
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Option<Role>> {
        self.store().role_by_id(id).await
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<Role>> {
        self.store()
            .role_by_name(&normalize(name))
            .await
    }

    pub async fn exists(&self, name: &str) -> Result<bool> {
        Ok(self.find_by_name(name).await?.is_some())
    }

    pub async fn list(&self, offset: u64, limit: u32) -> Result<(Vec<Role>, u64)> {
        self.store()
            .roles(offset, limit.min(crate::MAX_PAGE_SIZE))
            .await
    }

    pub async fn claims(&self, role: &Role) -> Result<Vec<Claim>> {
        self.store().role_claims(&role.id).await
    }

    /// Adds a claim to a role. Claims of type `permission` grant that permission to the role's
    /// users from their next session check on.
    pub async fn add_claim(&self, role: &Role, claim: Claim) -> Result<()> {
        self.store()
            .add_role_claim(&role.id, &claim)
            .await
    }

    pub async fn remove_claim(&self, role: &Role, claim: &Claim) -> Result<()> {
        self.store()
            .remove_role_claim(&role.id, claim)
            .await
    }
}
