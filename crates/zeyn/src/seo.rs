//! Head tags for search engines and link previews, and the sitemap.
//!
//! A screen sets its description, canonical URL, preview image, JSON-LD data and robots rule with
//! the builder methods of [`crate::Screen`]; [`crate::Feature::sitemap`] lists the URLs of
//! `/sitemap.xml`. Every URL is either an `https` URL or an app path starting with one `/`.

use crate::action::{Action, ActionMeta};
use crate::ctx::{BoxFuture, Ctx};
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::time::Date;
use std::sync::Arc;

/// Most URLs in one sitemap file (the limit of the sitemap protocol). Above it `/sitemap.xml`
/// becomes a sitemap index of numbered parts.
pub const MAX_SITEMAP_URLS: u64 = 50_000;

/// Most parts of a sitemap index: 50 million URLs.
pub const MAX_SITEMAP_PARTS: u64 = 1_000;

/// URLs the sitemap action is asked for at once; the XML is streamed batch by batch.
pub const SITEMAP_BATCH: u32 = 1_000;

/// Longest URL Zeyn writes into a head tag or a sitemap.
pub const MAX_URL_LEN: usize = 2_048;

/// Most language alternates of one page ([`crate::Screen::alternate`]).
pub const MAX_ALTERNATES: usize = 64;

/// Whether a text may be the `hreflang` of an alternate: `x-default`, or a language tag of
/// letters, digits and `-` that starts with a letter, at most 35 bytes.
///
/// ```
/// assert!(zeyn::seo::valid_hreflang("zh-Hans"));
/// assert!(zeyn::seo::valid_hreflang("x-default"));
/// assert!(!zeyn::seo::valid_hreflang("tr\"><script>"));
/// ```
pub fn valid_hreflang(tag: &str) -> bool {
    tag.len() <= 35
        && tag.starts_with(|c: char| c.is_ascii_alphabetic())
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Whether search engines may index a screen, sent as `<meta name="robots">`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Robots {
    /// `index, follow`.
    Index,
    /// `noindex`: the page stays out of search results.
    NoIndex,
}

/// Whether a URL may go into a head tag or a sitemap: an `https` URL with a host, or an app path
/// that starts with one `/`. No spaces, quotes, angle brackets, backslashes or control characters,
/// and at most [`MAX_URL_LEN`] bytes.
///
/// ```
/// assert!(zeyn::seo::valid_url("https://shop.example/pricing"));
/// assert!(zeyn::seo::valid_url("/pricing"));
/// assert!(!zeyn::seo::valid_url("http://shop.example/"));
/// assert!(!zeyn::seo::valid_url("//evil.example/"));
/// assert!(!zeyn::seo::valid_url("javascript:alert(1)"));
/// ```
pub fn valid_url(url: &str) -> bool {
    let clean = url.len() <= MAX_URL_LEN
        && !url.chars().any(|c| {
            c.is_control() || c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '\\' | '`')
        });
    let shape = match url.strip_prefix("https://") {
        Some(rest) => rest
            .split(['/', '?', '#'])
            .next()
            .is_some_and(|host| !host.is_empty() && !host.contains('@')),
        None => url.starts_with('/') && !url.starts_with("//"),
    };
    clean && shape
}

/// Head tags of a screen. Boxed in the screen, so screens without them stay small.
#[derive(Clone, Debug, Default)]
pub(crate) struct Head {
    pub(crate) description: Option<Text>,
    pub(crate) canonical: Option<String>,
    pub(crate) alternates: Vec<(String, String)>,
    pub(crate) og_image: Option<String>,
    pub(crate) json_ld: Option<serde_json::Value>,
    pub(crate) robots: Option<Robots>,
}

/// Keeps a URL for a head tag, or drops it with a warning when [`valid_url`] refuses it.
pub(crate) fn checked(url: String, what: &str) -> Option<String> {
    if valid_url(&url) {
        Some(url)
    } else {
        tracing::warn!("{what} must be an https URL or an app path starting with `/`; ignored");
        None
    }
}

/// URL as written into the page: `https` URLs as they are, app paths behind the site URL and the
/// base path.
pub(crate) fn absolute(url: &str, site: &str, prefix: &str) -> String {
    if url.starts_with('/') {
        format!("{site}{prefix}{url}")
    } else {
        url.to_string()
    }
}

impl Head {
    /// Renders the tags, each marked `data-zeyn-head` so in-app navigation can replace them.
    /// JSON-LD is escaped so that no `<`, `>` or `&` reaches the HTML and `</script>` in a value
    /// cannot end the script element.
    pub(crate) fn render(&self, r: &Render, site: &str, prefix: &str) -> String {
        let mut out = String::new();
        if let Some(d) = &self.description {
            let text = r.text(d);
            out.push_str(&format!(
                r#"<meta name="description" content="{text}" data-zeyn-head>"#
            ));
            out.push_str(&format!(
                r#"<meta property="og:description" content="{text}" data-zeyn-head>"#
            ));
        }
        if let Some(url) = &self.canonical {
            let url = escape(&absolute(url, site, prefix));
            out.push_str(&format!(
                r#"<link rel="canonical" href="{url}" data-zeyn-head>"#
            ));
            out.push_str(&format!(
                r#"<meta property="og:url" content="{url}" data-zeyn-head>"#
            ));
        }
        for (lang, url) in &self.alternates {
            let url = escape(&absolute(url, site, prefix));
            out.push_str(&format!(
                r#"<link rel="alternate" hreflang="{}" href="{url}" data-zeyn-head>"#,
                escape(lang)
            ));
        }
        if let Some(url) = &self.og_image {
            let url = escape(&absolute(url, site, prefix));
            out.push_str(&format!(
                r#"<meta property="og:image" content="{url}" data-zeyn-head>"#
            ));
        }
        if let Some(robots) = self.robots {
            let v = match robots {
                Robots::Index => "index, follow",
                Robots::NoIndex => "noindex",
            };
            out.push_str(&format!(
                r#"<meta name="robots" content="{v}" data-zeyn-head>"#
            ));
        }
        if let Some(data) = &self.json_ld {
            out.push_str(r#"<script type="application/ld+json" data-zeyn-head>"#);
            out.push_str(&json_ld(data));
            out.push_str("</script>");
        }
        out
    }
}

/// JSON for a `<script type="application/ld+json">`: `<`, `>` and `&` become `<`, `>`
/// and `&`, which JSON readers turn back into the same characters.
pub fn json_ld(data: &serde_json::Value) -> String {
    let raw = data.to_string();
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        match c {
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            c => out.push(c),
        }
    }
    out
}

/// A part of the sitemap the sitemap action is asked for: up to `limit` URLs after the first
/// `offset`, in a stable order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SitemapPart {
    pub offset: u64,
    pub limit: u32,
}

impl SitemapPart {
    /// The URLs of this part from a list in memory.
    pub fn of<T: Clone>(&self, all: &[T]) -> Vec<T> {
        all.iter()
            .skip(self.offset as usize)
            .take(self.limit as usize)
            .cloned()
            .collect()
    }
}

/// One URL of the sitemap, with the day it last changed.
#[derive(Clone, Debug, PartialEq)]
pub struct SitemapUrl {
    pub url: String,
    pub lastmod: Option<Date>,
}

impl SitemapUrl {
    /// An `https` URL or an app path (written behind `App::site_url`).
    pub fn new(url: impl Into<String>) -> Self {
        SitemapUrl {
            url: url.into(),
            lastmod: None,
        }
    }

    pub fn lastmod(mut self, day: Date) -> Self {
        self.lastmod = Some(day);
        self
    }
}

type List = Arc<
    dyn Fn(Ctx, SitemapPart) -> BoxFuture<crate::action::Result<Vec<SitemapUrl>>> + Send + Sync,
>;

/// The action behind `/sitemap.xml`, registered with [`crate::Feature::sitemap`].
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::seo::{SitemapPart, SitemapUrl};
///
/// #[action(public)]
/// pub async fn site_pages(part: SitemapPart) -> Result<Vec<SitemapUrl>> {
///     let all = [SitemapUrl::new("/"), SitemapUrl::new("/pricing")];
///     Ok(part.of(&all))
/// }
///
/// let feature = Feature::new("site").sitemap(site_pages);
/// ```
#[derive(Clone)]
pub struct Sitemap {
    meta: ActionMeta,
    list: List,
}

impl Sitemap {
    pub fn of<A>(_action: A) -> Self
    where
        A: Action<Input = SitemapPart, Output = crate::action::Result<Vec<SitemapUrl>>>,
    {
        let list: List = Arc::new(|ctx, part| Box::pin(async move { A::call(ctx, part).await? }));
        Sitemap {
            meta: A::META,
            list,
        }
    }

    pub fn meta(&self) -> &ActionMeta {
        &self.meta
    }

    /// Lists one part. Access is checked by the caller.
    pub fn run(
        &self,
        ctx: Ctx,
        part: SitemapPart,
    ) -> BoxFuture<crate::action::Result<Vec<SitemapUrl>>> {
        (self.list)(ctx, part)
    }
}
