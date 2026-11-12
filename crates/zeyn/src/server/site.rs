//! The site shell's frame (top bar, horizontal menu, footer) and its assets. See
//! [`crate::Shell`].

use super::{AppState, asset};
use crate::ctx::Ctx;
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::site::{CenterOnPhone, FooterTarget, SiteHeader, SiteShell};
use axum::Router;
use axum::routing::get;
use std::sync::Arc;

const JS: &str = include_str!("../../assets/site.js");
const CSS: &str = include_str!("../../assets/site.css");
const AUTH_CSS: &str = include_str!("../../assets/auth.css");

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; 3] = ["/_zeyn/site.js", "/_zeyn/site.css", "/_zeyn/auth.css"];

/// Links placed in the head of sign in shell pages only.
pub(super) fn auth_head(base: &str) -> String {
    let v = super::ops::asset_version();
    format!(r#"<link rel="stylesheet" href="{base}/_zeyn/auth.css{v}">"#)
}

/// The whole frame of a sign in shell page: the logo and name, the screen as one card, and the
/// language and theme pickers under it; with `split`, a brand panel beside it.
pub(super) fn auth_frame(
    st: &AppState,
    ctx: &Ctx,
    r: &Render,
    shell: &SiteShell,
    theme: &str,
    app: &str,
    main: &str,
) -> String {
    let split = shell.auth == Some(crate::site::AuthLayout::Split);
    let logo = super::logo_mark(
        st.logo.as_ref(),
        &escape(&st.prefix),
        app,
        "z-logo z-auth-logo",
    );
    let aside = if split {
        format!(
            r#"<aside class="z-auth-aside" aria-hidden="true"><p class="z-auth-aside-name">{app}</p></aside>"#
        )
    } else {
        String::new()
    };
    format!(
        r#"<div class="z-app z-auth{}" data-zeyn-shell="auth"><div class="z-auth-column"><p class="z-auth-brand">{logo}<span>{app}</span></p><main class="z-main z-auth-card" id="z-main">{main}</main><footer class="z-auth-foot">{}{}</footer></div>{aside}</div>"#,
        if split { " z-auth-split" } else { "" },
        st.locale_picker(ctx, r),
        st.look.picker(r, theme)
    )
}

/// Links placed in the head of site shell pages only.
pub(super) fn head(base: &str) -> String {
    let v = super::ops::asset_version();
    format!(
        r#"<link rel="stylesheet" href="{base}/_zeyn/site.css{v}"><script type="module" src="{base}/_zeyn/site.js{v}"></script>"#
    )
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route(
            "/_zeyn/site.js",
            get(|| async { asset(JS, "text/javascript; charset=utf-8") }),
        )
        .route(
            "/_zeyn/site.css",
            get(|| async { asset(CSS, "text/css; charset=utf-8") }),
        )
        .route(
            "/_zeyn/auth.css",
            get(|| async { asset(AUTH_CSS, "text/css; charset=utf-8") }),
        )
}

/// Address of a screen the user may open, after its feature's guards; `None` otherwise.
fn screen_url(st: &AppState, ctx: &Ctx, name: &str) -> Option<String> {
    let (fi, entry) = st.screens.get(name)?;
    let f = &st.features[*fi];
    if f.check_guards(ctx).is_err() || !ctx.identity().can(&entry.meta.access) {
        return None;
    }
    f.path_of(&entry.meta)
        .map(|p| st.url(&super::locale::path(st, ctx, &p)))
}

/// Where the brand links to: the screen at `/` when there is one, else the feature's first screen
/// with a route.
fn home_url(st: &AppState, fi: usize) -> String {
    if st.features.iter().any(|f| {
        f.routes()
            .iter()
            .any(|r| r.path == "/" || r.path.is_empty())
    }) {
        return st.url("/");
    }
    st.features[fi]
        .routes()
        .first()
        .map(|r| st.url(&r.path))
        .unwrap_or_else(|| st.url("/"))
}

/// The bottom bar of phones ([`SiteShell::bottom_nav`]): links to the screens the user may open,
/// the current one marked. Empty without items.
fn bottom_nav(
    st: &AppState,
    ctx: &Ctx,
    r: &Render,
    shell: &SiteShell,
    current: Option<&str>,
) -> String {
    let links: String = shell
        .bottom
        .iter()
        .filter_map(|item| {
            let href = screen_url(st, ctx, item.screen)?;
            let here = if current == Some(item.screen) { r#" aria-current="page""# } else { "" };
            let icon = item
                .icon
                .map(|i| i.html())
                .unwrap_or_default();
            Some(format!(
                r#"<li><a href="{}"{here}>{icon}<span class="z-site-bottom-label">{}</span></a></li>"#,
                escape(&href),
                r.text(&item.label)
            ))
        })
        .collect();
    if links.is_empty() {
        return String::new();
    }
    format!(
        r#"<nav class="z-site-bottom" aria-label="{}"><ul>{links}</ul></nav>"#,
        r.text(&Text::key("zeyn.shell.site.bottom"))
    )
}

/// The ready actions a header asks for: a sign in or sign out link, the language picker and the
/// theme picker. Empty when it asks for none.
pub(super) fn header_actions(st: &AppState, header: &SiteHeader, ctx: &Ctx, r: &Render) -> String {
    let mut actions = String::new();
    if header.search {
        let keys = escape(&st.palette_keys);
        actions.push_str(&format!(
            r#"<button type="button" class="z-topbar-search" data-zeyn-palette-open aria-haspopup="dialog" aria-keyshortcuts="{keys}">{}<span>{}</span><kbd aria-hidden="true">{}</kbd></button>"#,
            crate::screen::Icon::Search.html(),
            r.text(&Text::key("zeyn.shell.search")),
            escape(
                &st.palette_keys
                    .replace("Control", "Ctrl")
            )
        ));
    }
    if header.account {
        actions.push_str(&account_menu(st, ctx, r));
        return actions;
    }
    if header.sign_in {
        let signed_in = ctx.identity().is_signed_in();
        let link = if signed_in {
            st.sign_in.logout_url.as_deref()
        } else {
            st.login_url.as_deref()
        };
        if let Some(href) = link {
            let key = if signed_in {
                "zeyn.shell.site.sign_out"
            } else {
                "zeyn.shell.site.sign_in"
            };
            actions.push_str(&format!(
                r#"<a class="z-button z-site-sign-in" href="{}"{} data-zeyn-reload>{}</a>"#,
                escape(href),
                signout_attr(st, signed_in),
                r.text(&Text::key(key))
            ));
        }
    }
    if header.language {
        actions.push_str(&st.locale_picker(ctx, r));
    }
    if header.theme {
        actions.push_str(
            &st.look
                .picker(r, ctx.theme().unwrap_or("system")),
        );
    }
    actions
}

/// The account menu of `SiteHeader::account`: the user's initials on a button that opens a
/// non-modal dialog (`overlays.js`) with who is signed in, the pickers and the sign in or out link.
fn account_menu(st: &AppState, ctx: &Ctx, r: &Render) -> String {
    let user = ctx.identity().user.as_deref();
    let initials: String = user
        .unwrap_or("")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(2)
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .collect();
    let label = match user {
        Some(name) => r.text(&Text::key("zeyn.shell.account_of").arg("name", name.to_string())),
        None => r.text(&Text::key("zeyn.shell.account")),
    };
    let tone = user.map_or(0, |u| {
        u.bytes().fold(0u32, |h, b| {
            h.wrapping_mul(31)
                .wrapping_add(u32::from(b))
        }) % 8
    });
    let face = if initials.is_empty() {
        crate::screen::Icon::User.html()
    } else {
        escape(&initials)
    };
    let mut out = format!(
        r#"<div class="z-popover-wrap z-account" data-zeyn-popover><button type="button" class="z-popover-button z-avatar-button" id="z-account-button" aria-haspopup="dialog" aria-expanded="false" aria-controls="z-account" aria-label="{label}"><span class="z-avatar-face z-face-{tone}" aria-hidden="true">{face}</span></button><div class="z-popover z-account-panel" id="z-account" role="dialog" aria-labelledby="z-account-title" tabindex="-1" hidden><div class="z-popover-head"><h2 class="z-popover-title" id="z-account-title">{}</h2><button type="button" class="z-icon-button z-popover-close" data-zeyn-popover-close aria-label="{}">{}</button></div><div class="z-popover-body">"#,
        r.text(&Text::key("zeyn.shell.account")),
        r.text(&Text::key("zeyn.ui.popover.close")),
        crate::screen::Icon::X.html()
    );
    if let Some(name) = user {
        out.push_str(&format!(
            r#"<p class="z-account-who"><span>{}</span> <strong>{}</strong></p>"#,
            r.text(&Text::key("zeyn.shell.signed_in_as")),
            escape(name)
        ));
    }
    out.push_str(
        &st.look
            .picker(r, ctx.theme().unwrap_or("system")),
    );
    out.push_str(&super::theme::density_picker(r, ctx.compact()));
    out.push_str(&st.locale_picker(ctx, r));
    let signed_in = ctx.identity().is_signed_in();
    let link = if signed_in {
        st.sign_in.logout_url.as_deref()
    } else {
        st.login_url.as_deref()
    };
    if let Some(href) = link {
        let (key, icon) = if signed_in {
            ("zeyn.shell.site.sign_out", crate::screen::Icon::SignOut)
        } else {
            ("zeyn.shell.site.sign_in", crate::screen::Icon::SignIn)
        };
        out.push_str(&format!(
            r#"<a class="z-button z-ghost z-account-sign" href="{}"{} data-zeyn-reload>{}{}</a>"#,
            escape(href),
            signout_attr(st, signed_in),
            icon.html(),
            r.text(&Text::key(key))
        ));
    }
    out.push_str("</div></div></div>");
    out
}

/// The start, center, end and bottom slots of a header, rendered for the request's user with
/// render scope `scope`, each in `<div class="{class}-slot {class}-slot-{name}">`; empty slots
/// are empty strings. Node paths are `{slot}.{index}`, as [`SiteHeader::node`] reads them.
pub(super) fn header_slots(
    header: &SiteHeader,
    ctx: &Ctx,
    scope: &str,
    class: &str,
) -> [String; 4] {
    let sr = Render::new(scope, ctx.locale(), ctx.i18n())
        .with_tz(ctx.time_zone())
        .with_identity(ctx.identity())
        .with_ctx(ctx);
    std::array::from_fn(|i| {
        if header.slots[i].is_empty() {
            return String::new();
        }
        let name = crate::site::SLOT_NAMES[i];
        let mut out =
            format!(r#"<div class="{class}-slot {class}-slot-{name}" id="{class}-slot-{name}">"#);
        for (n, node) in header.slots[i].iter().enumerate() {
            crate::ui::render_node(&mut out, &sr, &format!("{i}.{n}"), node);
        }
        out.push_str("</div>");
        out
    })
}

/// The whole frame of a site shell page: top bar with the brand, menu button, menu and actions;
/// the screen; the footer.
pub(super) fn frame(
    st: &AppState,
    ctx: &Ctx,
    r: &Render,
    shell: &SiteShell,
    fi: usize,
    main: &str,
    current: Option<&str>,
) -> String {
    let menu_label = r.text(&Text::key("zeyn.shell.menu"));
    let mut menu = String::new();
    for (i, group) in st
        .menu_links(ctx, r, current, true)
        .into_iter()
        .enumerate()
    {
        let label = group.label().to_owned();
        let items = group.all_links();
        if items.len() == 1 {
            menu.push_str(&items[0]);
            continue;
        }
        let here = if items
            .iter()
            .any(|l| l.contains(r#"aria-current="page""#))
        {
            " data-current"
        } else {
            ""
        };
        menu.push_str(&format!(
            r#"<li class="z-site-group"><button type="button" class="z-site-group-button" aria-expanded="false" aria-controls="z-site-group-{i}"{here}>{label}<span class="z-site-caret" aria-hidden="true"></span></button><ul class="z-site-sub" id="z-site-group-{i}" hidden>{}</ul></li>"#,
            items.join("")
        ));
    }
    let actions = header_actions(st, &shell.header, ctx, r);
    let actions = if actions.is_empty() {
        String::new()
    } else {
        format!(r#"<div class="z-site-actions" id="z-site-actions">{actions}</div>"#)
    };
    let header = &shell.header;
    let [start, center, end, bottom] = header_slots(header, ctx, "site", "z-site");
    let icon = !center.is_empty() && header.center_on_phone == CenterOnPhone::Icon;
    let center_button = if icon {
        let (glyph, label) = header
            .center_button
            .as_ref()
            .map(|(i, l)| (i.html(), r.text(l)))
            .unwrap_or_else(|| {
                (
                    crate::screen::Icon::Search.html(),
                    r.text(&Text::key("zeyn.shell.site.center")),
                )
            });
        format!(
            r#"<button type="button" class="z-site-toggle z-site-center-toggle" aria-expanded="false" aria-controls="z-site-slot-center" data-zeyn-site-center-toggle hidden>{glyph}<span class="z-sr-only">{label}</span></button>"#
        )
    } else {
        String::new()
    };
    let controls = if actions.is_empty() {
        "z-site-nav"
    } else {
        "z-site-nav z-site-actions"
    };
    let phone = if icon { "icon" } else { "row" };
    let mut footer = String::new();
    for (i, column) in shell.footer.columns.iter().enumerate() {
        let links: String = column
            .links
            .iter()
            .filter_map(|l| {
                let href = match l.target {
                    FooterTarget::Screen(name) => screen_url(st, ctx, name)?,
                    FooterTarget::Url(url) => url.to_string(),
                };
                Some(format!(
                    r#"<li><a href="{}">{}</a></li>"#,
                    escape(&href),
                    r.text(&l.label)
                ))
            })
            .collect();
        if links.is_empty() {
            continue;
        }
        footer.push_str(&format!(
            r#"<section class="z-site-column" aria-labelledby="z-site-footer-{i}"><h2 id="z-site-footer-{i}">{}</h2><ul>{links}</ul></section>"#,
            r.text(&column.title)
        ));
    }
    if !footer.is_empty() {
        footer = format!(r#"<div class="z-site-columns">{footer}</div>"#);
    }
    if let Some(text) = &shell.footer.text {
        footer.push_str(&format!(r#"<p class="z-site-note">{}</p>"#, r.text(text)));
    }
    if !footer.is_empty() {
        footer = format!(r#"<footer class="z-site-footer">{footer}</footer>"#);
    }
    footer.push_str(&bottom_nav(st, ctx, r, shell, current));
    format!(
        r#"<div class="z-app z-site" data-zeyn-shell="site"><header class="z-site-header" data-zeyn-site-header data-zeyn-screen="_site.{feature}" data-zeyn-params="null" data-center-phone="{phone}"><div class="z-site-bar">{start}<a class="z-site-brand" href="{home}">{app}</a>{center_button}<button type="button" class="z-site-toggle" aria-expanded="false" aria-controls="{controls}" data-zeyn-site-toggle hidden><span class="z-site-burger" aria-hidden="true"></span><span class="z-sr-only">{menu_label}</span></button></div><nav class="z-site-nav" id="z-site-nav" aria-label="{menu_label}"><ul class="z-site-menu">{menu}</ul></nav>{center}{end}{actions}{bottom}</header><main class="z-main z-site-main" id="z-main">{main}</main>{footer}</div>"#,
        feature = escape(st.features[fi].name()),
        home = escape(&home_url(st, fi)),
        app = r.text(&st.name),
    )
}

/// The `data-zeyn-signout` attribute of a sign out link: the address the runtime posts to, with
/// the CSRF token, before it follows the link. Empty for sign in links and apps without one.
fn signout_attr(st: &AppState, signed_in: bool) -> String {
    match st.sign_in.signout {
        Some(path) if signed_in => {
            format!(r#" data-zeyn-signout="{}{}""#, escape(&st.prefix), path)
        }
        _ => String::new(),
    }
}
