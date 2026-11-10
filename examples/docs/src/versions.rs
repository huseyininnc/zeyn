//! Versioned documentation. Every release runs as its own build of this app: the latest at `/`
//! and each release also under `/v<version>/` (`App::base_path`). A build learns the other
//! versions from its configuration at startup, so an old build never needs a rebuild when a new
//! version comes out.
//!
//! The header's version picker links to `<version base>/_docs/switch` in the chosen version. That
//! build reads the page the reader came from out of `Referer` (the `same-origin` referrer policy
//! always sends it for links on the site) and redirects to the same page in its own pages, or to
//! its start page when it has no such page. The header stays the same during in app navigation,
//! so links that carried the path would point to the first page opened.

use crate::{LANGS, PAGES, lang_path};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use zeyn::prelude::*;
use zeyn::render::escape;
use zeyn::{Component, Config, Render};

/// Most versions a build lists in its picker.
pub const MAX_VERSIONS: usize = 50;

/// Path of the version switch, behind a version's base path.
pub const SWITCH: &str = "/_docs/switch";

/// The version of this build, the versions of the picker (newest first), the latest version and
/// the base path this build is served under. Cheap to clone; register it with `App::service`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Versions(Arc<Inner>);

#[derive(Debug, PartialEq, Eq)]
struct Inner {
    current: String,
    all: Vec<String>,
    latest: String,
    base: String,
}

impl Service for Versions {}

/// Whether a text is a version: numbers separated by dots, such as `0.0.9`, at most 32 bytes.
///
/// ```
/// use zeyn_example_docs::versions::valid;
/// assert!(valid("0.0.10"));
/// assert!(!valid("0.0.9-beta") && !valid("1..2") && !valid("") && !valid("../x"));
/// ```
pub fn valid(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 32
        && version
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

impl Versions {
    /// Checks the settings and falls back to safe values: an invalid `current` becomes the crate
    /// version, invalid or repeated entries of `list` are dropped and at most [`MAX_VERSIONS`]
    /// are kept, `current` is always listed, `latest` must be listed (else the first entry is
    /// the latest) and `base` must be empty or `/v<version>` (else it is empty).
    ///
    /// ```
    /// use zeyn_example_docs::versions::Versions;
    /// let v = Versions::new(Some("0.0.9"), Some("0.0.10, 0.0.9, x, 0.0.9"), None, Some("/v0.0.9/"));
    /// assert_eq!(v.all(), ["0.0.10", "0.0.9"]);
    /// assert_eq!((v.latest(), v.base()), ("0.0.10", "/v0.0.9"));
    /// ```
    pub fn new(
        current: Option<&str>,
        list: Option<&str>,
        latest: Option<&str>,
        base: Option<&str>,
    ) -> Versions {
        let current = current
            .map(str::trim)
            .filter(|v| valid(v))
            .unwrap_or(env!("CARGO_PKG_VERSION"))
            .to_string();
        let mut all: Vec<String> = Vec::new();
        for v in list
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|v| valid(v))
        {
            if all.len() == MAX_VERSIONS {
                break;
            }
            if !all.iter().any(|a| a == v) {
                all.push(v.to_string());
            }
        }
        if !all.contains(&current) {
            all.truncate(MAX_VERSIONS - 1);
            all.push(current.clone());
        }
        let latest = latest
            .map(str::trim)
            .filter(|l| all.iter().any(|a| a == l))
            .unwrap_or(&all[0])
            .to_string();
        let base = base
            .map(|b| b.trim().trim_matches('/'))
            .filter(|b| b.strip_prefix('v').is_some_and(valid))
            .map(|b| format!("/{b}"))
            .unwrap_or_default();
        Versions(Arc::new(Inner {
            current,
            all,
            latest,
            base,
        }))
    }

    /// The settings `version`, `versions`, `latest` and `base_path`, from `ZEYN_DOCS_VERSION`,
    /// `ZEYN_DOCS_VERSIONS`, `ZEYN_DOCS_LATEST` and `ZEYN_DOCS_BASE_PATH`.
    pub fn from_config(config: &Config) -> Versions {
        Versions::new(
            config.get("version"),
            config.get("versions"),
            config.get("latest"),
            config.get("base_path"),
        )
    }

    /// A single version build at `/`: the crate version, nothing else listed.
    pub fn single() -> Versions {
        Versions::new(None, None, None, None)
    }

    pub fn current(&self) -> &str {
        &self.0.current
    }

    /// The versions of the picker, newest first.
    pub fn all(&self) -> &[String] {
        &self.0.all
    }

    pub fn latest(&self) -> &str {
        &self.0.latest
    }

    /// The base path of this build: empty at `/`, else `/v<version>`.
    pub fn base(&self) -> &str {
        &self.0.base
    }

    /// Whether this build is an older version than the latest.
    pub fn is_old(&self) -> bool {
        self.0.current != self.0.latest
    }

    /// Whether this build is served under a version path. Search engines index only the build at
    /// `/`, so such a build sends `noindex`, a canonical URL at `/` and an empty sitemap.
    pub fn prefixed(&self) -> bool {
        !self.0.base.is_empty()
    }

    /// Where a version is served: `/` for the latest, `/v<version>` for the others.
    pub fn base_of(&self, version: &str) -> String {
        if version == self.0.latest {
            String::new()
        } else {
            format!("/v{version}")
        }
    }

    /// The link that opens the reader's page in a version.
    pub fn switch_url(&self, version: &str) -> String {
        format!("{}{SWITCH}", self.base_of(version))
    }

    /// The path of this build to send a reader to who came from `referer`: the same page in the
    /// same language when this build has it, else the start page of that language. Only the path
    /// of `referer` is read; a version segment at its start (`/v0.0.10`) is dropped, and the
    /// answer is always built from this build's own routes and languages.
    ///
    /// ```
    /// use zeyn_example_docs::versions::Versions;
    /// let v = Versions::new(Some("0.0.9"), Some("0.0.10,0.0.9"), None, Some("/v0.0.9"));
    /// assert_eq!(v.target(Some("https://docs.example/tr/guides/security?q=1")), "/v0.0.9/tr/guides/security");
    /// assert_eq!(v.target(Some("https://docs.example/v0.0.10/tr/not-here")), "/v0.0.9/tr/");
    /// assert_eq!(v.target(None), "/v0.0.9/");
    /// ```
    pub fn target(&self, referer: Option<&str>) -> String {
        let path = referer
            .and_then(|r| r.split_once("://"))
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.find('/').map(|i| &rest[i..]))
            .map(|p| {
                p.split(['?', '#'])
                    .next()
                    .unwrap_or("/")
            })
            .unwrap_or("/");
        let path = path
            .strip_prefix("/v")
            .and_then(|rest| {
                let (version, tail) = rest
                    .split_once('/')
                    .map_or((rest, ""), |(v, _)| (v, &rest[v.len()..]));
                valid(version).then_some(tail)
            })
            .unwrap_or(path);
        let (lang, route) = LANGS[1..]
            .iter()
            .find_map(|(code, _)| {
                let rest = path
                    .strip_prefix('/')?
                    .strip_prefix(code)?;
                match rest {
                    "" | "/" => Some((*code, "/")),
                    r if r.starts_with('/') => Some((*code, r)),
                    _ => None,
                }
            })
            .unwrap_or(("en", path));
        let route = PAGES
            .iter()
            .find(|p| p.route == route.trim_end_matches('/') || p.route == route)
            .map_or("/", |p| p.route);
        format!("{}{}", self.base(), lang_path(lang, route))
    }
}

/// Answers the version switch: `302 Found` to [`Versions::target`], never cached.
pub fn switch(versions: &Versions, headers: &HeaderMap) -> Response {
    let referer = headers
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok());
    let location = versions.target(referer);
    let mut res = StatusCode::FOUND.into_response();
    let h = res.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&location) {
        h.insert(header::LOCATION, v);
    }
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

/// The links of the version picker: every version, the current one marked with
/// `aria-current`, the latest one named so. The links load a whole page (`data-zeyn-reload`),
/// since each version is another app.
pub struct VersionLinks(pub Versions);

impl Component for VersionLinks {
    fn render(&self, out: &mut String, r: &Render) {
        let v = &self.0;
        out.push_str(r#"<ul class="z-doc-versions">"#);
        for version in v.all() {
            let current = if version == v.current() {
                r#" aria-current="true""#
            } else {
                ""
            };
            let latest = if version == v.latest() {
                format!(" ({})", r.text(&t!("docs.version.latest")))
            } else {
                String::new()
            };
            out.push_str(&format!(
                r#"<li><a href="{}" data-zeyn-reload{current}>{}{latest}</a></li>"#,
                escape(&v.switch_url(version)),
                escape(version)
            ));
        }
        out.push_str("</ul>");
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "doc_versions", "current": self.0.current(), "versions": self.0.all() })
    }
}

/// The version picker of the header: a button with this build's version that opens the list of
/// versions.
pub fn picker(versions: &Versions) -> Popover {
    Popover::new(Text::from(format!("v{}", versions.current())))
        .title(t!("docs.version.title"))
        .add(Node::new(VersionLinks(versions.clone())))
}

/// The note on every page of an older version, with a link to the same page in the latest.
pub fn notice(versions: &Versions, r: &Render) -> String {
    if !versions.is_old() {
        return String::new();
    }
    format!(
        r#"<p class="z-doc-note" role="note">{} <a href="{}" data-zeyn-reload>{}</a></p>"#,
        r.text(&t!(
            "docs.version.notice",
            version = versions.current(),
            latest = versions.latest()
        )),
        escape(&versions.switch_url(versions.latest())),
        r.text(&t!("docs.version.latest_link"))
    )
}
