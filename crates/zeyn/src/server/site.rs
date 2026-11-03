//! The site shell's frame (top bar, horizontal menu, footer) and its assets. See
//! [`crate::Shell`].

use super::{AppState, asset};
use crate::ctx::Ctx;
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::site::{FooterTarget, SiteShell};
use axum::Router;
use axum::routing::get;
use std::sync::Arc;

const JS: &str = include_str!("../../assets/site.js");
const CSS: &str = include_str!("../../assets/site.css");

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; 2] = ["/_zeyn/site.js", "/_zeyn/site.css"];

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
    for (i, (_, label, items)) in st
        .menu_links(ctx, r, current, true)
        .into_iter()
        .enumerate()
    {
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
    let mut actions = String::new();
    if shell.header.sign_in {
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
                r#"<a class="z-button z-site-sign-in" href="{}" data-zeyn-reload>{}</a>"#,
                escape(href),
                r.text(&Text::key(key))
            ));
        }
    }
    if shell.header.language {
        actions.push_str(&st.locale_picker(ctx, r));
    }
    if shell.header.theme {
        actions.push_str(
            &st.look
                .picker(r, ctx.theme().unwrap_or("system")),
        );
    }
    let actions = if actions.is_empty() {
        String::new()
    } else {
        format!(r#"<div class="z-site-actions">{actions}</div>"#)
    };
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
        r#"<div class="z-app z-site" data-zeyn-shell="site"><header class="z-site-header" data-zeyn-site-header><div class="z-site-bar"><a class="z-site-brand" href="{home}">{app}</a><button type="button" class="z-site-toggle" aria-expanded="false" aria-controls="z-site-nav" data-zeyn-site-toggle hidden><span class="z-site-burger" aria-hidden="true"></span><span class="z-sr-only">{menu_label}</span></button></div><nav class="z-site-nav" id="z-site-nav" aria-label="{menu_label}"><ul class="z-site-menu">{menu}</ul>{actions}</nav></header><main class="z-main z-site-main" id="z-main">{main}</main>{footer}</div>"#,
        home = escape(&home_url(st, fi)),
        app = r.text(&st.name),
    )
}
