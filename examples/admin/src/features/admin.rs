//! Administration: company settings with a sticky save bar, users and the permissions of each
//! role.

use crate::can;
use crate::models::{Settings, User, UserId};
use crate::roles::{PERMISSIONS, Role};
use crate::store::Store;
use zeyn::prelude::*;

/// Saves the company settings.
#[action(permission = "admin.settings.write", audit)]
pub async fn save_settings(input: Valid<Settings>, store: Store, ctx: Ctx) -> Result<Settings> {
    let settings = input.into_inner();
    let before: Settings = store.get(settings.id.0).await?;
    ctx.record_changes(&before, &settings);
    store.save(settings).await
}

/// The company settings. Users who may not change them see the values only.
#[screen(route = "/settings", permission = "admin.settings.read")]
pub async fn settings_page(_: (), store: Store, ctx: Ctx) -> Result<Screen> {
    let current: Settings = store.get(1).await?;
    let screen = Screen::new(t!("settings.title")).subtitle(t!("settings.subtitle"));
    if !can(&ctx, "admin.settings.write") {
        return Ok(screen.body(
            Card::new().add(
                DescriptionList::of(&current)
                    .field(Settings::company)
                    .field(Settings::support_email)
                    .field(Settings::ship_within_days)
                    .field(Settings::low_stock)
                    .field(Settings::notify_new_orders)
                    .field(Settings::weekly_report),
            ),
        ));
    }
    Ok(screen.body(
        Form::of(current)
            .settings()
            .section(t!("settings.company"))
            .field(Settings::company)
            .tip(Settings::company, t!("Settings.company_help"))
            .field(Settings::support_email)
            .tip(Settings::support_email, t!("Settings.support_email_help"))
            .section(t!("settings.orders"))
            .field(Settings::ship_within_days)
            .tip(
                Settings::ship_within_days,
                t!("Settings.ship_within_days_help"),
            )
            .field(Settings::low_stock)
            .tip(Settings::low_stock, t!("Settings.low_stock_help"))
            .section(t!("settings.notifications"))
            .field(Settings::notify_new_orders)
            .tip(
                Settings::notify_new_orders,
                t!("Settings.notify_new_orders_help"),
            )
            .field(Settings::weekly_report)
            .tip(Settings::weekly_report, t!("Settings.weekly_report_help"))
            .submit(t!("settings.save"), save_settings)
            .on_success(|_, ui| {
                ui.toast(t!("settings.saved"));
                ui.navigate(settings_page, ());
            }),
    ))
}

/// Users for the list.
#[action(permission = "admin.users.read")]
pub async fn search_users(q: DataQuery, store: Store) -> Result<Page<User>> {
    store.find(&q).await
}

/// Creates a user (id 0) or saves one.
#[action(permission = "admin.users.write", audit)]
pub async fn save_user(input: Valid<User>, store: Store, ctx: Ctx) -> Result<User> {
    let user = input.into_inner();
    if user.id.0 != 0 {
        let before: User = store.get(user.id.0).await?;
        ctx.record_changes(&before, &user);
    }
    store.save(user).await
}

/// The users of the app and their roles.
#[screen(route = "/users", permission = "admin.users.read")]
pub fn user_list(_: (), ctx: Ctx) -> Screen {
    let write = can(&ctx, "admin.users.write");
    let mut screen = Screen::new(t!("users.title"));
    if write {
        screen = screen.toolbar(|t| {
            t.button(t!("users.new"))
                .icon(Icon::UserPlus)
                .shortcut("Alt+N")
                .opens(user_form, UserFormParams { id: None })
        });
    }
    let mut grid = DataGrid::of(search_users)
        .paging(Paging::pages(25))
        .column(User::name)
        .flex(1)
        .filter()
        .column(User::email)
        .flex(1)
        .filter()
        .column(User::role)
        .width(140)
        .filter()
        .badge(|r| match r {
            Role::Admin => Tone::Danger,
            Role::Manager => Tone::Info,
            Role::Viewer => Tone::Neutral,
        })
        .column(User::active)
        .width(100)
        .filter()
        .check_icon()
        .filter_chips();
    if write {
        grid = grid.on_open(|u| Window::open(user_form, UserFormParams { id: Some(u.id) }));
    }
    screen.body(grid)
}

/// Which user the editor changes; `None` creates one.
#[params]
pub struct UserFormParams {
    pub id: Option<UserId>,
}

/// The user editor window.
#[screen(permission = "admin.users.write")]
pub async fn user_form(p: UserFormParams, store: Store) -> Result<Screen> {
    let user = match p.id {
        Some(id) => store.get(id.0).await?,
        None => User {
            active: true,
            role: Role::Viewer,
            ..User::default()
        },
    };
    Ok(Screen::window(t!("users.user"))
        .width(560)
        .body(
            Form::of(user)
                .field(User::name)
                .field(User::email)
                .row(|r| r.field(User::role).field(User::active))
                .submit(t!("users.save"), save_user)
                .on_success(|saved, ui| {
                    if let Ok(u) = saved {
                        ui.toast(t!("users.saved", name = u.name));
                    }
                    ui.close_and_refresh();
                }),
        ))
}

/// What each role may do, one row per permission.
#[screen(route = "/roles", permission = "admin.users.read")]
pub fn roles(_: (), ctx: Ctx) -> Screen {
    let mut table = Table::new(t!("roles.caption")).head(
        std::iter::once(ctx.t(&t!("roles.permission"))).chain(
            Role::ALL
                .iter()
                .map(|r| ctx.t(&Text::key(format!("Role.{r:?}")))),
        ),
    );
    for permission in PERMISSIONS {
        table = table.row(
            std::iter::once(ctx.t(&Text::key(format!("permissions.{permission}")))).chain(
                Role::ALL
                    .iter()
                    .map(|r| match r.permissions().contains(permission) {
                        true => ctx.t(&t!("roles.granted")),
                        false => ctx.t(&t!("roles.denied")),
                    }),
            ),
        );
    }
    Screen::new(t!("roles.title"))
        .subtitle(t!("roles.subtitle"))
        .body(Card::new().add(table.row_headers()))
}

/// The administration feature and its nested menu.
pub fn feature() -> Feature {
    Feature::new("admin")
        .guard(SignedIn)
        .route_prefix("/admin")
        .screen(settings_page)
        .screen(user_list)
        .screen(user_form)
        .screen(roles)
        .api(save_settings)
        .api(save_user)
        .menu(|m| {
            m.group(t!("menu.admin"), Icon::Settings)
                .item(t!("settings.title"), settings_page)
                .icon(Icon::Sliders)
                .sub(t!("menu.access"), |m| {
                    m.item(t!("users.title"), user_list)
                        .icon(Icon::Users)
                        .item(t!("roles.title"), roles)
                        .icon(Icon::Shield)
                        .badge(t!("menu.new"))
                })
        })
}
