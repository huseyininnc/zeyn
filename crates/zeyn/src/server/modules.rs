//! Browser modules of components: a page loads only the scripts and styles of the components it
//! holds.
//!
//! Every component that needs browser code declares its module in [`MODULES`] with the markers
//! its HTML carries: an attribute name (`data-zeyn-*`) or a class (`z-*`). A full page links the
//! modules whose markers are in its HTML ([`Used::head`]); `zeyn.js` gets the same table
//! ([`runtime`]) and loads a missing module when content arrives through `nav.js`, a window, a
//! lazy panel or a grid page, so a module is imported once and only where it is used. The shared
//! part of the component library (`components.js`) loads with the first module that imports it.
//! The core (`zeyn.js`, `nav.js`, `enterprise.js`) is the only script every page runs. Section 1.1
//! of `docs/runtime-protocol.md` describes the contract.

use super::{AppState, asset};
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};

/// A browser module: `/_zeyn/{name}.js` when `script` has markers, `/_zeyn/{name}.css` when
/// `style` has markers.
pub(crate) struct Module {
    /// The file name without its extension.
    pub(crate) name: &'static str,
    /// Markers of the components that run the script: attribute names (`data-zeyn-*`) and
    /// classes (`z-*`), matched as whole names.
    pub(crate) script: &'static [&'static str],
    /// Markers of the components the stylesheet styles, matched the same way.
    pub(crate) style: &'static [&'static str],
    /// Modules the script imports, linked with it so the browser fetches them at once.
    pub(crate) needs: &'static [&'static str],
}

/// A script module that imports the shared part of the component library.
const fn control(name: &'static str, script: &'static [&'static str]) -> Module {
    Module {
        name,
        script,
        style: &[],
        needs: &["components"],
    }
}

/// A stylesheet module without a script.
const fn style(name: &'static str, style: &'static [&'static str]) -> Module {
    Module {
        name,
        script: &[],
        style,
        needs: &[],
    }
}

/// A script module that imports nothing.
const fn script(name: &'static str, script: &'static [&'static str]) -> Module {
    Module {
        name,
        script,
        style: &[],
        needs: &[],
    }
}

/// Every browser module of a component. A new component with browser code adds its module here
/// (and its file to [`FILES`]); `tests/ui_modules.rs` fails when a page links a module it does not
/// use. The grid runtime is linked as a `modulepreload`, because `zeyn.js` imports it itself to
/// hand it the shared functions.
pub(crate) const MODULES: &[Module] = &[
    script("grid", &["data-zeyn-grid"]),
    script(
        "components",
        &[
            "z-select",
            "data-zeyn-dismiss",
            "data-zeyn-base-href",
            "data-zeyn-confirm",
        ],
    ),
    control(
        "tabs",
        &["data-zeyn-toggle", "data-zeyn-tabs", "data-zeyn-split"],
    ),
    control("dates", &["data-zeyn-date", "data-zeyn-filter-date"]),
    control(
        "choices",
        &["z-listbox-field", "data-zeyn-lookup", "data-zeyn-multi"],
    ),
    control(
        "inputs",
        &[
            "data-zeyn-number",
            "data-zeyn-autosize",
            "z-mask",
            "data-zeyn-password",
        ],
    ),
    control("colors", &["data-zeyn-color"]),
    control("length", &["data-zeyn-length"]),
    control("inspector", &["data-zeyn-inspector"]),
    control("lines", &["data-zeyn-lines"]),
    control("steps", &["data-zeyn-steps"]),
    control("tree", &["data-zeyn-tree"]),
    control("treedrag", &["data-zeyn-tree-drag"]),
    control("overlays", &["data-zeyn-tip", "z-popover-button"]),
    control("menubutton", &["z-menu-trigger", "z-split-button"]),
    script("busy", &["data-zeyn-busy"]),
    script(
        "palette",
        &[
            "data-zeyn-commands",
            "data-zeyn-palette-open",
            "data-zeyn-command-run",
        ],
    ),
    Module {
        name: "pro",
        script: &[
            "data-zeyn-menubar",
            "data-zeyn-toolbox",
            "data-zeyn-status-live",
            "data-zeyn-dock",
            "z-form-section",
        ],
        style: &[],
        needs: &["palette"],
    },
    script("reveal", &["data-zeyn-reveal"]),
    script("forms", &["data-zeyn-submit", "data-zeyn-transition"]),
    script("import", &["data-zeyn-import"]),
    Module {
        name: "catalog",
        script: &["data-zeyn-cards", "data-zeyn-searchbox"],
        style: &["z-cards", "z-rail", "z-poster", "z-search"],
        needs: &[],
    },
    Module {
        name: "media",
        script: &["data-zeyn-player", "z-embed", "data-zeyn-img"],
        style: &[
            "z-embed",
            "z-player",
            "z-shelf",
            "z-image",
            "z-image-blocked",
            "z-img",
            "z-img-host",
            "z-link-external-icon",
        ],
        needs: &[],
    },
    Module {
        name: "files",
        script: &["z-upload"],
        style: &["z-upload"],
        needs: &[],
    },
    Module {
        name: "charts",
        script: &["data-zeyn-chart"],
        style: &["z-chart", "z-spark", "z-kpi", "z-bullet"],
        needs: &[],
    },
    script("share", &["data-zeyn-share", "data-zeyn-copy"]),
    script("chips", &["data-zeyn-chip"]),
    script("reactions", &["data-zeyn-reactions"]),
    script("hero", &["data-zeyn-hero"]),
    script("state", &["data-zeyn-prefs-clear"]),
    script("topics", &["data-zeyn-listens", "data-zeyn-drives"]),
    script("vlist", &["data-zeyn-vlist"]),
    style("diff", &["z-diff"]),
    style("code", &["z-codeview"]),
    Module {
        name: "codeedit",
        script: &["data-zeyn-code"],
        style: &["z-codeedit"],
        needs: &[],
    },
    style("barcode", &["z-barcode"]),
    Module {
        name: "inbox",
        script: &["data-zeyn-inbox", "data-zeyn-notify"],
        style: &["z-inbox", "z-notify"],
        needs: &[],
    },
    Module {
        name: "kanban",
        script: &["data-zeyn-kanban"],
        style: &["z-kanban"],
        needs: &[],
    },
    Module {
        name: "calendar",
        script: &["data-zeyn-calendar"],
        style: &["z-cal"],
        needs: &[],
    },
    script("range", &["data-zeyn-range"]),
    script("quantity", &["data-zeyn-quantity"]),
    script("gallery", &["data-zeyn-gallery"]),
    script("countdown", &["data-zeyn-countdown"]),
    Module {
        name: "commerce",
        script: &[],
        style: &[
            "z-price",
            "z-range",
            "z-qty",
            "z-variant",
            "z-variants",
            "z-gallery",
            "z-facet",
            "z-facets",
            "z-reviews",
            "z-stars",
            "z-countdown",
        ],
        needs: &[],
    },
    script("stream", &[]),
    Module {
        name: "thread",
        script: &["data-zeyn-thread"],
        style: &["z-thread"],
        needs: &["stream"],
    },
    Module {
        name: "composer",
        script: &["data-zeyn-composer"],
        style: &["z-composer"],
        needs: &[],
    },
    Module {
        name: "typing",
        script: &["data-zeyn-typing"],
        style: &["z-typing"],
        needs: &[],
    },
    style("stories", &["z-stories", "z-story"]),
    Module {
        name: "lightbox",
        script: &["data-zeyn-lightbox"],
        style: &["z-lightbox-open", "z-lightbox"],
        needs: &[],
    },
    Module {
        name: "feed",
        script: &["data-zeyn-feed"],
        style: &["z-feed", "z-post"],
        needs: &[],
    },
    Module {
        name: "document",
        script: &["data-zeyn-doc"],
        style: &["z-doc"],
        needs: &[],
    },
];

const _: () = assert!(MODULES.len() <= 64, "Used keeps one bit per module");

/// The stylesheets of the component library in cascade order. Those of a module in [`MODULES`]
/// are linked only when the page uses them; the others style components without markers of
/// their own (layouts, display widgets, form sections, the palette) and every page links them,
/// as `zeyn.css` and `enterprise.css`.
const STYLES: [&str; 22] = [
    "components",
    "catalog",
    "content",
    "media",
    "pro",
    "files",
    "charts",
    "diff",
    "code",
    "codeedit",
    "barcode",
    "inbox",
    "kanban",
    "calendar",
    "commerce",
    "thread",
    "composer",
    "typing",
    "stories",
    "lightbox",
    "feed",
    "document",
];

/// The modules a page uses, one bit each in the order of [`MODULES`].
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct Used {
    script: u64,
    style: u64,
}

/// Every marker with the modules it brings, sorted by marker for a binary search.
fn markers() -> &'static [(&'static str, Used)] {
    static TABLE: OnceLock<Vec<(&'static str, Used)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table: Vec<(&'static str, Used)> = Vec::new();
        for (i, m) in MODULES.iter().enumerate() {
            let script = m
                .needs
                .iter()
                .fold(1u64 << i, |bits, n| bits | 1 << index(n));
            let entries = m
                .script
                .iter()
                .map(|k| (*k, Used { script, style: 0 }))
                .chain(m.style.iter().map(|k| {
                    (
                        *k,
                        Used {
                            script: 0,
                            style: 1 << i,
                        },
                    )
                }));
            for (marker, used) in entries {
                match table
                    .iter_mut()
                    .find(|(k, _)| *k == marker)
                {
                    Some((_, u)) => u.add(used),
                    None => table.push((marker, used)),
                }
            }
        }
        table.sort_unstable_by_key(|(k, _)| *k);
        table
    })
}

/// The position of a module in [`MODULES`].
fn index(name: &str) -> usize {
    MODULES
        .iter()
        .position(|m| m.name == name)
        .unwrap_or_else(|| panic!("no browser module named {name} in MODULES"))
}

/// Whether a byte belongs to an attribute or class name.
fn name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

impl Used {
    fn add(&mut self, other: Used) {
        self.script |= other.script;
        self.style |= other.style;
    }

    /// Adds the modules whose markers are in `html`, in one pass over the bytes that looks up
    /// every whole name starting with `z-` or `data-zeyn-`.
    pub(crate) fn scan(&mut self, html: &str) {
        let table = markers();
        let b = html.as_bytes();
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            if (c != b'z' && c != b'd') || (i > 0 && name_byte(b[i - 1])) {
                i += 1;
                continue;
            }
            let start = i;
            while i < b.len() && name_byte(b[i]) {
                i += 1;
            }
            let name = &html[start..i];
            if (name.starts_with("z-") || name.starts_with("data-zeyn-"))
                && let Ok(k) = table.binary_search_by_key(&name, |(m, _)| *m)
            {
                self.add(table[k].1);
            }
        }
    }

    /// Writes the links of the component library for the page head: the stylesheets every page
    /// links ([`STYLES`]) with those of the used modules in their cascade order, then the scripts
    /// of the used modules (with the modules they import), the grid runtime as a `modulepreload`.
    pub(crate) fn head(&self, base: &str, out: &mut String) {
        let v = super::ops::asset_version();
        for name in STYLES {
            let on = match MODULES
                .iter()
                .position(|m| m.name == name)
            {
                Some(i) if !MODULES[i].style.is_empty() => self.style & (1 << i) != 0,
                _ => true,
            };
            if on {
                let _ = write!(
                    out,
                    r#"<link rel="stylesheet" href="{base}/_zeyn/{name}.css{v}">"#
                );
            }
        }
        for (i, m) in MODULES.iter().enumerate() {
            if self.script & (1 << i) == 0 {
                continue;
            }
            if m.name == "grid" {
                let _ = write!(
                    out,
                    r#"<link rel="modulepreload" href="{base}/_zeyn/grid.js{v}">"#
                );
            } else {
                let _ = write!(
                    out,
                    r#"<script type="module" src="{base}/_zeyn/{}.js{v}"></script>"#,
                    m.name
                );
            }
        }
    }
}

/// The browser runtime `zeyn.js` with the table of [`MODULES`] in place of its empty
/// `const MODULES = [];`: `[name, script selector, style selector]` per module, a selector empty
/// when the module has no script or no style. The grid runtime is left out; `zeyn.js` loads it
/// itself. Built once.
pub(crate) fn runtime(js: &'static str) -> &'static str {
    static JS: OnceLock<String> = OnceLock::new();
    JS.get_or_init(|| {
        let selector = |markers: &[&str]| {
            markers
                .iter()
                .map(|m| match m.starts_with("data-") {
                    true => format!("[{m}]"),
                    false => format!(".{m}"),
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let table: Vec<_> = MODULES
            .iter()
            .filter(|m| m.name != "grid")
            .map(|m| (m.name, selector(m.script), selector(m.style)))
            .collect();
        let table = serde_json::to_string(&table).unwrap_or_else(|_| "[]".into());
        js.replacen(
            "const MODULES = [];",
            &format!("const MODULES = {table};"),
            1,
        )
    })
}

/// Script and style files of component modules, by path; the others are served next to their
/// endpoints.
const FILES: [(&str, &str); 56] = [
    ("/_zeyn/colors.js", include_str!("../../assets/colors.js")),
    ("/_zeyn/length.js", include_str!("../../assets/length.js")),
    (
        "/_zeyn/inspector.js",
        include_str!("../../assets/inspector.js"),
    ),
    (
        "/_zeyn/commerce.css",
        include_str!("../../assets/commerce.css"),
    ),
    ("/_zeyn/typing.js", include_str!("../../assets/typing.js")),
    ("/_zeyn/typing.css", include_str!("../../assets/typing.css")),
    (
        "/_zeyn/composer.js",
        include_str!("../../assets/composer.js"),
    ),
    (
        "/_zeyn/composer.css",
        include_str!("../../assets/composer.css"),
    ),
    ("/_zeyn/stream.js", include_str!("../../assets/stream.js")),
    ("/_zeyn/thread.js", include_str!("../../assets/thread.js")),
    ("/_zeyn/thread.css", include_str!("../../assets/thread.css")),
    (
        "/_zeyn/calendar.css",
        include_str!("../../assets/calendar.css"),
    ),
    (
        "/_zeyn/calendar.js",
        include_str!("../../assets/calendar.js"),
    ),
    ("/_zeyn/kanban.css", include_str!("../../assets/kanban.css")),
    ("/_zeyn/kanban.js", include_str!("../../assets/kanban.js")),
    ("/_zeyn/inbox.css", include_str!("../../assets/inbox.css")),
    ("/_zeyn/inbox.js", include_str!("../../assets/inbox.js")),
    (
        "/_zeyn/barcode.css",
        include_str!("../../assets/barcode.css"),
    ),
    ("/_zeyn/code.css", include_str!("../../assets/code.css")),
    (
        "/_zeyn/codeedit.js",
        include_str!("../../assets/codeedit.js"),
    ),
    (
        "/_zeyn/codeedit.css",
        include_str!("../../assets/codeedit.css"),
    ),
    ("/_zeyn/diff.css", include_str!("../../assets/diff.css")),
    (
        "/_zeyn/countdown.js",
        include_str!("../../assets/countdown.js"),
    ),
    ("/_zeyn/gallery.js", include_str!("../../assets/gallery.js")),
    (
        "/_zeyn/quantity.js",
        include_str!("../../assets/quantity.js"),
    ),
    ("/_zeyn/range.js", include_str!("../../assets/range.js")),
    ("/_zeyn/topics.js", include_str!("../../assets/topics.js")),
    ("/_zeyn/vlist.js", include_str!("../../assets/vlist.js")),
    ("/_zeyn/share.js", include_str!("../../assets/share.js")),
    ("/_zeyn/chips.js", include_str!("../../assets/chips.js")),
    ("/_zeyn/preview.js", include_str!("../../assets/preview.js")),
    (
        "/_zeyn/reactions.js",
        include_str!("../../assets/reactions.js"),
    ),
    ("/_zeyn/hero.js", include_str!("../../assets/hero.js")),
    ("/_zeyn/state.js", include_str!("../../assets/state.js")),
    ("/_zeyn/tabs.js", include_str!("../../assets/tabs.js")),
    ("/_zeyn/dates.js", include_str!("../../assets/dates.js")),
    ("/_zeyn/choices.js", include_str!("../../assets/choices.js")),
    ("/_zeyn/inputs.js", include_str!("../../assets/inputs.js")),
    ("/_zeyn/lines.js", include_str!("../../assets/lines.js")),
    ("/_zeyn/steps.js", include_str!("../../assets/steps.js")),
    ("/_zeyn/tree.js", include_str!("../../assets/tree.js")),
    (
        "/_zeyn/treedrag.js",
        include_str!("../../assets/treedrag.js"),
    ),
    (
        "/_zeyn/overlays.js",
        include_str!("../../assets/overlays.js"),
    ),
    (
        "/_zeyn/menubutton.js",
        include_str!("../../assets/menubutton.js"),
    ),
    ("/_zeyn/busy.js", include_str!("../../assets/busy.js")),
    ("/_zeyn/palette.js", include_str!("../../assets/palette.js")),
    ("/_zeyn/reveal.js", include_str!("../../assets/reveal.js")),
    ("/_zeyn/forms.js", include_str!("../../assets/forms.js")),
    ("/_zeyn/import.js", include_str!("../../assets/import.js")),
    (
        "/_zeyn/stories.css",
        include_str!("../../assets/stories.css"),
    ),
    (
        "/_zeyn/lightbox.css",
        include_str!("../../assets/lightbox.css"),
    ),
    (
        "/_zeyn/lightbox.js",
        include_str!("../../assets/lightbox.js"),
    ),
    ("/_zeyn/feed.css", include_str!("../../assets/feed.css")),
    ("/_zeyn/feed.js", include_str!("../../assets/feed.js")),
    (
        "/_zeyn/document.js",
        include_str!("../../assets/document.js"),
    ),
    (
        "/_zeyn/document.css",
        include_str!("../../assets/document.css"),
    ),
];

/// Asset paths served without authentication, like the runtime itself.
pub(super) const ASSETS: [&str; FILES.len()] = {
    let mut paths = [""; FILES.len()];
    let mut i = 0;
    while i < FILES.len() {
        paths[i] = FILES[i].0;
        i += 1;
    }
    paths
};

pub(super) fn routes(mut router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    for (path, body) in FILES {
        router = router.route(
            path,
            get(move || async move {
                match path.ends_with(".css") {
                    true => asset(body, "text/css; charset=utf-8"),
                    false => asset(body, "text/javascript; charset=utf-8"),
                }
            }),
        );
    }
    router
        .route("/_zeyn/prefs", get(prefs))
        .route("/_zeyn/speculation.json", get(speculation))
}

/// The Speculation Rules of `App::prefetch`: prefetch (never prerender) links to the app's own
/// path routed screens with moderate eagerness. Empty when prefetching is off.
async fn speculation(State(st): State<Arc<AppState>>) -> Response {
    let base = st.prefix.trim_end_matches('/');
    let rules = if st.prefetch == super::Prefetch::Moderate {
        serde_json::json!({ "prefetch": [{
            "source": "document",
            "where": { "and": [
                { "href_matches": format!("{base}/*") },
                { "not": { "href_matches": format!("{base}/_zeyn/*") } },
                { "not": { "href_matches": format!("{base}/api/*") } },
                { "not": { "href_matches": "/*.*" } },
                { "not": { "selector_matches": "[data-zeyn-reload], [download], [target], [data-zeyn-no-prefetch], [data-zeyn-nav] a" } },
            ] },
            "eagerness": "moderate",
        }] })
    } else {
        serde_json::json!({})
    };
    let mut r = axum::Json(rules).into_response();
    let h = r.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/speculationrules+json"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    r
}

/// Whether the user consented to keeping preference values beyond the session, by the app's
/// hook (`ClientPrefs::allowed_when`); `false` without one. Never cached, so a public page cache
/// never shares one user's answer.
async fn prefs(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let allowed = st
        .services
        .get::<crate::prefs::ClientPrefs>()
        .and_then(|p| p.allowed)
        .is_some_and(|hook| hook(&headers));
    let mut r = axum::Json(serde_json::json!({ "preferences": allowed })).into_response();
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}
