# User accounts

`zeyn::accounts` is an optional accounts system for apps that keep their own users: registration,
passwords, e-mail confirmation, password reset, lockout, roles and claims, two factor sign in,
passkeys, external logins, a default UI and JSON endpoints for SPAs and mobile apps. Its structure
follows ASP.NET Core Identity, so people who know that system find the same parts under similar
names. Apps that sign in through a single sign on proxy or an identity provider do not need it
(see `docs/auth.md`).

Everything sits behind the Cargo feature `accounts`. Without it nothing of this is compiled.

```toml
zeyn = { version = "0.0.10", features = ["accounts"] }            # memory store
zeyn = { version = "0.0.10", features = ["accounts", "postgres"] } # or "oracle"
```

The module is `zeyn::accounts` because `Identity` already names the request's user.

## Quick start

```rust
use zeyn::accounts::{self, Accounts, MailSender, MemoryAccountStore};

let accounts = Accounts::new(MemoryAccountStore::new(), config.secret("accounts.key")?)
    .public_url("https://shop.example.com")
    .sender(MailSender(mailer.clone()))
    .role("admins", [accounts::USERS_READ, accounts::USERS_WRITE, accounts::ROLES_WRITE])
    .options(|o| o.sign_in.require_confirmed_email = true);

App::new("Shop")
    .mailer(mailer)
    .accounts(accounts)
    .feature(accounts::ui().admin(true).build())
```

- `App::accounts` registers `Accounts`, `UserManager`, `RoleManager` and `SignInManager` as
  services (actions take them as arguments), makes the session cookie and bearer tokens the app's
  authenticator, checks security stamps, and serves the JSON endpoints under `/_zeyn/account`.
- `accounts::feature()` (or `accounts::ui()...build()`) adds the screens under `/account`. The
  app's `login_url`, the step up and re authentication pages and the idle sign out point there.
  The idle timeout ends the session with `POST /_zeyn/account/logout` and the CSRF token; there
  is no `GET` sign out, so a link or image on another site cannot sign a user out.
- The key must be at least 32 bytes and must not change: it seals cookies and stored
  authenticator keys and signs links. A shorter key stops the app at startup.
- `public_url` is used for links in messages and as the passkey relying party. It is never taken
  from the request's `Host` header. Without it, passkeys are off and links are app relative.

## Users and app fields

`User` has the fields of `IdentityUser`: id, user name, normalized user name, e-mail, normalized
e-mail, e-mail confirmed, password hash, security stamp, concurrency stamp, phone, phone
confirmed, two factor enabled, lockout end, lockout enabled and access failed count.

Apps add their own fields with a plain struct kept in `User::extra`:

```rust
#[derive(Serialize, Deserialize, Default)]
struct Profile { display_name: String, department: Option<String> }

let mut user = users.find_by_id(&id).await?.ok_or(Error::NotFound)?;
let mut p: Profile = user.data()?;
p.department = Some("Sales".into());
user.set_data(&p)?;
users.update(&mut user).await?;
```

Why not a generic `User<X>`: the stores, the managers, the session and every screen and action of
the default UI would all have to be generic over `X`. Zeyn actions and screens are generated,
non generic items, and services are looked up by type, so a generic user would need a second,
type erased API next to the typed one. One `User` with a typed accessor keeps one code path,
compiles once (smaller binaries) and needs no schema change per app field. The price is that app
fields are one JSON column in SQL. Fields an app wants to search or join on belong in the app's
own table keyed by the user id, which is also what keeps personal data easy to delete.

## Stores

One async trait, `AccountStore`, replaces `IUserStore`, `IUserPasswordStore`,
`IUserEmailStore`, `IUserPhoneNumberStore`, `IUserSecurityStampStore`, `IUserTwoFactorStore`,
`IUserLockoutStore`, `IUserRoleStore`, `IUserClaimStore`, `IUserLoginStore`,
`IUserAuthenticationTokenStore`, `IUserAuthenticatorKeyStore`,
`IUserTwoFactorRecoveryCodeStore`, `IUserPasskeyStore`, `IRoleStore` and `IRoleClaimStore`.
Fields of `User` (password, lockout, two factor, e-mail, phone) are saved with `update_user`; the
rest are a few methods each. A store that lacks a part does not compile, where ASP.NET Core
Identity throws `NotSupportedException` at run time.

| Store | Use |
| --- | --- |
| `MemoryAccountStore` | tests and demos; at most 10 000 users (`max_users`), 1 000 roles and 64 claims, logins, tokens or passkeys per user |
| `SqlAccountStore` | PostgreSQL or Oracle through `zeyn::sql::Db` |

`update_user` and `update_role` save only when the stored concurrency stamp is the one the
record was loaded with; otherwise `Error::Conflict`, like EF Core's concurrency token.

### SQL schema

`accounts::create_tables(dialect, prefix)` returns the statements; put them in a migration.
`SqlAccountStore::create_tables` runs them, for development and tests.

| Table (prefix `zeyn_`) | ASP.NET Core Identity | Key and indexes |
| --- | --- | --- |
| `zeyn_users` | `AspNetUsers` | `id`; unique `normalized_user_name`; index on `normalized_email`; names 256 characters |
| `zeyn_roles` | `AspNetRoles` | `id`; unique `normalized_name` |
| `zeyn_user_roles` | `AspNetUserRoles` | (`user_id`, `role_id`) |
| `zeyn_user_claims` | `AspNetUserClaims` | generated `id` |
| `zeyn_role_claims` | `AspNetRoleClaims` | generated `id` |
| `zeyn_user_logins` | `AspNetUserLogins` | (`login_provider`, `provider_key`) |
| `zeyn_user_tokens` | `AspNetUserTokens` | (`user_id`, `login_provider`, `name`) |
| `zeyn_user_passkeys` | `AspNetUserPasskeys` | `credential_id` (base64url) |

The prefix is lower case letters, digits and `_`, at most 16 characters, checked before it is
used in SQL. All values are bound parameters. Oracle needs 12c or later (identity columns).

## Managers

- `UserManager`: create, update, delete, find by id, name, e-mail or external login, list, check,
  add, change, reset and remove passwords, confirm e-mail, change e-mail and phone with tokens or
  codes, roles, claims, lockout, security stamps, purpose tokens, authenticator keys, recovery
  codes, e-mail and phone codes, external logins, passkeys and personal data.
- `RoleManager`: create, rename, delete, find, list, and role claims.
- `SignInManager`: password sign in with lockout and rate limits, the two factor flow, remembered
  devices, external logins, passkey sign in, sign out, refresh sign in and bearer tokens.

Errors that belong to a field are `Error::Validation` with messages `zeyn.accounts.error.<code>`;
the codes are the snake case of `IdentityErrorDescriber`'s (`password_too_short`,
`duplicate_user_name`, ...). Refusals are `Error::Denied`.

### Options

`Accounts::options(|o| ...)` changes `AccountOptions` (`IdentityOptions`). The defaults are
ASP.NET Core Identity's:

| Option | Default |
| --- | --- |
| `password.required_length` | 6 |
| `password.required_unique_chars` | 1 |
| `password.require_non_alphanumeric`, `require_lowercase`, `require_uppercase`, `require_digit` | `true` |
| `lockout.allowed_for_new_users` | `true` |
| `lockout.max_failed_access_attempts` | 5 |
| `lockout.default_lockout` | 5 minutes |
| `user.allowed_user_name_characters` | `a-z A-Z 0-9 - . _ @ +` |
| `user.require_unique_email` | `false` |
| `sign_in.require_confirmed_email`, `require_confirmed_phone_number` | `false` |
| `tokens.token_lifespan` | 1 day |
| `tokens.authenticator_issuer` | `Zeyn` |
| `session.expire` (sliding) | 14 days |
| `session.validation_interval` | 30 minutes |
| `session.remember_two_factor` | 14 days |
| `session.bearer_expiration` / `refresh_expiration` | 1 hour / 14 days |
| `hasher` | Argon2id, 19 MiB, 2 passes, 1 lane |
| `passkeys.timeout`, `challenge_size`, `user_verification`, `resident_key` | 5 minutes, 32, `required`, `preferred` |
| `ip_attempts` / `account_attempts` | 30 / 10 per 5 minutes (Zeyn only) |
| `registration` | `true` (Zeyn only) |

### Normalization

`accounts::normalize` is Unicode NFC followed by the Unicode default upper case mapping, never a
language's rules (`UpperInvariantLookupNormalizer`). `admin` is `ADMIN` on every server, also one
running in Turkish. The dotless `ı` and the ASCII `i` both become `I`, so `ali` and `alı` are the
same name: look alike names cannot both be registered. The dotted capital `İ` stays `İ`.

## Passwords

New hashes are Argon2id PHC strings with the OWASP parameters (19 MiB, two passes, one lane).
Hashing runs on blocking threads, at most four at a time (`MAX_HASHING`), so a burst of sign ins
can neither stall the async threads nor hold more than about 76 MiB.

ASP.NET Core Identity hashes keep working, so users migrated from a .NET app keep their
passwords. Copy `PasswordHash` into `password_hash` as it is:

| Format | Marker | Content |
| --- | --- | --- |
| v2 | `0x00` | PBKDF2 HMAC-SHA1, 1000 iterations, 16 byte salt, 32 byte subkey |
| v3 | `0x01` | PRF (0 SHA1, 1 SHA256, 2 SHA512), iteration count and salt length as big endian `u32`, salt, subkey |

A successful sign in rehashes them to Argon2id, as .NET does on `SuccessRehashNeeded`. Argon2
hashes with weaker parameters than configured are rehashed the same way. Iteration counts above
two million are refused instead of run, so a planted record cannot stall the server.
`accounts::dotnet::hash_v2` and `hash_v3` write the formats, for tests and migration tools.

`Accounts::breached_passwords` takes a `BreachedPasswords` check (such as a k-anonymity range
query or an offline list; `PasswordDenyList` is a fixed list). None is set by default, so nothing
leaves the server unless the app adds one. A failing check lets the password through.

## Tokens and codes

| Kind | .NET counterpart | How |
| --- | --- | --- |
| Purpose tokens: e-mail confirmation, password reset, e-mail change | `DataProtectorTokenProvider` | creation time plus HMAC-SHA256 over time, user id, purpose and security stamp; one day |
| E-mail and phone codes, phone number change | `EmailTokenProvider`, `PhoneNumberTokenProvider` | RFC 6238, six digits, three minute steps, two steps either way, key derived from the accounts key and the stamp |
| Authenticator codes | `AuthenticatorTokenProvider` | RFC 6238, 30 second steps, two steps either way, 160 bit key in base32 |
| Recovery codes | `GenerateNewTwoFactorRecoveryCodesAsync` | ten codes `XXXXX-XXXXX` from `23456789BCDFGHJKMNPQRTVWXY` |

Differences on purpose:

- Purpose tokens carry no readable data; the stamp is only in the MAC. Changing the password or
  the stamp voids every outstanding link at once.
- Short codes are keyed from the accounts key and the stamp, not from the stamp alone, so a stamp
  that leaks does not let anyone make codes.
- An authenticator code is accepted once: the last used step is stored.
- Recovery codes are stored as keyed hashes, never in plain text. Case and spaces do not matter.
- The authenticator key is stored sealed (AES-256-GCM) and bound to the user.

## Sessions

There is no session store on the server. The session is one sealed cookie, `zeyn_account`,
made the same way as `OidcAuth`'s: AES-256-GCM with a key derived from the accounts key, the
cookie name as associated data, `HttpOnly`, `SameSite=Lax`, `Secure` on HTTPS. It holds the user
id and name, roles, permission claims, when and how the user signed in, whether a second factor
was used, when the stamp was last checked, and a keyed fingerprint of the security stamp (never
the stamp itself).

- "Remember me" makes it persistent for `session.expire`; otherwise it is a browser session
  cookie. Either way it ends after `session.expire` without use.
- Every `session.validation_interval` the server compares the fingerprint with the stored stamp
  (`SecurityStampValidator`). A changed stamp signs the request out and deletes the cookie; an
  unchanged one renews the cookie with fresh roles. Changing the password, two factor settings,
  roles, claims, logins or passkeys changes the stamp, so other devices are signed out within one
  interval. `update_security_stamp` is "sign out everywhere".
- The identity gets the permissions of the user's roles (`Accounts::role`), the values of
  `permission` claims of the user and of the user's roles, and `signed_in_permissions`.
- A session that would be larger than 3800 bytes is refused with an error, never cut.

Bearer access tokens are the same session sealed under another name, for one hour. Their stamp is
checked at the same interval on each request. Refresh tokens last fourteen days and work only
while the stamp is unchanged.

## Two factor

- Authenticator apps: the manage screen shows a QR code drawn as SVG on the server (no script,
  no image request) and the key; a code turns two factor on and shows ten recovery codes once.
- E-mail codes go through the sender; phone codes through a sender of the app's own (`MailSender`
  drops them with a warning).
- After the password, a sealed `zeyn_account_2fa` cookie holds the pending sign in for five
  minutes, bound to the stamp. "Remember this browser" sets a sealed `zeyn_account_device` cookie
  for `remember_two_factor`, also bound to the stamp.
- Wrong codes count toward lockout, as in .NET.
- `Identity::mfa` is true after a second factor or a verified passkey, so `RequireMfa` works.
- Changing the second factor needs a recent sign in (`App::reauth_within`, five minutes unless
  set): turning it on or off, a new authenticator key, new recovery codes, adding or removing a
  passkey and removing an external login. Otherwise the call answers `401` with code
  `reauth_required`. The authenticator key is shown only after a recent sign in and only while two
  factor is off; `POST /manage/2fa` returns `sharedKey: null` otherwise.
- Refreshed bearer tokens keep the time of the original sign in, so a refresh never counts as a
  recent sign in.
- Claims of type `tenant` give a user their tenants for `App::tenancy` (see `docs/auth.md`).

## Passkeys

Passkeys sign in without a password and count as a second factor when the authenticator verified
the user. The ceremonies follow ASP.NET Core Identity 10's `PasskeyHandler`: ES256, RS256 and
EdDSA keys, user verification required, resident keys preferred, attestation `none` and
attestation statements not verified. The server checks the type, the challenge (once, from a
sealed cookie), the origin, the relying party id hash, user presence and verification, backup
flags, the algorithm, the credential id length, that the credential is new, the signature, the
user handle and a growing signature counter.

The verification is about 450 lines on `ring`, which the feature already links. `webauthn-rs`
was not used: it brings OpenSSL (a system library, awkward on Windows and in small containers)
and several megabytes for attestation formats the default flow does not verify anyway.

The browser side is `/_zeyn/account/accounts.js`, a separate asset as the CSP requires. Buttons
marked `data-zeyn-passkey` use it.

## External logins

`Accounts::external(name, OidcAuth)` (features `accounts` and `oidc`) links OpenID Connect
providers to local users, like `AspNetUserLogins`. Register
`<public url>/_zeyn/account/external/<name>/callback` at the provider.

- Signed out: a linked login signs in (with the second factor when it is on). An unknown login
  goes to "Finish signing in", which creates an account for the given e-mail and links the login.
  The address counts as confirmed only when the provider says `email_verified` and it is the same.
- An address that already has an account is refused there: sign in to it and link the provider
  from the manage screen. Accounts are never joined by e-mail alone.
- The provider's `sub` is the key. A login belongs to one user. The last way to sign in (no
  password, no other login, no passkey) cannot be unlinked.

## The default UI

`accounts::ui()` builds the feature. `registration(false)` hides registration (also set
`AccountOptions::registration` to close the endpoint), `admin(true)` adds administration and
`without("account_passkeys")` drops a screen so the app can serve its own at the same route.

| Screen | Route |
| --- | --- |
| Sign in, with passkey and provider buttons | `/account/login` |
| Register, check your e-mail | `/account/register`, `/account/check-email` |
| Sign out | `/account/logout` |
| Forgot password, reset password | `/account/forgot-password`, `/account/reset-password` |
| Confirm e-mail, resend confirmation | `/account/confirm-email`, `/account/resend-confirmation` |
| Two factor, recovery code | `/account/login-2fa`, `/account/login-recovery` |
| Lockout, access denied | `/account/lockout`, `/account/access-denied` |
| First external sign in | `/account/external-login` |
| Manage: profile, e-mail, password, two factor, recovery codes, passkeys, external logins, personal data | `/account/manage/...` |
| Users (lock, unlock, send reset link, roles), roles | `/account/admin/users`, `/account/admin/roles` |

The screens are Zeyn forms and components in English and Turkish. Moving between them, signing
in and signing out go through effects (`Effect::Go`), which load the next screen or the whole app
frame through `nav.js`, never a page load. Password and code inputs use `Control::Password`,
`Control::NewPassword` and `Control::OneTimeCode`; forms with them keep no drafts. All actions of
the UI are `ai = false`: no model ever sees or calls them.

## JSON endpoints

The equivalent of `MapIdentityApi`, under `/_zeyn/account`:

| Endpoint | Body | Answer |
| --- | --- | --- |
| `POST /register` | `email`, `password` | 200, the same for new and existing addresses |
| `POST /login?useCookies=&useSessionCookies=` | `email`, `password`, `twoFactorCode`, `twoFactorRecoveryCode` | cookie, or `{tokenType, accessToken, expiresIn, refreshToken}`; 401 with `RequiresTwoFactor`, `LockedOut`, `NotAllowed` or `Failed` |
| `POST /refresh` | `refreshToken` | new tokens, 401 after a stamp change |
| `GET /confirmEmail?userId=&code=&changedEmail=` | | 200 or 401 |
| `POST /resendConfirmationEmail` | `email` | 200 always |
| `POST /forgotPassword` | `email` | 200 always |
| `POST /resetPassword` | `email`, `resetCode`, `newPassword` | 200 or validation problem |
| `POST /manage/2fa` | `enable`, `twoFactorCode`, `resetSharedKey`, `resetRecoveryCodes`, `forgetMachine` | `sharedKey`, `recoveryCodes`, `recoveryCodesLeft`, `isTwoFactorEnabled`, `isMachineRemembered` |
| `GET`, `POST /manage/info` | `newEmail`, `newPassword`, `oldPassword` | `email`, `isEmailConfirmed` |
| `GET /manage/personalData` | | the user's data as a JSON download |
| `POST /passkeys/requestOptions`, `POST /passkeys/login` | `credentialJson` | options; cookie or tokens |
| `POST /manage/passkeys/creationOptions`, `GET`, `POST /manage/passkeys`, `POST /manage/passkeys/remove` | `credentialJson`, `name`, `credentialId` | options, list, the new passkey |
| `POST /logout` | | ends the cookie session |

Validation problems list errors by code in Pascal case (`PasswordTooShort`), as .NET does. Every
`POST` must be `application/json` (a cross site form cannot send that, and a cross site script
cannot without a CORS preflight the app does not grant). A `POST` that carries any cookie also
needs the `x-zeyn-csrf` token, unless it signs in with a bearer token; clients that sign in for
tokens send no cookies. The paths count as writes for the write rate limit.

## Security

- **No user enumeration.** Registration answers the same for a taken address (that address gets
  a "you already have an account" mail instead, after the same hashing work). Forgot password
  and resend answer the same in every case. A sign in for an unknown user checks a dummy hash, so
  it takes as long as a wrong password. ASP.NET Core Identity's default UI reports a taken user
  name; Zeyn does not.
- **Lockout and rate limits.** Five failures lock an account for five minutes. Sign in, two factor,
  registration, reset and code checks are also limited per client address and per account name
  through the app's `RateStore` (`RedisStore` shares the counts between servers).
- **CSRF** on every `POST` (the forms and actions use the runtime's token; the endpoints as above).
  `return_to` goes only to local paths; anything else goes to the start page.
- **Timing safe comparisons** for MACs, stamps, recovery codes, passkey challenges and hashes.
- **Secrets stay secret.** `User`'s `Debug` hides the hash and stamp; messages print as
  `ConfirmEmail([redacted])`; no screen, API answer, export, personal data download or AI tool
  carries a hash, stamp, key, token or code. Links and codes are never logged.
- **Audit events** for sign in, failure, lockout, sign out, registration, e-mail confirmation,
  password change and reset, two factor on and off, recovery codes, passkeys, logins, stamp
  changes, deletion and rate limiting (`Accounts::events`, else the log target `zeyn::accounts`).
  Events hold the user id and client address, never secrets.

## Costs

Measured on Windows, release build of `zeyn-example-inventory`: 5.99 MB without the feature,
7.58 MB with it (+1.6 MB: Argon2, PBKDF2 and HMAC from `ring`, Unicode normalization tables, the
QR encoder, WebAuthn and the screens). The server keeps no session state; memory grows only with
the store (`MemoryAccountStore`, about 1 KB per user) and, while hashing, 19 MiB per Argon2 hash,
at most four at once.

## Coming from ASP.NET Core Identity

| ASP.NET Core Identity | Zeyn | Source read (dotnet/aspnetcore `src/Identity`) |
| --- | --- | --- |
| `IdentityUser` | `User` (`extra` for app fields) | `Extensions.Stores/src/IdentityUser.cs` |
| `IdentityRole` | `Role` | `Extensions.Stores/src/IdentityRole.cs` |
| `IdentityUserClaim`, `IdentityRoleClaim` | `Claim` | `Extensions.Stores/src/IdentityUserClaim.cs`, `IdentityRoleClaim.cs` |
| `IdentityUserLogin`, `UserLoginInfo` | `UserLogin` | `Extensions.Stores/src/IdentityUserLogin.cs`, `Extensions.Core/src/UserLoginInfo.cs` |
| `IdentityUserToken` | `AccountStore::token` | `Extensions.Stores/src/IdentityUserToken.cs` |
| `IdentityUserPasskey`, `UserPasskeyInfo` | `Passkey` | `Extensions.Stores/src/IdentityUserPasskey.cs`, `IdentityPasskeyData.cs` |
| `IUserStore` and the other `I*Store` interfaces | `AccountStore` | `Extensions.Core/src/I*Store.cs`, `Extensions.Stores/src/UserStoreBase.cs`, `RoleStoreBase.cs` |
| `[AspNetUserStore]` tokens `AuthenticatorKey`, `RecoveryCodes` | `INTERNAL_PROVIDER`, `AUTHENTICATOR_KEY`, `RECOVERY_CODES` | `Extensions.Stores/src/UserStoreBase.cs` |
| `IdentityUserContext`, `IdentityDbContext` | `create_tables`, `SqlAccountStore` | `EntityFrameworkCore/src/IdentityUserContext.cs`, `IdentityDbContext.cs` |
| `UserManager<TUser>` | `UserManager` | `Extensions.Core/src/UserManager.cs` |
| `RoleManager<TRole>` | `RoleManager` | `Extensions.Core/src/RoleManager.cs` |
| `SignInManager<TUser>` | `SignInManager` | `Core/src/SignInManager.cs` |
| `SignInResult` | `SignInResult` | `Extensions.Core/src/SignInResult.cs` |
| `IdentityResult`, `IdentityErrorDescriber` | `Error::Validation` with `zeyn.accounts.error.*` | `Extensions.Core/src/IdentityResult.cs`, `IdentityErrorDescriber.cs` |
| `IdentityOptions` | `AccountOptions` | `Extensions.Core/src/IdentityOptions.cs` |
| `PasswordOptions` | `PasswordOptions` | `Extensions.Core/src/PasswordOptions.cs` |
| `LockoutOptions` | `LockoutOptions` | `Extensions.Core/src/LockoutOptions.cs` |
| `UserOptions` | `UserOptions` | `Extensions.Core/src/UserOptions.cs` |
| `SignInOptions` | `SignInOptions` | `Extensions.Core/src/SignInOptions.cs` |
| `TokenOptions`, `DataProtectionTokenProviderOptions` | `TokenOptions` | `Extensions.Core/src/TokenOptions.cs`, `Core/src/DataProtectionTokenProviderOptions.cs` |
| `StoreOptions`, `IdentitySchemaVersions` | table prefix, one schema (version 3 with passkeys) | `Extensions.Core/src/StoreOptions.cs`, `IdentitySchemaVersions.cs` |
| `ClaimsIdentityOptions`, `UserClaimsPrincipalFactory` | `Identity` built from the session | `Extensions.Core/src/ClaimsIdentityOptions.cs`, `UserClaimsPrincipalFactory.cs` |
| `PasswordHasher`, `PasswordHasherOptions`, `PasswordHasherCompatibilityMode` | `PasswordHasher` (Argon2id), `accounts::dotnet` | `Extensions.Core/src/PasswordHasher.cs`, `PasswordHasherOptions.cs`, `PasswordHasherCompatibilityMode.cs` |
| `PasswordVerificationResult` | `Verification` | `Extensions.Core/src/PasswordVerificationResult.cs` |
| `PasswordValidator` | `PasswordOptions::check`, `UserManager::validate_password` | `Extensions.Core/src/PasswordValidator.cs` |
| `UserValidator`, `RoleValidator` | `UserManager::validate`, `RoleManager::create` | `Extensions.Core/src/UserValidator.cs`, `RoleValidator.cs` |
| `UpperInvariantLookupNormalizer` | `normalize` | `Extensions.Core/src/UpperInvariantLookupNormalizer.cs` |
| `DataProtectorTokenProvider` | `Purpose` tokens | `Core/src/DataProtectorTokenProvider.cs` |
| `TotpSecurityStampBasedTokenProvider`, `Rfc6238AuthenticationService` | short codes | `Extensions.Core/src/TotpSecurityStampBasedTokenProvider.cs`, `Rfc6238AuthenticationService.cs` |
| `EmailTokenProvider`, `PhoneNumberTokenProvider` | `generate_email_code`, `generate_phone_code` | `Extensions.Core/src/EmailTokenProvider.cs`, `PhoneNumberTokenProvider.cs` |
| `AuthenticatorTokenProvider` | `verify_authenticator_code` | `Extensions.Core/src/AuthenticatorTokenProvider.cs` |
| `IEmailSender` | `AccountSender`, `MailSender` | `Core/src/IEmailSender.cs` |
| Application cookie, `SecurityStampValidator`, `SecurityStampValidatorOptions` | `zeyn_account` cookie, `SessionOptions` | `Core/src/SecurityStampValidator.cs`, `SecurityStampValidatorOptions.cs`, `IdentityCookiesBuilderExtensions.cs` |
| `TwoFactorUserId`, `TwoFactorRememberMe` cookies | `zeyn_account_2fa`, `zeyn_account_device` | `Core/src/IdentityConstants.cs`, `TwoFactorSecurityStampValidator.cs` |
| `PasskeyHandler`, `IdentityPasskeyOptions` | `accounts::passkey`, `PasskeyOptions` | `Core/src/PasskeyHandler.cs`, `IdentityPasskeyOptions.cs`, `Core/src/Passkeys/` |
| `ExternalLoginInfo` | `Accounts::external`, pending external cookie | `Core/src/ExternalLoginInfo.cs` |
| `MapIdentityApi` | `/_zeyn/account/*` | `Core/src/IdentityApiEndpointRouteBuilderExtensions.cs`, `Core/src/Data/` |
| `PersonalDataAttribute` | `UserManager::personal_data` | `Extensions.Core/src/PersonalDataAttribute.cs` |
| Identity UI pages | `accounts::ui()` | `UI/src/Areas/Identity/Pages/V5/Account/` |

What is deliberately different: Argon2id as the default hash; a stateless sealed session cookie
instead of an authentication ticket; tokens that never carry the stamp in readable form; hashed
recovery codes; single use authenticator codes; one store trait; no user enumeration in the
default UI; rate limits next to lockout; and passkeys that also count as a second factor when the
authenticator verified the user.

## Testing

- `crates/zeyn/tests/accounts.rs`: managers, the policy, .NET v2 and v3 hashes and their
  migration, purpose tokens and their expiry, lockout and events, constant time for missing users,
  recovery and authenticator codes, roles to permissions, the JSON endpoints, the security stamp
  sign out, bearer tokens, two factor, CSRF and content type, rate limits, secrets in output, the
  screens and external logins.
- `crates/zeyn/tests/accounts_passkeys.rs`: registration and sign in against a software ES256
  authenticator, and the refusals for a reused challenge, a counter that does not grow, a changed
  signature and another origin.
- `crates/zeyn/tests/accounts_sql.rs`: the schema for both dialects, and a round trip against a
  database when `ZEYN_TEST_PG` (and with the `oracle` feature `ZEYN_TEST_ORACLE`) is set, as in
  `docs/database.md`.

The showcase runs with accounts instead of the development sign in:

```
NORTHWIND_ACCOUNTS__ENABLED=1 NORTHWIND_ACCOUNTS__KEY=<32 or more bytes> \
NORTHWIND_ACCOUNTS__PUBLIC_URL=http://localhost:3002 \
NORTHWIND_MAIL__TRANSPORT=file NORTHWIND_MAIL__FILE__DIR=./mail \
cargo run -p zeyn-example-showcase
```

`NORTHWIND_ACCOUNTS__ADMIN_EMAIL` and `NORTHWIND_ACCOUNTS__ADMIN_PASSWORD` add an administrator.
Passkeys need `localhost` or HTTPS; a bare IP address is not a valid relying party.
