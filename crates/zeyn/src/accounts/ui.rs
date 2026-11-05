//! The default UI of the accounts system as a [`Feature`] (the pages of ASP.NET Core Identity's
//! default UI): sign in, registration, password recovery, e-mail confirmation, two factor and
//! recovery code sign in, lockout and access denied, the manage pages (profile, e-mail,
//! password, two factor with a QR code, recovery codes, passkeys, external logins, personal
//! data) and optional administration of users and roles. Built from Zeyn screens and forms; every
//! move between them goes through effects, never a page load.
//!
//! ```ignore
//! App::new("Shop").accounts(accounts).feature(accounts::ui().registration(false).without("account_passkeys").admin(true).build())
//! ```

use super::signin::SESSION;
use super::{
    Accounts, SignInManager, SignInResult, TwoFactorKind, User, UserManager, denied, invalid, scope,
};
use crate::prelude::*;
use crate::render::{Render, escape};
use crate::screen::Component;
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

/// Permission to see the user administration.
pub const USERS_READ: &str = "accounts.users.read";
/// Permission to lock, unlock and reset users and change their roles.
pub const USERS_WRITE: &str = "accounts.users.write";
/// Permission to create and delete roles.
pub const ROLES_WRITE: &str = "accounts.roles.write";

/// Name of the cookie that carries freshly made recovery codes to the screen that shows them once.
const CODES: &str = "zeyn_account_codes";

/// Builder of the default UI: which screens to serve.
pub struct AccountsUi {
    registration: bool,
    admin: bool,
    off: Vec<&'static str>,
}

/// The default UI with every screen except administration.
pub fn ui() -> AccountsUi {
    AccountsUi {
        registration: true,
        admin: false,
        off: Vec::new(),
    }
}

/// The default UI as a feature: [`ui`] with its defaults.
pub fn feature() -> Feature {
    ui().build()
}

impl AccountsUi {
    /// Shows the registration screen (on by default). Also turn off
    /// `AccountOptions::registration` to close the JSON endpoint.
    pub fn registration(mut self, on: bool) -> Self {
        self.registration = on;
        self
    }

    /// Adds the user and role administration screens, guarded by [`USERS_READ`],
    /// [`USERS_WRITE`] and [`ROLES_WRITE`].
    pub fn admin(mut self, on: bool) -> Self {
        self.admin = on;
        self
    }

    /// Leaves out a screen by name (such as `account_passkeys`), to replace it with the app's own
    /// screen at the same route or to drop it.
    pub fn without(mut self, screen: &'static str) -> Self {
        self.off.push(screen);
        self
    }

    /// The feature.
    pub fn build(self) -> Feature {
        let on = |name: &str| !self.off.contains(&name);
        let mut f = Feature::new("accounts");
        macro_rules! add {
            ($($s:ident),*) => { $( if on(stringify!($s)) { f = f.screen($s); } )* };
        }
        add!(
            account_login,
            account_logout,
            account_forgot_password,
            account_reset_password,
            account_confirm_email,
            account_resend_confirmation
        );
        add!(
            account_check_email,
            account_login_2fa,
            account_login_recovery,
            account_lockout,
            account_access_denied
        );
        add!(
            account_manage,
            account_email,
            account_password,
            account_two_factor,
            account_recovery_codes,
            account_passkeys,
            account_logins,
            account_personal_data
        );
        if self.registration && on("account_register") {
            f = f.screen(account_register);
        }
        #[cfg(feature = "oidc")]
        if on("account_external_login") {
            f = f.screen(account_external_login);
        }
        if self.admin {
            add!(
                account_admin_users,
                account_admin_user_roles,
                account_admin_roles
            );
            f = f.menu(|m| {
                m.group(t!("zeyn.accounts.admin.menu"), Icon::Table)
                    .item(t!("zeyn.accounts.admin.users"), account_admin_users)
                    .item(t!("zeyn.accounts.admin.roles"), account_admin_roles)
            });
        }
        f
    }
}

/// What a sign in step does next: go to `url`, reloading the app frame when the session changed.
#[derive(Clone, Debug, Serialize)]
pub struct Next {
    pub url: String,
    pub frame: bool,
}

fn go(url: String, frame: bool) -> Result<Next> {
    Ok(Next { url, frame })
}

fn path(p: &str) -> String {
    format!("{}{p}", scope::prefix())
}

/// The local path to go to after signing in: `return_to` when it is local, else the start page.
fn back(return_to: &str) -> String {
    crate::auth::safe_return(return_to)
        .map(str::to_string)
        .unwrap_or_else(|| path("/"))
}

fn with_params(p: &str, params: Json) -> String {
    format!("{}?params={}", path(p), super::pct(&params.to_string()))
}

/// Raw markup for parts no component covers (QR code, code lists, links, passkey buttons).
/// Every value in it is escaped by the caller.
struct Markup(Box<dyn Fn(&Render) -> String + Send + Sync>);

impl Component for Markup {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&(self.0)(r));
    }

    fn schema(&self) -> Json {
        json!({ "type": "markup" })
    }
}

fn markup(f: impl Fn(&Render) -> String + Send + Sync + 'static) -> Node {
    Node::new(Markup(Box::new(f)))
}

fn tx(r: &Render, key: &str) -> String {
    r.text(&Text::key(format!("zeyn.accounts.{key}")))
}

/// A link to an app path; `nav.js` loads it in place.
fn link(key: &'static str, to: &'static str) -> Node {
    let href = path(to);
    markup(move |r| format!(r#"<p><a href="{}">{}</a></p>"#, escape(&href), tx(r, key)))
}

/// The signed in user of the request.
async fn me(accounts: &Accounts) -> Result<User> {
    let id = scope::cookie(SESSION)
        .and_then(|v| accounts.sign_in().open_session(&v))
        .map(|s| s.id)
        .ok_or(Error::Forbidden)?;
    accounts
        .users()
        .find_by_id(&id)
        .await?
        .ok_or(Error::Forbidden)
}

/// A screen with the narrow account layout.
fn page(title: &'static str) -> Screen {
    Screen::new(Text::key(format!("zeyn.accounts.{title}")))
}

/// The navigation of the manage screens, with the current one marked.
fn manage_nav(current: &'static str) -> Node {
    const ITEMS: [(&str, &str); 7] = [
        ("manage.profile", "/account/manage"),
        ("manage.email", "/account/manage/email"),
        ("manage.password", "/account/manage/password"),
        ("manage.two_factor", "/account/manage/two-factor"),
        ("manage.passkeys", "/account/manage/passkeys"),
        ("manage.logins", "/account/manage/logins"),
        ("manage.personal_data", "/account/manage/personal-data"),
    ];
    let prefix = scope::prefix();
    markup(move |r| {
        let items: String = ITEMS
            .iter()
            .map(|(key, to)| {
                let mark = if *to == current {
                    r#" aria-current="page""#
                } else {
                    ""
                };
                format!(
                    r#"<li><a href="{}{}"{mark}>{}</a></li>"#,
                    escape(&prefix),
                    to,
                    tx(r, key)
                )
            })
            .collect();
        format!(
            r#"<nav class="z-account-nav" aria-label="{}"><ul>{items}</ul></nav>"#,
            tx(r, "manage.title")
        )
    })
}

/// A notice chosen by a fixed code from the screen's params; unknown codes show nothing, so a link
/// can never put its own text on the page.
fn notice(code: Option<&str>) -> Option<Node> {
    let key = match code? {
        "confirmed" => "notice.confirmed",
        "reset" => "notice.reset",
        "signed_out" => "notice.signed_out",
        "deleted" => "notice.deleted",
        "external" => "error.external",
        "check_email" => "notice.check_email",
        _ => return None,
    };
    Some(Node::new(Alert::info(Text::key(format!(
        "zeyn.accounts.{key}"
    )))))
}

/// Sign in form.
#[model]
pub struct AccountLogin {
    #[rule(required, max_len = 254)]
    pub user_name: String,
    #[rule(required, max_len = 1024)]
    pub password: String,
    pub remember_me: bool,
    pub return_to: String,
}

/// Where the sign in screen sends the user afterwards, and a notice to show.
#[params]
pub struct LoginParams {
    pub return_to: Option<String>,
    pub notice: Option<String>,
    pub error: Option<String>,
}

/// Signs in with a user name or e-mail and a password; lockout counts failures.
#[action(public, ai = false)]
pub async fn account_sign_in(input: Valid<AccountLogin>, sign_in: SignInManager) -> Result<Next> {
    match sign_in
        .password_sign_in(&input.user_name, &input.password, input.remember_me, true)
        .await?
    {
        SignInResult::Succeeded => go(back(&input.return_to), true),
        SignInResult::RequiresTwoFactor => go(
            with_params(
                "/account/login-2fa",
                json!({ "return_to": input.return_to, "remember": input.remember_me }),
            ),
            false,
        ),
        SignInResult::LockedOut => go(path("/account/lockout"), false),
        SignInResult::NotAllowed => Err(denied("not_allowed")),
        SignInResult::Failed => Err(denied("invalid_login")),
    }
}

fn next_effect(n: &Result<Next>, ui: &mut UiCtx) {
    if let Ok(n) = n {
        ui.effect(Effect::go(&n.url, n.frame));
    }
}

/// A paragraph of catalog text.
fn para(key: &'static str) -> Node {
    markup(move |r| format!("<p>{}</p>", r.text(&Text::key(key))))
}

#[screen(route = "/account/login", public)]
pub async fn account_login(p: LoginParams, accounts: Accounts) -> Result<Screen> {
    let return_to = p.return_to.clone().unwrap_or_default();
    let mut body = Stack::new();
    if let Some(n) = notice(p.notice.as_deref()).or_else(|| notice(p.error.as_deref())) {
        body = body.add(n);
    }
    body = body.add(
        Form::of(AccountLogin {
            return_to: return_to.clone(),
            ..AccountLogin::default()
        })
        .field(AccountLogin::user_name)
        .field(AccountLogin::password)
        .control(AccountLogin::password, Control::Password)
        .field(AccountLogin::remember_me)
        .submit(t!("zeyn.accounts.login.submit"), account_sign_in)
        .on_success(next_effect),
    );
    if accounts.settings().passkeys.enabled && accounts.0.public_url.is_some() {
        let back = back(&return_to);
        body = body.add(markup(move |r| {
            format!(
                r#"<p><button type="button" class="z-button" data-zeyn-passkey="login" data-return="{}" data-failed="{}" data-unsupported="{}">{}</button></p><p class="z-field-error" data-zeyn-passkey-error aria-live="polite"></p>"#,
                escape(&back),
                tx(r, "passkeys.failed"),
                tx(r, "passkeys.unsupported"),
                tx(r, "login.passkey")
            )
        }));
    }
    let providers = accounts.external_names();
    if !providers.is_empty() {
        let prefix = scope::prefix();
        let rt = super::pct(&return_to);
        body = body.add(markup(move |r| {
            let buttons: String = providers
                .iter()
                .map(|p| format!(r#"<li><a class="z-button" href="{}/_zeyn/account/external/{}?return_to={rt}">{} {}</a></li>"#, escape(&prefix), escape(p), tx(r, "login.with"), escape(p)))
                .collect();
            format!(r#"<section aria-labelledby="z-external-title"><h2 id="z-external-title">{}</h2><ul class="z-account-providers">{buttons}</ul></section>"#, tx(r, "login.external"))
        }));
    }
    body = body
        .add(link("login.forgot", "/account/forgot-password"))
        .add(link("login.resend", "/account/resend-confirmation"));
    if accounts.settings().registration {
        body = body.add(link("login.register", "/account/register"));
    }
    Ok(page("login.title").body(body))
}

/// Registration form.
#[model]
pub struct AccountRegister {
    #[rule(required, email, max_len = 254)]
    pub email: String,
    #[rule(required, max_len = 1024)]
    pub password: String,
    #[rule(required, max_len = 1024)]
    pub confirm_password: String,
}

/// Creates an account and sends the confirmation link. The answer is the same when the address
/// already has an account; that address gets a notice instead.
#[action(public, ai = false)]
pub async fn account_sign_up(
    input: Valid<AccountRegister>,
    accounts: Accounts,
    ctx: Ctx,
) -> Result<Next> {
    if input.password != input.confirm_password {
        return Err(invalid("confirm_password", "password_confirm_mismatch"));
    }
    crate::server::accounts::register_user(&ctx, &accounts, &input.email, &input.password).await?;
    go(path("/account/check-email"), false)
}

#[screen(route = "/account/register", public)]
pub fn account_register(_: ()) -> Screen {
    page("register.title").body(
        Stack::new()
            .add(
                Form::of(AccountRegister::default())
                    .field(AccountRegister::email)
                    .field(AccountRegister::password)
                    .control(AccountRegister::password, Control::NewPassword)
                    .field(AccountRegister::confirm_password)
                    .control(AccountRegister::confirm_password, Control::NewPassword)
                    .submit(t!("zeyn.accounts.register.submit"), account_sign_up)
                    .on_success(next_effect),
            )
            .add(link("register.login", "/account/login")),
    )
}

#[screen(route = "/account/check-email", public)]
pub fn account_check_email(_: ()) -> Screen {
    page("check_email.title").body(
        Stack::new()
            .add(Alert::info(t!("zeyn.accounts.check_email.body")))
            .add(link("register.login", "/account/login")),
    )
}

/// E-mail form for password recovery and confirmation links.
#[model]
pub struct AccountEmailOnly {
    #[rule(required, email, max_len = 254)]
    pub email: String,
}

/// Sends a password reset link when the address has a confirmed account; answers the same either way.
#[action(public, ai = false)]
pub async fn account_forgot(
    input: Valid<AccountEmailOnly>,
    accounts: Accounts,
    ctx: Ctx,
) -> Result<Next> {
    crate::server::accounts::forgot_password(&ctx, &accounts, &input.email).await?;
    go(path("/account/check-email"), false)
}

#[screen(route = "/account/forgot-password", public)]
pub fn account_forgot_password(_: ()) -> Screen {
    page("forgot.title").body(
        Stack::new()
            .add(para("zeyn.accounts.forgot.intro"))
            .add(
                Form::of(AccountEmailOnly::default())
                    .field(AccountEmailOnly::email)
                    .submit(t!("zeyn.accounts.forgot.submit"), account_forgot)
                    .on_success(next_effect),
            ),
    )
}

/// Sends the confirmation link again; answers the same either way.
#[action(public, ai = false)]
pub async fn account_resend(
    input: Valid<AccountEmailOnly>,
    accounts: Accounts,
    ctx: Ctx,
) -> Result<Next> {
    crate::server::accounts::resend_confirmation(&ctx, &accounts, &input.email).await?;
    go(path("/account/check-email"), false)
}

#[screen(route = "/account/resend-confirmation", public)]
pub fn account_resend_confirmation(_: ()) -> Screen {
    page("resend.title").body(
        Form::of(AccountEmailOnly::default())
            .field(AccountEmailOnly::email)
            .submit(t!("zeyn.accounts.resend.submit"), account_resend)
            .on_success(next_effect),
    )
}

/// The link of a reset or confirmation message.
#[params]
pub struct LinkParams {
    pub user: Option<String>,
    pub code: Option<String>,
    pub email: Option<String>,
}

/// New password from a reset link.
#[model]
pub struct AccountReset {
    #[rule(required, max_len = 1024)]
    pub password: String,
    #[rule(required, max_len = 1024)]
    pub confirm_password: String,
    pub user: String,
    pub code: String,
}

/// Sets a new password with a reset link.
#[action(public, ai = false)]
pub async fn account_reset(
    input: Valid<AccountReset>,
    users: UserManager,
    sign_in: SignInManager,
) -> Result<Next> {
    sign_in.throttle_ip().await?;
    if input.password != input.confirm_password {
        return Err(invalid("confirm_password", "password_confirm_mismatch"));
    }
    let Some(mut user) = users.find_by_id(&input.user).await? else {
        return Err(denied("invalid_token"));
    };
    users
        .reset_password(&mut user, &input.code, &input.password)
        .await?;
    go(
        with_params("/account/login", json!({ "notice": "reset" })),
        false,
    )
}

#[screen(route = "/account/reset-password", public)]
pub fn account_reset_password(p: LinkParams) -> Screen {
    let model = AccountReset {
        user: p.user.unwrap_or_default(),
        code: p.code.unwrap_or_default(),
        ..AccountReset::default()
    };
    page("reset.title").body(
        Form::of(model)
            .field(AccountReset::password)
            .control(AccountReset::password, Control::NewPassword)
            .field(AccountReset::confirm_password)
            .control(AccountReset::confirm_password, Control::NewPassword)
            .submit(t!("zeyn.accounts.reset.submit"), account_reset)
            .on_success(next_effect),
    )
}

/// Confirms an e-mail address, or an e-mail change, from the link in the message.
#[screen(route = "/account/confirm-email", public)]
pub async fn account_confirm_email(
    p: LinkParams,
    users: UserManager,
    sign_in: SignInManager,
) -> Result<Screen> {
    sign_in.throttle_ip().await?;
    let found = match &p.user {
        Some(id) => users.find_by_id(id).await?,
        None => None,
    };
    let done = match (found, p.code.as_deref()) {
        (Some(mut user), Some(code)) => match p.email.as_deref() {
            Some(new) => {
                let same = user.email.as_deref() == Some(user.user_name.as_str());
                users
                    .change_email(&mut user, new, code, same)
                    .await
                    .is_ok()
            }
            None => users
                .confirm_email(&mut user, code)
                .await
                .is_ok(),
        },
        _ => false,
    };
    let alert = if done {
        Alert::success(t!("zeyn.accounts.confirm.done"))
    } else {
        Alert::danger(t!("zeyn.accounts.confirm.failed"))
    };
    Ok(page("confirm.title").body(
        Stack::new()
            .add(alert)
            .add(link("register.login", "/account/login")),
    ))
}

/// Where a two factor step goes afterwards.
#[params]
pub struct TwoFactorParams {
    pub return_to: Option<String>,
    pub remember: Option<bool>,
}

/// A code from an authenticator app, e-mail or text message.
#[model]
pub struct AccountCode {
    #[rule(required, max_len = 16)]
    pub code: String,
    pub remember_machine: bool,
    pub return_to: String,
}

/// Completes a sign in with an authenticator code.
#[action(public, ai = false)]
pub async fn account_verify_code(
    input: Valid<AccountCode>,
    sign_in: SignInManager,
) -> Result<Next> {
    match sign_in
        .two_factor_sign_in(
            TwoFactorKind::Authenticator,
            &input.code,
            input.remember_machine,
        )
        .await?
    {
        SignInResult::Succeeded => go(back(&input.return_to), true),
        SignInResult::LockedOut => go(path("/account/lockout"), false),
        _ => Err(invalid("code", "invalid_code")),
    }
}

/// A code sent by e-mail, a form of its own so the screen has unique field ids.
#[model]
pub struct AccountEmailCode {
    #[rule(required, max_len = 16)]
    pub code: String,
    pub remember_machine: bool,
    pub return_to: String,
}

/// Completes a sign in with a code sent by e-mail.
#[action(public, ai = false)]
pub async fn account_verify_email_code(
    input: Valid<AccountEmailCode>,
    sign_in: SignInManager,
) -> Result<Next> {
    match sign_in
        .two_factor_sign_in(TwoFactorKind::Email, &input.code, input.remember_machine)
        .await?
    {
        SignInResult::Succeeded => go(back(&input.return_to), true),
        SignInResult::LockedOut => go(path("/account/lockout"), false),
        _ => Err(invalid("code", "invalid_code")),
    }
}

/// Sends a sign in code to the user's confirmed e-mail address.
#[action(public, ai = false)]
pub async fn account_send_email_code(_input: (), sign_in: SignInManager, ctx: Ctx) -> Result<()> {
    sign_in
        .send_two_factor_code(&ctx, TwoFactorKind::Email)
        .await
}

#[screen(route = "/account/login-2fa", public)]
pub async fn account_login_2fa(p: TwoFactorParams, sign_in: SignInManager) -> Result<Screen> {
    let Some(user) = sign_in.two_factor_user().await? else {
        return Ok(page("two_factor.title").body(
            Stack::new()
                .add(Alert::warning(t!("zeyn.accounts.error.two_factor_expired")))
                .add(link("register.login", "/account/login")),
        ));
    };
    let kinds = sign_in
        .0
        .users()
        .two_factor_kinds(&user)
        .await?;
    let return_to = p.return_to.unwrap_or_default();
    let model = AccountCode {
        return_to: return_to.clone(),
        ..AccountCode::default()
    };
    let mut body = Stack::new();
    if kinds.contains(&TwoFactorKind::Authenticator) {
        body = body
            .add(para("zeyn.accounts.two_factor.intro"))
            .add(
                Form::of(model.clone())
                    .field(AccountCode::code)
                    .control(AccountCode::code, Control::OneTimeCode)
                    .field(AccountCode::remember_machine)
                    .submit(t!("zeyn.accounts.two_factor.submit"), account_verify_code)
                    .on_success(next_effect),
            );
    }
    if kinds.contains(&TwoFactorKind::Email) {
        body = body
            .add(para("zeyn.accounts.two_factor.email_intro"))
            .add(
                ActionButton::new(
                    t!("zeyn.accounts.two_factor.send_email"),
                    account_send_email_code,
                    (),
                )
                .on_success(|_, ui| ui.toast(t!("zeyn.accounts.two_factor.sent"))),
            )
            .add(
                Form::of(AccountEmailCode {
                    return_to: model.return_to.clone(),
                    ..AccountEmailCode::default()
                })
                .field(AccountEmailCode::code)
                .control(AccountEmailCode::code, Control::OneTimeCode)
                .field(AccountEmailCode::remember_machine)
                .submit(
                    t!("zeyn.accounts.two_factor.submit_email"),
                    account_verify_email_code,
                )
                .on_success(next_effect),
            );
    }
    let recovery = with_params("/account/login-recovery", json!({ "return_to": return_to }));
    body = body.add(markup(move |r| {
        format!(
            r#"<p><a href="{}">{}</a></p>"#,
            escape(&recovery),
            tx(r, "two_factor.use_recovery")
        )
    }));
    Ok(page("two_factor.title").body(body))
}

/// A recovery code.
#[model]
pub struct AccountRecovery {
    #[rule(required, max_len = 32)]
    pub code: String,
    pub return_to: String,
}

/// Completes a sign in with a recovery code, which is used up.
#[action(public, ai = false)]
pub async fn account_use_recovery(
    input: Valid<AccountRecovery>,
    sign_in: SignInManager,
) -> Result<Next> {
    match sign_in
        .recovery_code_sign_in(&input.code)
        .await?
    {
        SignInResult::Succeeded => go(back(&input.return_to), true),
        SignInResult::LockedOut => go(path("/account/lockout"), false),
        _ => Err(invalid("code", "invalid_recovery_code")),
    }
}

#[screen(route = "/account/login-recovery", public)]
pub fn account_login_recovery(p: TwoFactorParams) -> Screen {
    page("recovery.title").body(
        Stack::new()
            .add(para("zeyn.accounts.recovery.intro"))
            .add(
                Form::of(AccountRecovery {
                    return_to: p.return_to.unwrap_or_default(),
                    ..AccountRecovery::default()
                })
                .field(AccountRecovery::code)
                .control(AccountRecovery::code, Control::OneTimeCode)
                .submit(t!("zeyn.accounts.recovery.submit"), account_use_recovery)
                .on_success(next_effect),
            ),
    )
}

#[screen(route = "/account/lockout", public)]
pub fn account_lockout(_: ()) -> Screen {
    page("lockout.title").body(
        Stack::new()
            .add(Alert::danger(t!("zeyn.accounts.lockout.body")))
            .add(link("login.forgot", "/account/forgot-password")),
    )
}

#[screen(route = "/account/access-denied", public)]
pub fn account_access_denied(_: ()) -> Screen {
    page("denied.title").body(Alert::warning(t!("zeyn.accounts.denied.body")))
}

/// Signs out of this browser.
#[action(public, ai = false)]
pub async fn account_sign_out(_input: (), accounts: Accounts) -> Result<Next> {
    let user = me(&accounts).await.ok();
    accounts
        .sign_in()
        .sign_out(user.as_ref().map(|u| u.id.as_str()));
    go(
        with_params("/account/login", json!({ "notice": "signed_out" })),
        true,
    )
}

#[screen(route = "/account/logout", public)]
pub fn account_logout(_: ()) -> Screen {
    page("logout.title").body(
        Stack::new()
            .add(para("zeyn.accounts.logout.body"))
            .add(
                ActionButton::new(t!("zeyn.accounts.logout.submit"), account_sign_out, ())
                    .on_success(next_effect),
            ),
    )
}

/// Profile: the phone number.
#[model]
pub struct AccountProfile {
    #[readonly]
    pub user_name: String,
    #[rule(max_len = 32)]
    pub phone_number: Option<String>,
}

/// Saves the profile. A changed phone number is not confirmed until a code proves it.
#[action(public, ai = false)]
pub async fn account_save_profile(input: Valid<AccountProfile>, accounts: Accounts) -> Result<()> {
    let mut user = me(&accounts).await?;
    let phone = input
        .phone_number
        .clone()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    if phone != user.phone_number {
        if phone.as_deref().is_some_and(|p| {
            !p.chars()
                .all(|c| c.is_ascii_digit() || " +-()".contains(c))
        }) {
            return Err(invalid("phone_number", "invalid_phone"));
        }
        user.phone_number = phone;
        user.phone_number_confirmed = false;
        accounts
            .users()
            .update(&mut user)
            .await?;
    }
    Ok(())
}

#[screen(route = "/account/manage", public)]
pub async fn account_manage(_: (), accounts: Accounts) -> Result<Screen> {
    let user = me(&accounts).await?;
    Ok(page("manage.profile").body(
        Stack::new()
            .add(manage_nav("/account/manage"))
            .add(
                Form::of(AccountProfile {
                    user_name: user.user_name.clone(),
                    phone_number: user.phone_number.clone(),
                })
                .field(AccountProfile::user_name)
                .field(AccountProfile::phone_number)
                .submit(t!("zeyn.accounts.save"), account_save_profile)
                .on_success(|_, ui| ui.toast(t!("zeyn.accounts.saved"))),
            ),
    ))
}

/// A new e-mail address.
#[model]
pub struct AccountNewEmail {
    #[rule(required, email, max_len = 254)]
    pub new_email: String,
}

/// Sends a confirmation link to the new address; the address changes when the link is opened.
#[action(public, ai = false)]
pub async fn account_change_email(
    input: Valid<AccountNewEmail>,
    accounts: Accounts,
    ctx: Ctx,
) -> Result<()> {
    let user = me(&accounts).await?;
    accounts.sign_in().throttle_ip().await?;
    if Some(input.new_email.as_str()) == user.email.as_deref() {
        return Err(invalid("new_email", "email_unchanged"));
    }
    crate::server::accounts::send_confirmation(&ctx, &accounts, &user, Some(&input.new_email)).await
}

/// Sends the confirmation link for the current address again.
#[action(public, ai = false)]
pub async fn account_send_verification(_input: (), accounts: Accounts, ctx: Ctx) -> Result<()> {
    let user = me(&accounts).await?;
    accounts.sign_in().throttle_ip().await?;
    crate::server::accounts::send_confirmation(&ctx, &accounts, &user, None).await
}

#[screen(route = "/account/manage/email", public)]
pub async fn account_email(_: (), accounts: Accounts) -> Result<Screen> {
    let user = me(&accounts).await?;
    let current = user.email.clone().unwrap_or_default();
    let confirmed = user.email_confirmed;
    let mut body = Stack::new()
        .add(manage_nav("/account/manage/email"))
        .add(markup(move |r| {
            let state = if confirmed {
                tx(r, "email.confirmed")
            } else {
                tx(r, "email.unconfirmed")
            };
            format!(
                r#"<p>{} <strong>{}</strong> ({state})</p>"#,
                tx(r, "email.current"),
                escape(&current)
            )
        }));
    if !confirmed {
        body = body.add(
            ActionButton::new(
                t!("zeyn.accounts.email.send_verification"),
                account_send_verification,
                (),
            )
            .on_success(|_, ui| ui.toast(t!("zeyn.accounts.email.sent"))),
        );
    }
    body = body.add(
        Form::of(AccountNewEmail::default())
            .field(AccountNewEmail::new_email)
            .submit(t!("zeyn.accounts.email.change"), account_change_email)
            .on_success(|_, ui| ui.toast(t!("zeyn.accounts.email.sent"))),
    );
    Ok(page("manage.email").body(body))
}

/// Password change.
#[model]
pub struct AccountPassword {
    #[rule(max_len = 1024)]
    pub old_password: String,
    #[rule(required, max_len = 1024)]
    pub new_password: String,
    #[rule(required, max_len = 1024)]
    pub confirm_password: String,
}

/// Changes the password, or sets one for a user without one. Other sessions are signed out by
/// the new security stamp; this one is renewed.
#[action(public, ai = false)]
pub async fn account_change_password(
    input: Valid<AccountPassword>,
    accounts: Accounts,
) -> Result<()> {
    let mut user = me(&accounts).await?;
    accounts.sign_in().throttle_ip().await?;
    if input.new_password != input.confirm_password {
        return Err(invalid("confirm_password", "password_confirm_mismatch"));
    }
    if user.password_hash.is_some() {
        accounts
            .users()
            .change_password(&mut user, &input.old_password, &input.new_password)
            .await?;
    } else {
        accounts
            .users()
            .add_password(&mut user, &input.new_password)
            .await?;
    }
    accounts
        .sign_in()
        .refresh_sign_in(&user)
        .await
}

#[screen(route = "/account/manage/password", public)]
pub async fn account_password(_: (), accounts: Accounts) -> Result<Screen> {
    let user = me(&accounts).await?;
    let mut form = Form::of(AccountPassword::default());
    if user.password_hash.is_some() {
        form = form
            .field(AccountPassword::old_password)
            .control(AccountPassword::old_password, Control::Password);
    }
    Ok(page("manage.password").body(
        Stack::new()
            .add(manage_nav("/account/manage/password"))
            .add(
                form.field(AccountPassword::new_password)
                    .control(AccountPassword::new_password, Control::NewPassword)
                    .field(AccountPassword::confirm_password)
                    .control(AccountPassword::confirm_password, Control::NewPassword)
                    .submit(t!("zeyn.accounts.password.submit"), account_change_password)
                    .on_success(|_, ui| {
                        ui.toast(t!("zeyn.accounts.password.changed"));
                        ui.effect(Effect::go(&path("/account/manage/password"), false));
                    }),
            ),
    ))
}

#[derive(Serialize, Deserialize)]
struct FreshCodes {
    #[serde(rename = "u")]
    user: String,
    #[serde(rename = "c")]
    codes: Vec<String>,
    #[serde(rename = "e")]
    expires: i64,
}

/// Makes new recovery codes and hands them to the screen that shows them once.
async fn new_codes(accounts: &Accounts, user: &User) -> Result<Next> {
    let codes = accounts
        .users()
        .generate_recovery_codes(user, 10)
        .await?;
    let sealer = accounts
        .0
        .sealer
        .as_ref()
        .ok_or_else(|| Error::Internal("accounts key too short".into()))?;
    let fresh = FreshCodes {
        user: user.id.clone(),
        codes,
        expires: super::now_secs() + 300,
    };
    scope::set_cookie(CODES, Some(&sealer.seal(CODES, &fresh)), Some(300));
    go(path("/account/manage/recovery-codes"), false)
}

/// Turns on two factor sign in with the authenticator app code shown on the screen.
#[action(public, reauth, ai = false)]
pub async fn account_enable_authenticator(
    input: Valid<AccountCode>,
    accounts: Accounts,
) -> Result<Next> {
    let mut user = me(&accounts).await?;
    accounts.sign_in().throttle_ip().await?;
    if !accounts
        .users()
        .verify_authenticator_code(&user, &input.code)
        .await?
    {
        return Err(invalid("code", "invalid_code"));
    }
    accounts
        .users()
        .set_two_factor_enabled(&mut user, true)
        .await?;
    accounts
        .sign_in()
        .refresh_sign_in(&user)
        .await?;
    if accounts
        .users()
        .count_recovery_codes(&user)
        .await?
        == 0
    {
        return new_codes(&accounts, &user).await;
    }
    go(path("/account/manage/two-factor"), false)
}

/// Turns two factor sign in off.
#[action(public, reauth, ai = false)]
pub async fn account_disable_two_factor(_input: (), accounts: Accounts) -> Result<Next> {
    let mut user = me(&accounts).await?;
    accounts
        .users()
        .set_two_factor_enabled(&mut user, false)
        .await?;
    accounts
        .sign_in()
        .refresh_sign_in(&user)
        .await?;
    go(path("/account/manage/two-factor"), false)
}

/// A new authenticator key: two factor is off until the app proves the new key.
#[action(public, reauth, ai = false)]
pub async fn account_reset_authenticator(_input: (), accounts: Accounts) -> Result<Next> {
    let mut user = me(&accounts).await?;
    accounts
        .users()
        .set_two_factor_enabled(&mut user, false)
        .await?;
    accounts
        .users()
        .reset_authenticator_key(&mut user)
        .await?;
    accounts
        .sign_in()
        .refresh_sign_in(&user)
        .await?;
    go(path("/account/manage/two-factor"), false)
}

/// Forgets this browser, so the next sign in asks for the second factor again.
#[action(public, ai = false)]
pub async fn account_forget_browser(_input: (), accounts: Accounts) -> Result<()> {
    me(&accounts).await?;
    accounts
        .sign_in()
        .forget_two_factor_client();
    Ok(())
}

/// Replaces the recovery codes.
#[action(public, reauth, ai = false)]
pub async fn account_new_recovery_codes(_input: (), accounts: Accounts) -> Result<Next> {
    let user = me(&accounts).await?;
    new_codes(&accounts, &user).await
}

#[screen(route = "/account/manage/two-factor", public)]
pub async fn account_two_factor(_: (), accounts: Accounts, ctx: Ctx) -> Result<Screen> {
    let mut user = me(&accounts).await?;
    let users = accounts.users();
    let mut body = Stack::new().add(manage_nav("/account/manage/two-factor"));
    if user.two_factor_enabled {
        let left = users
            .count_recovery_codes(&user)
            .await?;
        body = body
            .add(Alert::success(t!("zeyn.accounts.two_factor.on")))
            .add(markup(move |r| {
                format!(
                    "<p>{}</p>",
                    r.text(
                        &Text::key("zeyn.accounts.two_factor.codes_left")
                            .arg("n", left.to_string())
                    )
                )
            }))
            .add(
                Row::new()
                    .col(
                        ActionButton::new(
                            t!("zeyn.accounts.two_factor.new_codes"),
                            account_new_recovery_codes,
                            (),
                        )
                        .confirm(t!("zeyn.accounts.two_factor.new_codes_confirm"))
                        .on_success(next_effect),
                    )
                    .col(
                        ActionButton::new(
                            t!("zeyn.accounts.two_factor.forget"),
                            account_forget_browser,
                            (),
                        )
                        .on_success(|_, ui| ui.toast(t!("zeyn.accounts.saved"))),
                    )
                    .col(
                        ActionButton::new(
                            t!("zeyn.accounts.two_factor.disable"),
                            account_disable_two_factor,
                            (),
                        )
                        .confirm(t!("zeyn.accounts.two_factor.disable_confirm"))
                        .tone(Tone::Danger)
                        .on_success(next_effect),
                    )
                    .col(
                        ActionButton::new(
                            t!("zeyn.accounts.two_factor.reset_key"),
                            account_reset_authenticator,
                            (),
                        )
                        .confirm(t!("zeyn.accounts.two_factor.reset_key_confirm"))
                        .on_success(next_effect),
                    ),
            );
    } else if !ctx.signed_in_recently() {
        let again = path("/account/login?return_to=%2Faccount%2Fmanage%2Ftwo-factor");
        body = body
            .add(Alert::info(t!("zeyn.accounts.two_factor.reauth")))
            .add(markup(move |r| {
                format!(
                    r#"<p><a href="{}">{}</a></p>"#,
                    escape(&again),
                    tx(r, "two_factor.reauth_link")
                )
            }));
    } else {
        let key = match users.authenticator_key(&user).await? {
            Some(k) => k,
            None => {
                users
                    .reset_authenticator_key(&mut user)
                    .await?
            }
        };
        accounts
            .sign_in()
            .refresh_sign_in(&user)
            .await?;
        let uri = users.authenticator_uri(&user, &key);
        let grouped: Vec<String> = key
            .to_lowercase()
            .as_bytes()
            .chunks(4)
            .map(|c| String::from_utf8_lossy(c).into_owned())
            .collect();
        body = body
            .add(para("zeyn.accounts.two_factor.setup"))
            .add(markup(move |r| {
                format!(
                    r#"<div class="z-account-qr">{}</div><p>{} <code class="z-totp-key">{}</code></p>"#,
                    crate::auth::totp::qr_svg(&uri, &tx(r, "two_factor.qr")).unwrap_or_default(),
                    tx(r, "two_factor.manual"),
                    escape(&grouped.join(" "))
                )
            }))
            .add(
                Form::of(AccountCode::default())
                    .field(AccountCode::code)
                    .control(AccountCode::code, Control::OneTimeCode)
                    .submit(t!("zeyn.accounts.two_factor.enable"), account_enable_authenticator)
                    .on_success(next_effect),
            );
    }
    Ok(page("manage.two_factor").body(body))
}

/// Shows freshly made recovery codes once.
#[screen(route = "/account/manage/recovery-codes", public)]
pub async fn account_recovery_codes(_: (), accounts: Accounts) -> Result<Screen> {
    let user = me(&accounts).await?;
    let fresh = scope::cookie(CODES)
        .and_then(|v| {
            accounts
                .0
                .sealer
                .as_ref()?
                .open::<FreshCodes>(CODES, &v)
        })
        .filter(|f| f.user == user.id && f.expires > super::now_secs());
    scope::set_cookie(CODES, None, None);
    let mut body = Stack::new().add(manage_nav("/account/manage/two-factor"));
    match fresh {
        Some(f) => {
            body = body
                .add(Alert::warning(t!("zeyn.accounts.codes.keep")))
                .add(markup(move |_| {
                    let items: String = f
                        .codes
                        .iter()
                        .map(|c| format!("<li><code>{}</code></li>", escape(c)))
                        .collect();
                    format!(r#"<ul class="z-totp-codes">{items}</ul>"#)
                }));
        }
        None => body = body.add(Alert::info(t!("zeyn.accounts.codes.gone"))),
    }
    Ok(page("codes.title").body(body.add(link("codes.back", "/account/manage/two-factor"))))
}

/// Removes a passkey by its credential id (base64url).
#[action(public, reauth, ai = false)]
pub async fn account_remove_passkey(id: String, accounts: Accounts) -> Result<Next> {
    let mut user = me(&accounts).await?;
    let raw = crate::auth::b64url_decode(&id).ok_or(Error::NotFound)?;
    accounts
        .users()
        .remove_passkey(&mut user, &raw)
        .await?;
    accounts
        .sign_in()
        .refresh_sign_in(&user)
        .await?;
    go(path("/account/manage/passkeys"), false)
}

#[screen(route = "/account/manage/passkeys", public)]
pub async fn account_passkeys(_: (), accounts: Accounts) -> Result<Screen> {
    let user = me(&accounts).await?;
    let list = accounts.users().passkeys(&user).await?;
    let mut body = Stack::new()
        .add(manage_nav("/account/manage/passkeys"))
        .add(para("zeyn.accounts.passkeys.intro"));
    if list.is_empty() {
        body = body.add(EmptyState::new(t!("zeyn.accounts.passkeys.none")));
    }
    for p in list {
        let name = p
            .name
            .clone()
            .unwrap_or_else(|| "Passkey".into());
        let created = crate::time::DateTime::from_unix_ms(p.created_at.saturating_mul(1000));
        body = body.add(
            Row::new()
                .col(markup(move |r| {
                    format!(
                        "<p><strong>{}</strong> {}</p>",
                        escape(&name),
                        r.value(
                            &crate::value::Value::DateTime(created),
                            &crate::value::FieldKind::DateTime
                        )
                    )
                }))
                .col(
                    ActionButton::new(
                        t!("zeyn.accounts.passkeys.remove"),
                        account_remove_passkey,
                        crate::auth::b64url(&p.credential_id),
                    )
                    .confirm(t!("zeyn.accounts.passkeys.remove_confirm"))
                    .tone(Tone::Danger)
                    .on_success(next_effect),
                ),
        );
    }
    if accounts.0.public_url.is_some() {
        body = body.add(markup(|r| {
            format!(
                r#"<div class="z-field"><label for="z-passkey-name">{}</label><input id="z-passkey-name" type="text" maxlength="100" data-zeyn-passkey-name></div><p><button type="button" class="z-button z-primary" data-zeyn-passkey="add" data-failed="{}" data-unsupported="{}">{}</button></p><p class="z-field-error" data-zeyn-passkey-error aria-live="polite"></p>"#,
                tx(r, "passkeys.name"),
                tx(r, "passkeys.failed"),
                tx(r, "passkeys.unsupported"),
                tx(r, "passkeys.add")
            )
        }));
    }
    Ok(page("manage.passkeys").body(body))
}

/// An external login of the signed in user.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LoginRef {
    pub provider: String,
    pub key: String,
}

/// Unlinks an external login, unless it is the last way to sign in.
#[action(public, reauth, ai = false)]
pub async fn account_remove_login(login: LoginRef, accounts: Accounts) -> Result<Next> {
    let mut user = me(&accounts).await?;
    accounts
        .users()
        .remove_login(&mut user, &login.provider, &login.key)
        .await?;
    accounts
        .sign_in()
        .refresh_sign_in(&user)
        .await?;
    go(path("/account/manage/logins"), false)
}

#[screen(route = "/account/manage/logins", public)]
pub async fn account_logins(_: (), accounts: Accounts) -> Result<Screen> {
    let user = me(&accounts).await?;
    let logins = accounts.users().logins(&user).await?;
    let mut body = Stack::new().add(manage_nav("/account/manage/logins"));
    if logins.is_empty() {
        body = body.add(EmptyState::new(t!("zeyn.accounts.logins.none")));
    }
    let linked: Vec<String> = logins
        .iter()
        .map(|l| l.provider.clone())
        .collect();
    for l in logins {
        let label = format!(
            "{} {}",
            l.provider,
            l.display_name
                .clone()
                .unwrap_or_default()
        );
        body = body.add(
            Row::new()
                .col(markup(move |_| format!("<p>{}</p>", escape(&label))))
                .col(
                    ActionButton::new(
                        t!("zeyn.accounts.logins.remove"),
                        account_remove_login,
                        LoginRef {
                            provider: l.provider,
                            key: l.provider_key,
                        },
                    )
                    .confirm(t!("zeyn.accounts.logins.remove_confirm"))
                    .on_success(next_effect),
                ),
        );
    }
    let available: Vec<String> = accounts
        .external_names()
        .into_iter()
        .filter(|p| !linked.contains(p))
        .collect();
    if !available.is_empty() {
        let prefix = scope::prefix();
        body = body.add(markup(move |r| {
            let items: String = available
                .iter()
                .map(|p| format!(r#"<li><a class="z-button" href="{0}/_zeyn/account/external/{1}?link=1&amp;return_to={2}">{3} {1}</a></li>"#, escape(&prefix), escape(p), super::pct(&format!("{prefix}/account/manage/logins")), tx(r, "logins.add")))
                .collect();
            format!(r#"<ul class="z-account-providers">{items}</ul>"#)
        }));
    }
    Ok(page("manage.logins").body(body))
}

/// Confirms deleting the account.
#[model]
pub struct AccountDelete {
    #[rule(max_len = 1024)]
    pub password: String,
}

/// Deletes the account and its data, after the password when the user has one.
#[action(public, ai = false)]
pub async fn account_delete(input: Valid<AccountDelete>, accounts: Accounts) -> Result<Next> {
    let mut user = me(&accounts).await?;
    accounts.sign_in().throttle_ip().await?;
    if user.password_hash.is_some()
        && !accounts
            .users()
            .check_password(&mut user, &input.password)
            .await?
    {
        return Err(invalid("password", "password_mismatch"));
    }
    accounts.users().delete(&user).await?;
    accounts
        .sign_in()
        .sign_out(Some(&user.id));
    go(
        with_params("/account/login", json!({ "notice": "deleted" })),
        true,
    )
}

#[screen(route = "/account/manage/personal-data", public)]
pub async fn account_personal_data(_: (), accounts: Accounts) -> Result<Screen> {
    let user = me(&accounts).await?;
    let download = path("/_zeyn/account/manage/personalData");
    let mut form = Form::of(AccountDelete::default());
    if user.password_hash.is_some() {
        form = form
            .field(AccountDelete::password)
            .control(AccountDelete::password, Control::Password);
    }
    Ok(page("manage.personal_data").body(
        Stack::new()
            .add(manage_nav("/account/manage/personal-data"))
            .add(para("zeyn.accounts.data.intro"))
            .add(markup(move |r| {
                format!(
                    r#"<p><a class="z-button" href="{}" download>{}</a></p>"#,
                    escape(&download),
                    tx(r, "data.download")
                )
            }))
            .add(Alert::danger(t!("zeyn.accounts.data.delete_warning")))
            .add(
                form.submit(t!("zeyn.accounts.data.delete"), account_delete)
                    .on_success(next_effect),
            ),
    ))
}

/// A user as the administration lists it: no hashes, stamps or keys.
#[model]
pub struct AccountUserRow {
    #[readonly]
    pub id: String,
    pub user_name: String,
    pub email: String,
    pub email_confirmed: bool,
    pub two_factor: bool,
    pub locked: bool,
    pub roles: String,
}

/// Users for the administration grid.
#[action(permission = "accounts.users.read", ai = false)]
pub async fn account_admin_list(q: DataQuery, accounts: Accounts) -> Result<Page<AccountUserRow>> {
    let (offset, limit) = match q.page {
        crate::action::PageRequest::Offset { offset, limit } => (offset, limit),
        crate::action::PageRequest::After { limit, .. } => (0, limit),
    };
    let search = q
        .filters
        .iter()
        .find(|f| !f.contains.is_empty())
        .map(|f| f.contains.clone());
    let (users, total) = accounts
        .users()
        .list(search.as_deref(), offset, limit.min(crate::MAX_PAGE_SIZE))
        .await?;
    let mut rows = Vec::with_capacity(users.len());
    for u in users {
        let roles = accounts
            .users()
            .roles(&u)
            .await?
            .join(", ");
        rows.push(AccountUserRow {
            locked: u.is_locked_out(super::now_secs()),
            id: u.id,
            user_name: u.user_name,
            email: u.email.unwrap_or_default(),
            email_confirmed: u.email_confirmed,
            two_factor: u.two_factor_enabled,
            roles,
        });
    }
    Ok(Page::new(rows).total(total))
}

async fn admin_user(accounts: &Accounts, id: &str) -> Result<User> {
    accounts
        .users()
        .find_by_id(id)
        .await?
        .ok_or(Error::NotFound)
}

/// Locks a user out for a hundred years and signs out their sessions.
#[action(permission = "accounts.users.write", audit, ai = false)]
pub async fn account_admin_lock(id: String, accounts: Accounts) -> Result<()> {
    let mut user = admin_user(&accounts, &id).await?;
    user.lockout_enabled = true;
    accounts
        .users()
        .set_lockout_end(&mut user, Some(super::now_secs() + 100 * 365 * 86_400))
        .await?;
    accounts
        .users()
        .update_security_stamp(&mut user)
        .await
}

/// Ends a lockout.
#[action(permission = "accounts.users.write", audit, ai = false)]
pub async fn account_admin_unlock(id: String, accounts: Accounts) -> Result<()> {
    let mut user = admin_user(&accounts, &id).await?;
    user.lockout_enabled = true;
    accounts
        .users()
        .set_lockout_end(&mut user, None)
        .await
}

/// Sends the user a password reset link.
#[action(permission = "accounts.users.write", audit, ai = false)]
pub async fn account_admin_reset(id: String, accounts: Accounts, ctx: Ctx) -> Result<()> {
    let user = admin_user(&accounts, &id).await?;
    let to = user
        .email
        .clone()
        .ok_or_else(|| invalid("email", "invalid_email"))?;
    let code = accounts
        .users()
        .generate_password_reset_token(&user);
    let link = crate::server::accounts::screen_link(
        &accounts,
        "/account/reset-password",
        &json!({ "user": user.id, "code": code }),
    );
    accounts
        .0
        .sender
        .send(&ctx, &to, super::AccountMessage::ResetPassword { link })
        .await
}

#[screen(route = "/account/admin/users", permission = "accounts.users.read")]
pub fn account_admin_users(_: ()) -> Screen {
    page("admin.users").body(
        DataGrid::of(account_admin_list)
            .mode(DataMode::Server)
            .column(AccountUserRow::user_name)
            .filter()
            .column(AccountUserRow::email)
            .column(AccountUserRow::email_confirmed)
            .column(AccountUserRow::two_factor)
            .column(AccountUserRow::locked)
            .column(AccountUserRow::roles)
            .row_action(
                t!("zeyn.accounts.admin.lock"),
                account_admin_lock,
                |u: &AccountUserRow| u.id.clone(),
            )
            .row_action(
                t!("zeyn.accounts.admin.unlock"),
                account_admin_unlock,
                |u: &AccountUserRow| u.id.clone(),
            )
            .row_action(
                t!("zeyn.accounts.admin.reset"),
                account_admin_reset,
                |u: &AccountUserRow| u.id.clone(),
            )
            .on_open(|u| Window::open(account_admin_user_roles, UserParams { id: u.id.clone() })),
    )
}

/// The user an administration window edits.
#[params]
pub struct UserParams {
    pub id: String,
}

/// A user's roles, comma separated.
#[model]
pub struct AccountUserRoles {
    #[readonly]
    pub id: String,
    #[rule(max_len = 2000)]
    pub roles: String,
}

/// Sets a user's roles to the listed ones (which must exist).
#[action(permission = "accounts.users.write", audit, ai = false)]
pub async fn account_admin_save_roles(
    input: Valid<AccountUserRoles>,
    accounts: Accounts,
) -> Result<()> {
    let mut user = admin_user(&accounts, &input.id).await?;
    let wanted: Vec<String> = input
        .roles
        .split(',')
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(str::to_string)
        .collect();
    for name in &wanted {
        if !accounts.roles().exists(name).await? {
            return Err(invalid("roles", "unknown_role"));
        }
    }
    let current = accounts.users().roles(&user).await?;
    for r in &current {
        if !wanted
            .iter()
            .any(|w| super::normalize(w) == super::normalize(r))
        {
            accounts
                .users()
                .remove_from_role(&mut user, r)
                .await?;
        }
    }
    for w in &wanted {
        if !current
            .iter()
            .any(|r| super::normalize(w) == super::normalize(r))
        {
            accounts
                .users()
                .add_to_role(&mut user, w)
                .await?;
        }
    }
    Ok(())
}

#[screen(permission = "accounts.users.write")]
pub async fn account_admin_user_roles(p: UserParams, accounts: Accounts) -> Result<Screen> {
    let user = admin_user(&accounts, &p.id).await?;
    let roles = accounts
        .users()
        .roles(&user)
        .await?
        .join(", ");
    Ok(
        Screen::window(Text::Literal(user.user_name.clone().into())).body(
            Form::of(AccountUserRoles { id: user.id, roles })
                .field(AccountUserRoles::roles)
                .submit(t!("zeyn.accounts.save"), account_admin_save_roles)
                .on_success(|_, ui| ui.close_and_refresh()),
        ),
    )
}

/// A role in the administration.
#[model]
pub struct AccountRoleRow {
    #[readonly]
    pub id: String,
    #[rule(required, max_len = 254)]
    pub name: String,
    pub permissions: String,
}

/// Roles for the administration grid.
#[action(permission = "accounts.roles.write", ai = false)]
pub async fn account_admin_role_list(
    q: DataQuery,
    accounts: Accounts,
) -> Result<Page<AccountRoleRow>> {
    let (offset, limit) = match q.page {
        crate::action::PageRequest::Offset { offset, limit } => (offset, limit),
        crate::action::PageRequest::After { limit, .. } => (0, limit),
    };
    let (roles, total) = accounts
        .roles()
        .list(offset, limit)
        .await?;
    let mut rows = Vec::with_capacity(roles.len());
    for r in roles {
        let claims = accounts.roles().claims(&r).await?;
        let permissions = claims
            .into_iter()
            .filter(|c| c.kind == "permission")
            .map(|c| c.value)
            .collect::<Vec<_>>()
            .join(", ");
        rows.push(AccountRoleRow {
            id: r.id,
            name: r.name,
            permissions,
        });
    }
    Ok(Page::new(rows).total(total))
}

/// Creates a role with permission claims (comma separated).
#[action(permission = "accounts.roles.write", audit, ai = false)]
pub async fn account_admin_create_role(
    input: Valid<AccountRoleRow>,
    accounts: Accounts,
) -> Result<()> {
    let role = accounts
        .roles()
        .create(&input.name)
        .await?;
    for p in input
        .permissions
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        accounts
            .roles()
            .add_claim(&role, super::Claim::new("permission", p))
            .await?;
    }
    Ok(())
}

/// Deletes a role; its users lose it.
#[action(permission = "accounts.roles.write", audit, ai = false)]
pub async fn account_admin_delete_role(id: String, accounts: Accounts) -> Result<()> {
    let role = accounts
        .roles()
        .find_by_id(&id)
        .await?
        .ok_or(Error::NotFound)?;
    accounts.roles().delete(&role).await
}

#[screen(route = "/account/admin/roles", permission = "accounts.roles.write")]
pub fn account_admin_roles(_: ()) -> Screen {
    page("admin.roles").body(
        Stack::new()
            .add(
                DataGrid::of(account_admin_role_list)
                    .mode(DataMode::Server)
                    .column(AccountRoleRow::name)
                    .column(AccountRoleRow::permissions)
                    .row_action(
                        t!("zeyn.accounts.admin.delete_role"),
                        account_admin_delete_role,
                        |r: &AccountRoleRow| r.id.clone(),
                    ),
            )
            .add(
                Form::of(AccountRoleRow::default())
                    .field(AccountRoleRow::name)
                    .field(AccountRoleRow::permissions)
                    .submit(
                        t!("zeyn.accounts.admin.create_role"),
                        account_admin_create_role,
                    )
                    .on_success(|_, ui| {
                        ui.effect(Effect::go(&path("/account/admin/roles"), false))
                    }),
            ),
    )
}

/// The account for a first sign in with an external provider.
#[cfg(feature = "oidc")]
#[model]
pub struct AccountExternal {
    #[rule(required, email, max_len = 254)]
    pub email: String,
}

/// Creates an account for the pending external sign in, links the login and signs in. An address
/// that already has an account is refused: sign in there and link the provider from the manage
/// screen, so nobody takes over an account through a provider.
#[cfg(feature = "oidc")]
#[action(public, ai = false)]
pub async fn account_external_create(
    input: Valid<AccountExternal>,
    accounts: Accounts,
) -> Result<Next> {
    use crate::server::accounts::{EXTERNAL_PENDING, External};
    let pending = scope::cookie(EXTERNAL_PENDING)
        .and_then(|v| {
            accounts
                .0
                .sealer
                .as_ref()?
                .open::<External>(EXTERNAL_PENDING, &v)
        })
        .filter(|e| e.expires > super::now_secs());
    let Some(p) = pending else {
        return Err(denied("external_expired"));
    };
    if accounts
        .users()
        .find_by_name_or_email(&input.email)
        .await?
        .is_some()
    {
        return Err(invalid("email", "external_email_taken"));
    }
    let mut user = User::new(&input.email).with_email(&input.email);
    user.email_confirmed = p.verified
        && p.email
            .as_deref()
            .is_some_and(|e| super::normalize(e) == super::normalize(&input.email));
    let mut user = accounts
        .users()
        .create(user, None)
        .await?;
    accounts
        .users()
        .add_login(
            &mut user,
            super::UserLogin {
                provider: p.provider.clone(),
                provider_key: p.key,
                display_name: p.email.clone(),
            },
        )
        .await?;
    scope::set_cookie(EXTERNAL_PENDING, None, None);
    accounts.event(super::AccountEventKind::Registered, Some(&user.id));
    if !accounts.sign_in().can_sign_in(&user) {
        return go(path("/account/check-email"), false);
    }
    accounts
        .sign_in()
        .sign_in(&user, false, &p.provider, false)
        .await?;
    go(back(&p.return_to), true)
}

#[cfg(feature = "oidc")]
#[screen(route = "/account/external-login", public)]
pub fn account_external_login(_: (), accounts: Accounts) -> Screen {
    use crate::server::accounts::{EXTERNAL_PENDING, External};
    let pending = scope::cookie(EXTERNAL_PENDING)
        .and_then(|v| {
            accounts
                .0
                .sealer
                .as_ref()?
                .open::<External>(EXTERNAL_PENDING, &v)
        })
        .filter(|e| e.expires > super::now_secs());
    let Some(p) = pending else {
        return page("external.title").body(
            Stack::new()
                .add(Alert::warning(t!("zeyn.accounts.error.external_expired")))
                .add(link("register.login", "/account/login")),
        );
    };
    let provider = p.provider.clone();
    page("external.title").body(
        Stack::new()
            .add(markup(move |r| {
                format!(
                    "<p>{} <strong>{}</strong></p>",
                    tx(r, "external.intro"),
                    escape(&provider)
                )
            }))
            .add(
                Form::of(AccountExternal {
                    email: p.email.unwrap_or_default(),
                })
                .field(AccountExternal::email)
                .submit(t!("zeyn.accounts.external.submit"), account_external_create)
                .on_success(next_effect),
            ),
    )
}
