//! The language in the URL (`App::locale_paths`): the default language at `/orders`, the others
//! at `/tr/orders`, `/ar/orders`. A layer in front of the router takes the language segment off
//! the path before routing, so screens keep one route each, and tells [`AppState::ctx`] the
//! language through an internal header that clients cannot set. Requests of the runtime
//! (`/_zeyn/...`) carry no language segment; they take the language of the page that sent them
//! from `Referer`, which the `same-origin` referrer policy always sends for them.

use super::AppState;
use crate::ctx::Ctx;
use crate::i18n::Text;
use crate::render::{Render, escape};
use axum::extract::{Request, State};
use axum::http::uri::PathAndQuery;
use axum::http::{HeaderValue, Uri, header};
use axum::middleware::Next;
use axum::response::Response;
use std::sync::Arc;

/// Header the layer sets with the language of the request. Removed from every incoming request
/// first, so only the layer decides it.
pub(super) const HEADER: &str = "x-zeyn-path-locale";

/// Locales that get a path segment: every offered locale except the default one.
fn path_locales(st: &AppState) -> impl Iterator<Item = &str> {
    let default = st.i18n.default_locale();
    st.i18n
        .locales()
        .iter()
        .map(String::as_str)
        .filter(move |l| *l != default)
}

/// The language segment at the start of an app path and the path without it: `/tr/orders`
/// gives `("tr", "/orders")`, `/tr` and `/tr/` give `("tr", "/")`.
fn split<'a>(st: &'a AppState, path: &'a str) -> Option<(&'a str, &'a str)> {
    let rest = path.strip_prefix('/')?;
    let (first, tail) = rest
        .split_once('/')
        .map_or((rest, ""), |(a, _)| (a, &rest[a.len()..]));
    let locale = path_locales(st).find(|l| *l == first)?;
    Some((locale, if tail.is_empty() { "/" } else { tail }))
}

/// Takes the language segment off the path before routing and records the language.
pub(super) async fn layer(
    State(st): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    req.headers_mut().remove(HEADER);
    let path = req.uri().path().to_string();
    let found = match split(&st, &path) {
        Some((locale, rest)) => {
            let query = req
                .uri()
                .query()
                .map(|q| format!("?{q}"))
                .unwrap_or_default();
            let mut parts = req.uri().clone().into_parts();
            parts.path_and_query = PathAndQuery::try_from(format!("{rest}{query}")).ok();
            if let Ok(uri) = Uri::from_parts(parts) {
                *req.uri_mut() = uri;
            }
            Some(locale.to_string())
        }
        None if path.starts_with("/_zeyn/") => req
            .headers()
            .get(header::REFERER)
            .and_then(|v| v.to_str().ok())
            .and_then(|r| r.parse::<Uri>().ok())
            .and_then(|u| {
                let page = u
                    .path()
                    .strip_prefix(st.prefix.as_str())?;
                split(&st, page).map(|(l, _)| l.to_string())
            }),
        None => None,
    };
    if let Some(v) = found.and_then(|l| HeaderValue::from_str(&l).ok()) {
        req.headers_mut().insert(HEADER, v);
    }
    next.run(req).await
}

/// Path segment of the request's language for links the server writes: empty for the default
/// language or when the app keeps the language out of the URL, `/tr` otherwise.
pub(super) fn segment<'a>(st: &AppState, ctx: &'a Ctx) -> &'a str {
    if st.locale_paths && ctx.locale() != st.i18n.default_locale() {
        ctx.locale()
    } else {
        ""
    }
}

/// App path with the request's language segment in front: `/orders` becomes `/tr/orders`.
pub(super) fn path(st: &AppState, ctx: &Ctx, path: &str) -> String {
    match segment(st, ctx) {
        "" => path.to_string(),
        l if path == "/" => format!("/{l}/"),
        l => format!("/{l}{path}"),
    }
}

/// The language picker of an app with the language in the URL: choosing a language opens the
/// same page at that language's path (`nav.js`), without a page load.
pub(super) fn picker(st: &AppState, ctx: &Ctx, r: &Render) -> String {
    let options: String = st
        .i18n
        .locales()
        .iter()
        .map(|l| {
            let selected = if l == ctx.locale() { " selected" } else { "" };
            let name = st
                .i18n
                .lookup(l, "zeyn.locale.name")
                .unwrap_or(l);
            format!(
                r#"<option value="{0}" lang="{0}"{selected}>{1}</option>"#,
                escape(l),
                escape(name)
            )
        })
        .collect();
    format!(
        r#"<div class="z-locale"><span id="z-locale-label">{}</span><div class="z-select" data-text-none=""><select id="z-locale" aria-labelledby="z-locale-label" data-zeyn-locale-path data-zeyn-locale-default="{}">{options}</select></div></div>"#,
        r.text(&Text::key("zeyn.shell.language")),
        escape(st.i18n.default_locale())
    )
}

/// Whether a language is written right to left, for `<html dir="rtl">`.
pub(super) fn rtl(locale: &str) -> bool {
    let primary = locale
        .split(['-', '_'])
        .next()
        .unwrap_or(locale);
    matches!(
        primary,
        "ar" | "he" | "fa" | "ur" | "ps" | "sd" | "ug" | "yi" | "dv" | "ckb"
    )
}
