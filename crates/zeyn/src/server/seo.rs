//! `/sitemap.xml` and its numbered parts `/sitemaps/{n}.xml`, streamed from the sitemap action
//! batch by batch.

use super::{AppState, Call, CallKind, Chunks, Failure};
use crate::ctx::Ctx;
use crate::render::escape;
use crate::seo::{
    MAX_SITEMAP_PARTS, MAX_SITEMAP_URLS, SITEMAP_BATCH, Sitemap, SitemapPart, SitemapUrl, valid_url,
};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

const URLSET: &str = r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#;
const INDEX: &str = r#"<?xml version="1.0" encoding="UTF-8"?><sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">"#;

pub(super) fn routes(router: Router<Arc<AppState>>, st: &Arc<AppState>) -> Router<Arc<AppState>> {
    if st
        .features
        .iter()
        .any(|f| f.sitemap_source().is_some())
    {
        router
            .route("/sitemap.xml", get(sitemap))
            .route("/sitemaps/{part}", get(sitemap_part))
    } else {
        router
    }
}

/// The feature with the sitemap, after its guards and the action's access.
fn source<'a>(st: &'a AppState, ctx: &Ctx) -> Result<(&'static str, &'a Sitemap), Failure> {
    let (f, s) = st
        .features
        .iter()
        .find_map(|f| f.sitemap_source().map(|s| (f, s)))
        .ok_or(Failure::App(crate::Error::NotFound))?;
    f.check_guards(ctx)?;
    if !ctx.identity().can(&s.meta().access) {
        return Err(Failure::App(crate::Error::Forbidden));
    }
    Ok((f.name(), s))
}

/// Runs the action for one part through the interceptors, keeping at most `part.limit` URLs.
async fn list(
    st: &AppState,
    ctx: &Ctx,
    feature: &'static str,
    s: &Sitemap,
    part: SitemapPart,
) -> Result<Vec<SitemapUrl>, Failure> {
    let call = Call {
        kind: CallKind::Screen,
        screen: feature,
        action: Some(*s.meta()),
        via_agent: false,
    };
    let mut urls = st
        .intercept(ctx, &call, || st.timed(s.run(ctx.clone(), part)))
        .await?;
    urls.truncate(part.limit as usize);
    Ok(urls)
}

async fn sitemap(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let (feature, s) = match source(&st, &ctx) {
        Ok(found) => found,
        Err(f) => return st.error_response(&ctx, f),
    };
    let beyond = match list(
        &st,
        &ctx,
        feature,
        s,
        SitemapPart {
            offset: MAX_SITEMAP_URLS,
            limit: 1,
        },
    )
    .await
    {
        Ok(urls) => !urls.is_empty(),
        Err(f) => return st.error_response(&ctx, f),
    };
    if beyond {
        stream(st, ctx, feature, None)
    } else {
        stream(st, ctx, feature, Some(1))
    }
}

async fn sitemap_part(
    State(st): State<Arc<AppState>>,
    Path(file): Path<String>,
    headers: HeaderMap,
) -> Response {
    let ctx = st.licensed(st.ctx(&headers)).await;
    let n = file
        .strip_suffix(".xml")
        .and_then(|n| n.parse::<u64>().ok())
        .filter(|n| (1..=MAX_SITEMAP_PARTS).contains(n));
    let (Some(n), Ok((feature, _))) = (n, source(&st, &ctx)) else {
        return st.error_response(&ctx, Failure::App(crate::Error::NotFound));
    };
    stream(st, ctx, feature, Some(n))
}

/// Streams part `n` as a `urlset`, or the index of parts when `part` is `None`.
fn stream(st: Arc<AppState>, ctx: Ctx, feature: &'static str, part: Option<u64>) -> Response {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(4);
    tokio::spawn(async move {
        let Some(s) = st
            .features
            .iter()
            .find_map(|f| f.sitemap_source())
        else {
            return;
        };
        let done = match part {
            Some(n) => urlset(&st, &ctx, feature, s, n, &tx).await,
            None => index(&st, &ctx, feature, s, &tx).await,
        };
        if done.is_err() {
            let _ = tx
                .send(Err(std::io::Error::other("sitemap failed")))
                .await;
        }
    });
    let mut r = axum::body::Body::from_stream(Chunks(rx)).into_response();
    let h = r.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    r
}

async fn send(tx: &Sender<Result<Bytes, std::io::Error>>, chunk: String) -> Result<(), ()> {
    tx.send(Ok(chunk.into()))
        .await
        .map_err(|_| ())
}

async fn urlset(
    st: &AppState,
    ctx: &Ctx,
    feature: &'static str,
    s: &Sitemap,
    n: u64,
    tx: &Sender<Result<Bytes, std::io::Error>>,
) -> Result<(), ()> {
    send(tx, URLSET.into()).await?;
    let start = (n - 1) * MAX_SITEMAP_URLS;
    let mut sent = 0;
    let mut skipped = 0usize;
    while sent < MAX_SITEMAP_URLS {
        let limit = (MAX_SITEMAP_URLS - sent).min(SITEMAP_BATCH as u64) as u32;
        let urls = list(
            st,
            ctx,
            feature,
            s,
            SitemapPart {
                offset: start + sent,
                limit,
            },
        )
        .await
        .map_err(|_| ())?;
        let got = urls.len() as u64;
        let mut chunk = String::with_capacity(urls.len() * 96);
        for u in &urls {
            match located(st, &u.url) {
                Some(loc) => {
                    chunk.push_str("<url><loc>");
                    chunk.push_str(&escape(&loc));
                    chunk.push_str("</loc>");
                    if let Some(day) = u.lastmod {
                        chunk.push_str(&format!("<lastmod>{day}</lastmod>"));
                    }
                    chunk.push_str("</url>");
                }
                None => skipped += 1,
            }
        }
        send(tx, chunk).await?;
        sent += got;
        if got < limit as u64 {
            break;
        }
    }
    if skipped > 0 {
        tracing::warn!(
            "{skipped} sitemap URLs were left out: each must be an https URL, or an app path with `App::site_url` set"
        );
    }
    send(tx, "</urlset>".into()).await
}

async fn index(
    st: &AppState,
    ctx: &Ctx,
    feature: &'static str,
    s: &Sitemap,
    tx: &Sender<Result<Bytes, std::io::Error>>,
) -> Result<(), ()> {
    send(tx, INDEX.into()).await?;
    for n in 1..=MAX_SITEMAP_PARTS {
        if n > 2 {
            let probe = SitemapPart {
                offset: (n - 1) * MAX_SITEMAP_URLS,
                limit: 1,
            };
            if list(st, ctx, feature, s, probe)
                .await
                .map_err(|_| ())?
                .is_empty()
            {
                break;
            }
        }
        let loc = format!("{}{}/sitemaps/{n}.xml", st.site_url, st.prefix);
        send(
            tx,
            format!("<sitemap><loc>{}</loc></sitemap>", escape(&loc)),
        )
        .await?;
    }
    send(tx, "</sitemapindex>".into()).await
}

/// The absolute URL of a sitemap entry, or `None` when it cannot be one.
fn located(st: &AppState, url: &str) -> Option<String> {
    if !valid_url(url) {
        return None;
    }
    if url.starts_with('/') {
        return (!st.site_url.is_empty()).then(|| format!("{}{}{url}", st.site_url, st.prefix));
    }
    Some(url.to_string())
}
