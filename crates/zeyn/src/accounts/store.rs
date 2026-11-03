//! Where accounts live: the [`AccountStore`] trait and [`MemoryAccountStore`].
//!
//! ASP.NET Core Identity splits the store into many optional interfaces (`IUserStore`,
//! `IUserPasswordStore`, `IUserRoleStore`, `IUserClaimStore`, `IUserLoginStore`,
//! `IUserTwoFactorStore`, `IUserLockoutStore`, `IUserAuthenticationTokenStore`,
//! `IUserPasskeyStore`, `IRoleStore`, `IRoleClaimStore`) and the manager checks at run time which
//! ones a store implements. Here one async trait holds them all: password, lockout, two factor,
//! e-mail and phone are fields of [`User`] saved with `update_user`, and the rest are a few
//! methods each. A store that lacks a part is a compile error, not a `NotSupportedException`.

use super::model::{Claim, Passkey, Role, User, UserLogin};
use crate::ctx::BoxFuture;
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Storage of users, roles, their claims, external logins, tokens and passkeys.
///
/// Names are looked up by their normalized form (see [`super::normalize`]). `update_user` and
/// `update_role` save only when the stored concurrency stamp is still `expected`, else they
/// answer [`Error::Conflict`]. `create_user` answers [`Error::Conflict`] for a normalized user
/// name that is taken. Deleting a user deletes its roles, claims, logins, tokens and passkeys.
pub trait AccountStore: Send + Sync + 'static {
    fn create_user(&self, user: &User) -> BoxFuture<Result<()>>;
    fn update_user(&self, user: &User, expected: &str) -> BoxFuture<Result<()>>;
    fn delete_user(&self, id: &str) -> BoxFuture<Result<()>>;
    fn user_by_id(&self, id: &str) -> BoxFuture<Result<Option<User>>>;
    fn user_by_name(&self, normalized: &str) -> BoxFuture<Result<Option<User>>>;
    fn user_by_email(&self, normalized: &str) -> BoxFuture<Result<Option<User>>>;
    /// A page of users whose name or e-mail contains `search` (normalized), ordered by user name,
    /// and the total.
    fn users(
        &self,
        search: Option<&str>,
        offset: u64,
        limit: u32,
    ) -> BoxFuture<Result<(Vec<User>, u64)>>;

    fn create_role(&self, role: &Role) -> BoxFuture<Result<()>>;
    fn update_role(&self, role: &Role, expected: &str) -> BoxFuture<Result<()>>;
    fn delete_role(&self, id: &str) -> BoxFuture<Result<()>>;
    fn role_by_id(&self, id: &str) -> BoxFuture<Result<Option<Role>>>;
    fn role_by_name(&self, normalized: &str) -> BoxFuture<Result<Option<Role>>>;
    fn roles(&self, offset: u64, limit: u32) -> BoxFuture<Result<(Vec<Role>, u64)>>;

    fn add_to_role(&self, user_id: &str, role_id: &str) -> BoxFuture<Result<()>>;
    fn remove_from_role(&self, user_id: &str, role_id: &str) -> BoxFuture<Result<()>>;
    fn roles_of(&self, user_id: &str) -> BoxFuture<Result<Vec<Role>>>;
    fn users_in_role(&self, role_id: &str, limit: u32) -> BoxFuture<Result<Vec<User>>>;

    fn claims_of(&self, user_id: &str) -> BoxFuture<Result<Vec<Claim>>>;
    fn add_claim(&self, user_id: &str, claim: &Claim) -> BoxFuture<Result<()>>;
    fn remove_claim(&self, user_id: &str, claim: &Claim) -> BoxFuture<Result<()>>;
    fn role_claims(&self, role_id: &str) -> BoxFuture<Result<Vec<Claim>>>;
    fn add_role_claim(&self, role_id: &str, claim: &Claim) -> BoxFuture<Result<()>>;
    fn remove_role_claim(&self, role_id: &str, claim: &Claim) -> BoxFuture<Result<()>>;

    /// Links an external login. [`Error::Conflict`] when the provider key is linked already.
    fn add_login(&self, user_id: &str, login: &UserLogin) -> BoxFuture<Result<()>>;
    fn remove_login(&self, user_id: &str, provider: &str, key: &str) -> BoxFuture<Result<()>>;
    fn logins_of(&self, user_id: &str) -> BoxFuture<Result<Vec<UserLogin>>>;
    fn user_by_login(&self, provider: &str, key: &str) -> BoxFuture<Result<Option<User>>>;

    fn token(&self, user_id: &str, provider: &str, name: &str)
    -> BoxFuture<Result<Option<String>>>;
    fn set_token(
        &self,
        user_id: &str,
        provider: &str,
        name: &str,
        value: &str,
    ) -> BoxFuture<Result<()>>;
    fn remove_token(&self, user_id: &str, provider: &str, name: &str) -> BoxFuture<Result<()>>;

    fn passkeys_of(&self, user_id: &str) -> BoxFuture<Result<Vec<Passkey>>>;
    /// The passkey with this credential id and the id of its user.
    fn passkey(&self, credential_id: &[u8]) -> BoxFuture<Result<Option<(String, Passkey)>>>;
    /// Adds a passkey or updates the one with the same credential id of the same user.
    /// [`Error::Conflict`] when the credential belongs to another user.
    fn save_passkey(&self, user_id: &str, passkey: &Passkey) -> BoxFuture<Result<()>>;
    fn remove_passkey(&self, user_id: &str, credential_id: &[u8]) -> BoxFuture<Result<()>>;
}

#[derive(Default)]
struct State {
    users: HashMap<String, User>,
    roles: HashMap<String, Role>,
    user_roles: Vec<(String, String)>,
    claims: Vec<(String, Claim)>,
    role_claims: Vec<(String, Claim)>,
    logins: Vec<(String, UserLogin)>,
    tokens: HashMap<(String, String, String), String>,
    passkeys: Vec<(String, Passkey)>,
}

/// Accounts in memory, for tests and demos. At most `max_users` users (10 000 unless set) and
/// 1 000 roles; more is refused, so a sign up flood cannot exhaust memory. Everything is gone
/// when the process ends.
#[derive(Clone)]
pub struct MemoryAccountStore {
    state: Arc<Mutex<State>>,
    max_users: usize,
}

impl Default for MemoryAccountStore {
    fn default() -> Self {
        MemoryAccountStore {
            state: Arc::default(),
            max_users: 10_000,
        }
    }
}

/// Most roles a [`MemoryAccountStore`] keeps.
const MAX_ROLES: usize = 1_000;
/// Most claims, logins, tokens or passkeys a [`MemoryAccountStore`] keeps per user.
const MAX_PER_USER: usize = 64;

fn ready<T: Send + 'static>(value: Result<T>) -> BoxFuture<Result<T>> {
    Box::pin(async move { value })
}

impl MemoryAccountStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Caps the number of users.
    pub fn max_users(mut self, max: usize) -> Self {
        self.max_users = max.max(1);
        self
    }

    fn with<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut State) -> Result<T>,
    ) -> BoxFuture<Result<T>> {
        let result = match self.state.lock() {
            Ok(mut s) => f(&mut s),
            Err(_) => Err(Error::Internal("account store lock poisoned".into())),
        };
        ready(result)
    }

    /// Number of users, for tests.
    pub fn len(&self) -> usize {
        self.state
            .lock()
            .map(|s| s.users.len())
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn per_user<T>(list: &[(String, T)], user: &str) -> Result<()> {
    if list
        .iter()
        .filter(|(u, _)| u == user)
        .count()
        >= MAX_PER_USER
    {
        return Err(Error::Internal(
            "too many entries for one user in the memory account store".into(),
        ));
    }
    Ok(())
}

impl AccountStore for MemoryAccountStore {
    fn create_user(&self, user: &User) -> BoxFuture<Result<()>> {
        let (user, max) = (user.clone(), self.max_users);
        self.with(move |s| {
            if s.users.len() >= max {
                return Err(Error::Internal("the memory account store is full".into()));
            }
            if s.users.contains_key(&user.id)
                || s.users
                    .values()
                    .any(|u| u.normalized_user_name == user.normalized_user_name)
            {
                return Err(Error::Conflict);
            }
            s.users.insert(user.id.clone(), user);
            Ok(())
        })
    }

    fn update_user(&self, user: &User, expected: &str) -> BoxFuture<Result<()>> {
        let (user, expected) = (user.clone(), expected.to_string());
        self.with(move |s| {
            if s.users
                .values()
                .any(|u| u.id != user.id && u.normalized_user_name == user.normalized_user_name)
            {
                return Err(Error::Conflict);
            }
            match s.users.get_mut(&user.id) {
                Some(stored) if stored.concurrency_stamp == expected => {
                    *stored = user;
                    Ok(())
                }
                _ => Err(Error::Conflict),
            }
        })
    }

    fn delete_user(&self, id: &str) -> BoxFuture<Result<()>> {
        let id = id.to_string();
        self.with(move |s| {
            s.users
                .remove(&id)
                .ok_or(Error::NotFound)?;
            s.user_roles.retain(|(u, _)| *u != id);
            s.claims.retain(|(u, _)| *u != id);
            s.logins.retain(|(u, _)| *u != id);
            s.tokens.retain(|(u, _, _), _| *u != id);
            s.passkeys.retain(|(u, _)| *u != id);
            Ok(())
        })
    }

    fn user_by_id(&self, id: &str) -> BoxFuture<Result<Option<User>>> {
        let id = id.to_string();
        self.with(move |s| Ok(s.users.get(&id).cloned()))
    }

    fn user_by_name(&self, normalized: &str) -> BoxFuture<Result<Option<User>>> {
        let n = normalized.to_string();
        self.with(move |s| {
            Ok(s.users
                .values()
                .find(|u| u.normalized_user_name == n)
                .cloned())
        })
    }

    fn user_by_email(&self, normalized: &str) -> BoxFuture<Result<Option<User>>> {
        let n = normalized.to_string();
        self.with(move |s| {
            Ok(s.users
                .values()
                .filter(|u| u.normalized_email.as_deref() == Some(n.as_str()))
                .min_by(|a, b| a.id.cmp(&b.id))
                .cloned())
        })
    }

    fn users(
        &self,
        search: Option<&str>,
        offset: u64,
        limit: u32,
    ) -> BoxFuture<Result<(Vec<User>, u64)>> {
        let search = search.map(str::to_string);
        self.with(move |s| {
            let mut all: Vec<&User> = s
                .users
                .values()
                .filter(|u| {
                    search.as_deref().is_none_or(|q| {
                        u.normalized_user_name.contains(q)
                            || u.normalized_email
                                .as_deref()
                                .is_some_and(|e| e.contains(q))
                    })
                })
                .collect();
            all.sort_by(|a, b| {
                a.normalized_user_name
                    .cmp(&b.normalized_user_name)
            });
            let total = all.len() as u64;
            Ok((
                all.into_iter()
                    .skip(offset as usize)
                    .take(limit as usize)
                    .cloned()
                    .collect(),
                total,
            ))
        })
    }

    fn create_role(&self, role: &Role) -> BoxFuture<Result<()>> {
        let role = role.clone();
        self.with(move |s| {
            if s.roles.len() >= MAX_ROLES {
                return Err(Error::Internal(
                    "the memory account store has too many roles".into(),
                ));
            }
            if s.roles.contains_key(&role.id)
                || s.roles
                    .values()
                    .any(|r| r.normalized_name == role.normalized_name)
            {
                return Err(Error::Conflict);
            }
            s.roles.insert(role.id.clone(), role);
            Ok(())
        })
    }

    fn update_role(&self, role: &Role, expected: &str) -> BoxFuture<Result<()>> {
        let (role, expected) = (role.clone(), expected.to_string());
        self.with(move |s| {
            if s.roles
                .values()
                .any(|r| r.id != role.id && r.normalized_name == role.normalized_name)
            {
                return Err(Error::Conflict);
            }
            match s.roles.get_mut(&role.id) {
                Some(stored) if stored.concurrency_stamp == expected => {
                    *stored = role;
                    Ok(())
                }
                _ => Err(Error::Conflict),
            }
        })
    }

    fn delete_role(&self, id: &str) -> BoxFuture<Result<()>> {
        let id = id.to_string();
        self.with(move |s| {
            s.roles
                .remove(&id)
                .ok_or(Error::NotFound)?;
            s.user_roles.retain(|(_, r)| *r != id);
            s.role_claims.retain(|(r, _)| *r != id);
            Ok(())
        })
    }

    fn role_by_id(&self, id: &str) -> BoxFuture<Result<Option<Role>>> {
        let id = id.to_string();
        self.with(move |s| Ok(s.roles.get(&id).cloned()))
    }

    fn role_by_name(&self, normalized: &str) -> BoxFuture<Result<Option<Role>>> {
        let n = normalized.to_string();
        self.with(move |s| {
            Ok(s.roles
                .values()
                .find(|r| r.normalized_name == n)
                .cloned())
        })
    }

    fn roles(&self, offset: u64, limit: u32) -> BoxFuture<Result<(Vec<Role>, u64)>> {
        self.with(move |s| {
            let mut all: Vec<&Role> = s.roles.values().collect();
            all.sort_by(|a, b| {
                a.normalized_name
                    .cmp(&b.normalized_name)
            });
            let total = all.len() as u64;
            Ok((
                all.into_iter()
                    .skip(offset as usize)
                    .take(limit as usize)
                    .cloned()
                    .collect(),
                total,
            ))
        })
    }

    fn add_to_role(&self, user_id: &str, role_id: &str) -> BoxFuture<Result<()>> {
        let pair = (user_id.to_string(), role_id.to_string());
        self.with(move |s| {
            if !s.users.contains_key(&pair.0) || !s.roles.contains_key(&pair.1) {
                return Err(Error::NotFound);
            }
            if !s.user_roles.contains(&pair) {
                per_user(&s.user_roles, &pair.0)?;
                s.user_roles.push(pair);
            }
            Ok(())
        })
    }

    fn remove_from_role(&self, user_id: &str, role_id: &str) -> BoxFuture<Result<()>> {
        let pair = (user_id.to_string(), role_id.to_string());
        self.with(move |s| {
            s.user_roles.retain(|p| *p != pair);
            Ok(())
        })
    }

    fn roles_of(&self, user_id: &str) -> BoxFuture<Result<Vec<Role>>> {
        let user = user_id.to_string();
        self.with(move |s| {
            let mut roles: Vec<Role> = s
                .user_roles
                .iter()
                .filter(|(u, _)| *u == user)
                .filter_map(|(_, r)| s.roles.get(r).cloned())
                .collect();
            roles.sort_by(|a, b| {
                a.normalized_name
                    .cmp(&b.normalized_name)
            });
            Ok(roles)
        })
    }

    fn users_in_role(&self, role_id: &str, limit: u32) -> BoxFuture<Result<Vec<User>>> {
        let role = role_id.to_string();
        self.with(move |s| {
            Ok(s.user_roles
                .iter()
                .filter(|(_, r)| *r == role)
                .filter_map(|(u, _)| s.users.get(u).cloned())
                .take(limit as usize)
                .collect())
        })
    }

    fn claims_of(&self, user_id: &str) -> BoxFuture<Result<Vec<Claim>>> {
        let user = user_id.to_string();
        self.with(move |s| {
            Ok(s.claims
                .iter()
                .filter(|(u, _)| *u == user)
                .map(|(_, c)| c.clone())
                .collect())
        })
    }

    fn add_claim(&self, user_id: &str, claim: &Claim) -> BoxFuture<Result<()>> {
        let (user, claim) = (user_id.to_string(), claim.clone());
        self.with(move |s| {
            per_user(&s.claims, &user)?;
            s.claims.push((user, claim));
            Ok(())
        })
    }

    fn remove_claim(&self, user_id: &str, claim: &Claim) -> BoxFuture<Result<()>> {
        let (user, claim) = (user_id.to_string(), claim.clone());
        self.with(move |s| {
            s.claims
                .retain(|(u, c)| !(*u == user && *c == claim));
            Ok(())
        })
    }

    fn role_claims(&self, role_id: &str) -> BoxFuture<Result<Vec<Claim>>> {
        let role = role_id.to_string();
        self.with(move |s| {
            Ok(s.role_claims
                .iter()
                .filter(|(r, _)| *r == role)
                .map(|(_, c)| c.clone())
                .collect())
        })
    }

    fn add_role_claim(&self, role_id: &str, claim: &Claim) -> BoxFuture<Result<()>> {
        let (role, claim) = (role_id.to_string(), claim.clone());
        self.with(move |s| {
            per_user(&s.role_claims, &role)?;
            s.role_claims.push((role, claim));
            Ok(())
        })
    }

    fn remove_role_claim(&self, role_id: &str, claim: &Claim) -> BoxFuture<Result<()>> {
        let (role, claim) = (role_id.to_string(), claim.clone());
        self.with(move |s| {
            s.role_claims
                .retain(|(r, c)| !(*r == role && *c == claim));
            Ok(())
        })
    }

    fn add_login(&self, user_id: &str, login: &UserLogin) -> BoxFuture<Result<()>> {
        let (user, login) = (user_id.to_string(), login.clone());
        self.with(move |s| {
            if s.logins
                .iter()
                .any(|(_, l)| l.provider == login.provider && l.provider_key == login.provider_key)
            {
                return Err(Error::Conflict);
            }
            per_user(&s.logins, &user)?;
            s.logins.push((user, login));
            Ok(())
        })
    }

    fn remove_login(&self, user_id: &str, provider: &str, key: &str) -> BoxFuture<Result<()>> {
        let (user, provider, key) = (user_id.to_string(), provider.to_string(), key.to_string());
        self.with(move |s| {
            s.logins
                .retain(|(u, l)| !(*u == user && l.provider == provider && l.provider_key == key));
            Ok(())
        })
    }

    fn logins_of(&self, user_id: &str) -> BoxFuture<Result<Vec<UserLogin>>> {
        let user = user_id.to_string();
        self.with(move |s| {
            Ok(s.logins
                .iter()
                .filter(|(u, _)| *u == user)
                .map(|(_, l)| l.clone())
                .collect())
        })
    }

    fn user_by_login(&self, provider: &str, key: &str) -> BoxFuture<Result<Option<User>>> {
        let (provider, key) = (provider.to_string(), key.to_string());
        self.with(move |s| {
            Ok(s.logins
                .iter()
                .find(|(_, l)| l.provider == provider && l.provider_key == key)
                .and_then(|(u, _)| s.users.get(u).cloned()))
        })
    }

    fn token(
        &self,
        user_id: &str,
        provider: &str,
        name: &str,
    ) -> BoxFuture<Result<Option<String>>> {
        let k = (user_id.to_string(), provider.to_string(), name.to_string());
        self.with(move |s| Ok(s.tokens.get(&k).cloned()))
    }

    fn set_token(
        &self,
        user_id: &str,
        provider: &str,
        name: &str,
        value: &str,
    ) -> BoxFuture<Result<()>> {
        let k = (user_id.to_string(), provider.to_string(), name.to_string());
        let value = value.to_string();
        self.with(move |s| {
            if !s.tokens.contains_key(&k)
                && s.tokens
                    .keys()
                    .filter(|(u, _, _)| *u == k.0)
                    .count()
                    >= MAX_PER_USER
            {
                return Err(Error::Internal(
                    "too many tokens for one user in the memory account store".into(),
                ));
            }
            s.tokens.insert(k, value);
            Ok(())
        })
    }

    fn remove_token(&self, user_id: &str, provider: &str, name: &str) -> BoxFuture<Result<()>> {
        let k = (user_id.to_string(), provider.to_string(), name.to_string());
        self.with(move |s| {
            s.tokens.remove(&k);
            Ok(())
        })
    }

    fn passkeys_of(&self, user_id: &str) -> BoxFuture<Result<Vec<Passkey>>> {
        let user = user_id.to_string();
        self.with(move |s| {
            Ok(s.passkeys
                .iter()
                .filter(|(u, _)| *u == user)
                .map(|(_, p)| p.clone())
                .collect())
        })
    }

    fn passkey(&self, credential_id: &[u8]) -> BoxFuture<Result<Option<(String, Passkey)>>> {
        let id = credential_id.to_vec();
        self.with(move |s| {
            Ok(s.passkeys
                .iter()
                .find(|(_, p)| p.credential_id == id)
                .cloned())
        })
    }

    fn save_passkey(&self, user_id: &str, passkey: &Passkey) -> BoxFuture<Result<()>> {
        let (user, passkey) = (user_id.to_string(), passkey.clone());
        self.with(move |s| {
            match s
                .passkeys
                .iter_mut()
                .find(|(_, p)| p.credential_id == passkey.credential_id)
            {
                Some((owner, _)) if *owner != user => Err(Error::Conflict),
                Some((_, p)) => {
                    *p = passkey;
                    Ok(())
                }
                None => {
                    per_user(&s.passkeys, &user)?;
                    s.passkeys.push((user, passkey));
                    Ok(())
                }
            }
        })
    }

    fn remove_passkey(&self, user_id: &str, credential_id: &[u8]) -> BoxFuture<Result<()>> {
        let (user, id) = (user_id.to_string(), credential_id.to_vec());
        self.with(move |s| {
            s.passkeys
                .retain(|(u, p)| !(*u == user && p.credential_id == id));
            Ok(())
        })
    }
}
