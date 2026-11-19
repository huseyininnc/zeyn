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
use std::sync::{Arc, Mutex, OnceLock};

/// A browser module: `/_zeyn/{name}.js` when `script` has markers, `/_zeyn/{name}.css` when
/// `style` has markers.
pub(crate) struct Builtin {
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
const fn control(name: &'static str, script: &'static [&'static str]) -> Builtin {
    Builtin {
        name,
        script,
        style: &[],
        needs: &["components"],
    }
}

/// A stylesheet module without a script.
const fn style(name: &'static str, style: &'static [&'static str]) -> Builtin {
    Builtin {
        name,
        script: &[],
        style,
        needs: &[],
    }
}

/// A script module that imports nothing.
const fn script(name: &'static str, script: &'static [&'static str]) -> Builtin {
    Builtin {
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
pub(crate) const MODULES: &[Builtin] = &[
    Builtin {
        name: "grid",
        script: &["data-zeyn-grid"],
        style: &[],
        needs: &["drag"],
    },
    script("drag", &["data-zeyn-dock"]),
    script("net", &[]),
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
        &[
            "data-zeyn-toggle",
            "data-zeyn-tabs",
            "data-zeyn-split",
            "data-zeyn-defer",
        ],
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
    Builtin {
        name: "treedrag",
        script: &["data-zeyn-tree-drag", "data-zeyn-tree-move"],
        style: &["z-tree-movable"],
        needs: &["components"],
    },
    control("overlays", &["data-zeyn-tip", "z-popover-button"]),
    control("menubutton", &["z-menu-trigger", "z-split-button"]),
    script("busy", &["data-zeyn-busy"]),
    script(
        "menu",
        &[
            "data-zeyn-menu",
            "data-zeyn-menu-filter",
            "data-zeyn-menu-count",
        ],
    ),
    script(
        "palette",
        &[
            "data-zeyn-commands",
            "data-zeyn-palette-open",
            "data-zeyn-command-run",
        ],
    ),
    Builtin {
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
    Builtin {
        name: "forms",
        script: &["data-zeyn-submit", "data-zeyn-transition"],
        style: &["z-form-settings"],
        needs: &[],
    },
    script("import", &["data-zeyn-import"]),
    Builtin {
        name: "catalog",
        script: &["data-zeyn-cards", "data-zeyn-searchbox"],
        style: &["z-cards", "z-rail", "z-poster", "z-search"],
        needs: &["drag"],
    },
    Builtin {
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
            "z-link-external",
            "z-link-external-icon",
        ],
        needs: &[],
    },
    Builtin {
        name: "files",
        script: &["z-upload"],
        style: &["z-upload"],
        needs: &["net"],
    },
    Builtin {
        name: "charts",
        script: &["data-zeyn-chart"],
        style: &["z-chart", "z-spark", "z-kpi", "z-skeleton-kpi", "z-bullet"],
        needs: &[],
    },
    script("share", &["data-zeyn-share", "data-zeyn-copy"]),
    script("chips", &["data-zeyn-chip"]),
    script("reactions", &["data-zeyn-reactions"]),
    script("hero", &["data-zeyn-hero"]),
    script("state", &["data-zeyn-prefs-clear"]),
    script("topics", &["data-zeyn-listens", "data-zeyn-drives"]),
    Builtin {
        name: "vlist",
        script: &["data-zeyn-vlist"],
        style: &[],
        needs: &["net"],
    },
    style("diff", &["z-diff"]),
    style("code", &["z-codeview"]),
    Builtin {
        name: "codeedit",
        script: &["data-zeyn-code"],
        style: &["z-codeedit"],
        needs: &[],
    },
    style("barcode", &["z-barcode"]),
    Builtin {
        name: "inbox",
        script: &["data-zeyn-inbox", "data-zeyn-notify"],
        style: &["z-inbox", "z-notify"],
        needs: &[],
    },
    Builtin {
        name: "sortable",
        script: &["data-zeyn-sortable"],
        style: &["z-sortable"],
        needs: &["drag", "net"],
    },
    Builtin {
        name: "kanban",
        script: &["data-zeyn-kanban"],
        style: &["z-kanban"],
        needs: &["drag", "net"],
    },
    Builtin {
        name: "calendar",
        script: &["data-zeyn-calendar"],
        style: &["z-cal"],
        needs: &[],
    },
    script("range", &["data-zeyn-range"]),
    script("quantity", &["data-zeyn-quantity"]),
    script("gallery", &["data-zeyn-gallery"]),
    script("countdown", &["data-zeyn-countdown"]),
    style(
        "display",
        &[
            "z-accordion",
            "z-avatar-group",
            "z-avatar-more",
            "z-compare",
            "z-copy",
            "z-divider",
            "z-flow",
            "z-funnel",
            "z-gantt",
            "z-heat",
            "z-highlight",
            "z-imgcompare",
            "z-indicator",
            "z-keys",
            "z-meter",
            "z-org",
            "z-pagelink",
            "z-pagination",
            "z-quote",
            "z-ring",
            "z-rings",
            "z-stage",
            "z-stepper",
            "z-table",
            "z-toc",
            "z-tour",
            "z-tourstop",
        ],
    ),
    script("imgcompare", &["data-zeyn-imgcompare"]),
    script("tour", &["data-zeyn-tour"]),
    Builtin {
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
    Builtin {
        name: "thread",
        script: &["data-zeyn-thread"],
        style: &["z-thread"],
        needs: &["stream"],
    },
    Builtin {
        name: "composer",
        script: &["data-zeyn-composer"],
        style: &["z-composer"],
        needs: &[],
    },
    Builtin {
        name: "typing",
        script: &["data-zeyn-typing"],
        style: &["z-typing"],
        needs: &[],
    },
    style("stories", &["z-stories", "z-story"]),
    Builtin {
        name: "lightbox",
        script: &["data-zeyn-lightbox"],
        style: &["z-lightbox-open", "z-lightbox"],
        needs: &[],
    },
    Builtin {
        name: "feed",
        script: &["data-zeyn-feed"],
        style: &["z-feed", "z-post"],
        needs: &[],
    },
    Builtin {
        name: "document",
        script: &["data-zeyn-doc"],
        style: &["z-doc"],
        needs: &[],
    },
    Builtin {
        name: "gridchips",
        script: &["data-zeyn-filter-chips"],
        style: &["z-filter-chips"],
        needs: &[],
    },
    Builtin {
        name: "surface",
        script: &["data-zeyn-surface"],
        style: &["z-surface"],
        needs: &["drag", "net"],
    },
    #[cfg(debug_assertions)]
    script("inspect", &["data-zeyn-src"]),
];

const _: () = assert!(MODULES.len() <= 64, "Used keeps one bit per module");

/// The stylesheets of the component library in cascade order. Those of a module in [`MODULES`]
/// are linked only when the page uses them; the others style components without markers of
/// their own (layouts, display widgets, form sections, the palette) and every page links them,
/// as `zeyn.css` and `enterprise.css`.
const STYLES: [&str; 27] = [
    "components",
    "forms",
    "catalog",
    "content",
    "display",
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
    "sortable",
    "calendar",
    "commerce",
    "thread",
    "composer",
    "typing",
    "stories",
    "lightbox",
    "feed",
    "document",
    "gridchips",
    "surface",
];

/// The modules a page uses, one bit each in the order of [`MODULES`].
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub(crate) struct Used {
    script: u64,
    style: u64,
    /// The extension modules ([`Extensions`]) the page uses, one bit each in registration order.
    ext: u64,
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
                .map(|k| {
                    (
                        *k,
                        Used {
                            script,
                            ..Used::default()
                        },
                    )
                })
                .chain(m.style.iter().map(|k| {
                    (
                        *k,
                        Used {
                            style: 1 << i,
                            ..Used::default()
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
        self.ext |= other.ext;
    }

    /// Adds the modules whose markers are in `html`, the framework's and the app's extensions,
    /// in one pass over the bytes that looks up every whole name starting with `z-` or
    /// `data-zeyn-`.
    pub(crate) fn scan(&mut self, html: &str, ext: &Extensions) {
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
            if !name.starts_with("z-") && !name.starts_with("data-zeyn-") {
                continue;
            }
            if let Ok(k) = table.binary_search_by_key(&name, |(m, _)| *m) {
                self.add(table[k].1);
            } else if let Ok(k) = ext
                .markers
                .binary_search_by_key(&name, |(m, _)| *m)
            {
                self.ext |= ext.markers[k].1;
            }
        }
    }

    /// Writes the links of the component library for the page head: the stylesheets every page
    /// links ([`STYLES`]) with those of the used modules in their cascade order, then the scripts
    /// of the used modules (with the modules they import), the grid runtime as a `modulepreload`.
    pub(crate) fn head(&self, base: &str, ext: &Extensions, out: &mut String) {
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
        let used = || {
            ext.list
                .iter()
                .enumerate()
                .filter(|(i, _)| self.ext & (1 << i) != 0)
                .map(|(_, m)| m)
        };
        for m in used().filter(|m| m.css.is_some()) {
            let _ = write!(
                out,
                r#"<link rel="stylesheet" href="{base}/_zeyn/ext/{}.css{v}">"#,
                m.name
            );
        }
        for m in used().filter(|m| m.js.is_some()) {
            let _ = write!(
                out,
                r#"<script type="module" src="{base}/_zeyn/ext/{}.js{v}"></script>"#,
                m.name
            );
        }
    }
}

/// The browser runtime `zeyn.js` with the table of [`MODULES`] and the app's extension modules
/// in place of its empty `const MODULES = [];`: `[name, script selector, style selector]` per
/// module, a selector empty when the module has no script or no style. An extension module is
/// listed as `ext/{name}`, so the runtime loads it from `/_zeyn/ext/`. The grid runtime is left
/// out; `zeyn.js` loads it itself. Built once per distinct table and kept for the life of the
/// process, so its address stays the key of the compressed copy (`static_files`).
pub(crate) fn runtime(js: &'static str, ext: &[Module]) -> &'static str {
    static BUILT: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
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
    let mut table: Vec<_> = MODULES
        .iter()
        .filter(|m| m.name != "grid")
        .map(|m| (m.name.to_string(), selector(m.script), selector(m.style)))
        .collect();
    for m in ext {
        let all = selector(&m.markers);
        let pick = |on: bool| if on { all.clone() } else { String::new() };
        table.push((
            format!("ext/{}", m.name),
            pick(m.js.is_some()),
            pick(m.css.is_some()),
        ));
    }
    let table = serde_json::to_string(&table).unwrap_or_else(|_| "[]".into());
    let text = js.replacen(
        "const MODULES = [];",
        &format!("const MODULES = {table};"),
        1,
    );
    let mut built = BUILT
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(known) = built.iter().find(|k| ***k == text) {
        return known;
    }
    let kept: &'static str = Box::leak(text.into_boxed_str());
    built.push(kept);
    kept
}

/// The most extension modules one app registers.
pub const MAX_EXTENSION_MODULES: usize = 64;

const _: () = assert!(
    MAX_EXTENSION_MODULES <= 64,
    "Used keeps one bit per extension module"
);

/// Names an extension module cannot take: the framework's own modules and the files the core
/// serves under `/_zeyn/`.
const RESERVED: [&str; 8] = [
    "zeyn",
    "nav",
    "enterprise",
    "assistant",
    "inspect",
    "icons",
    "theme",
    "ext",
];

/// The browser module of a component from a crate outside the framework: its script, its
/// stylesheet, the markers its HTML carries and its texts. Register it with `App::module`,
/// usually from a [`crate::Plugin`] the crate ships.
///
/// A page links the module only when its HTML holds one of the markers, and `zeyn.js` loads it
/// when such content arrives later, as for the framework's own modules. The files are served at
/// `/_zeyn/ext/{name}.js` and `/_zeyn/ext/{name}.css` with the build in the URL, so browsers
/// keep them until the next build. Both are files, never inline, so the Content Security Policy
/// stays strict.
///
/// The naming rule, checked when the module is registered: a crate `zeyn-{name}` registers the
/// module `{name}`, its classes start with `z-{name}-` and its attributes with
/// `data-zeyn-{name}-`. The name is lower case letters, digits and single hyphens, at most 32
/// characters, and no framework module has it.
///
/// ```
/// use zeyn::{App, Module};
///
/// let app = App::new("Shop").module(
///     Module::new("rating")
///         .js("export {};")
///         .css(".z-rating-stars { color: var(--z-accent); }")
///         .marker("data-zeyn-rating-stars")
///         .marker("z-rating-stars")
///         .texts(&[("en", r#"{ "rating": { "label": "Rating" } }"#)]),
/// );
/// # let _ = app;
/// ```
#[derive(Clone, Debug)]
pub struct Module {
    name: &'static str,
    js: Option<&'static str>,
    css: Option<&'static str>,
    markers: Vec<&'static str>,
    texts: &'static [(&'static str, &'static str)],
}

impl Module {
    /// A module named `name`, the crate name without `zeyn-`.
    pub fn new(name: &'static str) -> Self {
        Module {
            name,
            js: None,
            css: None,
            markers: Vec::new(),
            texts: &[],
        }
    }

    /// The script, an ES module, usually `include_str!("../assets/{name}.js")`. It sets up
    /// what it finds when it runs and listens to `zeyn:content` for content that arrives later.
    pub fn js(mut self, source: &'static str) -> Self {
        self.js = Some(source);
        self
    }

    /// The stylesheet. Colors, spacing and radii come from the theme tokens (`var(--z-...)`),
    /// so the module follows the light and the dark theme.
    pub fn css(mut self, source: &'static str) -> Self {
        self.css = Some(source);
        self
    }

    /// A marker of the component's HTML: an attribute name `data-zeyn-{name}-...` or a class
    /// `z-{name}-...`, matched as a whole name.
    pub fn marker(mut self, marker: &'static str) -> Self {
        self.markers.push(marker);
        self
    }

    /// Message catalogs of the component, by locale (nested JSON, keys under `{name}.`). The
    /// app's own catalogs win over them, so an app can reword any text.
    pub fn texts(mut self, catalogs: &'static [(&'static str, &'static str)]) -> Self {
        self.texts = catalogs;
        self
    }

    /// The module's name.
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn catalogs(&self) -> &'static [(&'static str, &'static str)] {
        self.texts
    }

    /// Checks the naming rule and that the module clashes with neither the framework nor the
    /// modules registered before it.
    pub(crate) fn check(&self, before: &[Module]) -> Result<(), String> {
        let name = self.name;
        let fix = "The naming rule: a crate zeyn-NAME registers Module::new(\"NAME\"), its classes \
                   start with z-NAME- and its attributes with data-zeyn-NAME-.";
        let shape = !name.is_empty()
            && name.len() <= 32
            && name.starts_with(|c: char| c.is_ascii_lowercase())
            && !name.ends_with('-')
            && !name.contains("--")
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !shape {
            return Err(format!(
                "browser module `{name}`: a name is lower case letters, digits and single \
                 hyphens, starts with a letter and has at most 32 characters. {fix}"
            ));
        }
        if RESERVED.contains(&name) || MODULES.iter().any(|m| m.name == name) {
            return Err(format!(
                "browser module `{name}`: the framework has a module or file with this name; \
                 pick the name of your crate without `zeyn-`. {fix}"
            ));
        }
        if before.iter().any(|m| m.name == name) {
            return Err(format!(
                "browser module `{name}` is registered twice; register each module once, \
                 usually in the Plugin of its crate."
            ));
        }
        if before.len() >= MAX_EXTENSION_MODULES {
            return Err(format!(
                "browser module `{name}`: an app registers at most {MAX_EXTENSION_MODULES} \
                 extension modules; join the components of one crate into one module."
            ));
        }
        if self.js.is_none() && self.css.is_none() {
            return Err(format!(
                "browser module `{name}` has neither a script nor a stylesheet; add \
                 .js(include_str!(..)) or .css(include_str!(..))."
            ));
        }
        if self.markers.is_empty() {
            return Err(format!(
                "browser module `{name}` has no marker, so no page would load it; add \
                 .marker(\"data-zeyn-{name}-...\") with an attribute its HTML carries."
            ));
        }
        let attr = format!("data-zeyn-{name}-");
        let class = format!("z-{name}-");
        for marker in &self.markers {
            let rest = marker
                .strip_prefix(attr.as_str())
                .or_else(|| marker.strip_prefix(class.as_str()));
            let whole = marker.bytes().all(name_byte);
            if rest.is_none_or(str::is_empty) || !whole {
                return Err(format!(
                    "browser module `{name}`: marker `{marker}` must be an attribute \
                     `{attr}...` or a class `{class}...` of letters, digits, `-` and `_`. {fix}"
                ));
            }
            if markers()
                .binary_search_by_key(marker, |(m, _)| *m)
                .is_ok()
            {
                return Err(format!(
                    "browser module `{name}`: marker `{marker}` belongs to a framework \
                     module; pick another."
                ));
            }
            if let Some(other) = before
                .iter()
                .find(|m| m.markers.contains(marker))
            {
                return Err(format!(
                    "browser module `{name}`: marker `{marker}` is already a marker of module \
                     `{}`; pick another.",
                    other.name
                ));
            }
        }
        for (locale, json) in self.texts {
            crate::I18n::new("en")
                .catalog(locale, json)
                .map_err(|e| format!("browser module `{name}`: {e}"))?;
        }
        Ok(())
    }
}

/// The extension modules of an app ([`Module`]), with their markers sorted for a binary search
/// and the runtime that knows them.
pub(crate) struct Extensions {
    list: Vec<Module>,
    markers: Vec<(&'static str, u64)>,
    /// `zeyn.js` with the module table of the framework and of these modules.
    pub(crate) runtime: &'static str,
}

impl Extensions {
    pub(crate) fn new(list: Vec<Module>, js: &'static str) -> Self {
        let mut markers: Vec<(&'static str, u64)> = list
            .iter()
            .enumerate()
            .flat_map(|(i, m)| {
                m.markers
                    .iter()
                    .map(move |k| (*k, 1u64 << i))
            })
            .collect();
        markers.sort_unstable_by_key(|(k, _)| *k);
        let runtime = runtime(js, &list);
        Extensions {
            list,
            markers,
            runtime,
        }
    }

    /// Adds the routes of the modules' files, `/_zeyn/ext/{name}.js` and `.css`.
    pub(crate) fn routes(&self, mut router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
        for m in &self.list {
            if let Some(js) = m.js {
                router = router.route(
                    &format!("/_zeyn/ext/{}.js", m.name),
                    get(move || async move { asset(js, "text/javascript; charset=utf-8") }),
                );
            }
            if let Some(css) = m.css {
                router = router.route(
                    &format!("/_zeyn/ext/{}.css", m.name),
                    get(move || async move { asset(css, "text/css; charset=utf-8") }),
                );
            }
        }
        router
    }
}

/// Script and style files of component modules, by path; the others are served next to their
/// endpoints.
const FILES: [(&str, &str); 70] = [
    (
        "/_zeyn/display.css",
        include_str!("../../assets/display.css"),
    ),
    ("/_zeyn/drag.js", include_str!("../../assets/drag.js")),
    ("/_zeyn/net.js", include_str!("../../assets/net.js")),
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
    (
        "/_zeyn/surface.css",
        include_str!("../../assets/surface.css"),
    ),
    ("/_zeyn/surface.js", include_str!("../../assets/surface.js")),
    (
        "/_zeyn/sortable.css",
        include_str!("../../assets/sortable.css"),
    ),
    (
        "/_zeyn/sortable.js",
        include_str!("../../assets/sortable.js"),
    ),
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
    ("/_zeyn/tour.js", include_str!("../../assets/tour.js")),
    (
        "/_zeyn/imgcompare.js",
        include_str!("../../assets/imgcompare.js"),
    ),
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
        "/_zeyn/treedrag.css",
        include_str!("../../assets/treedrag.css"),
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
    ("/_zeyn/menu.js", include_str!("../../assets/menu.js")),
    ("/_zeyn/palette.js", include_str!("../../assets/palette.js")),
    ("/_zeyn/reveal.js", include_str!("../../assets/reveal.js")),
    ("/_zeyn/forms.js", include_str!("../../assets/forms.js")),
    ("/_zeyn/forms.css", include_str!("../../assets/forms.css")),
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
    (
        "/_zeyn/gridchips.js",
        include_str!("../../assets/gridchips.js"),
    ),
    (
        "/_zeyn/gridchips.css",
        include_str!("../../assets/gridchips.css"),
    ),
];

/// The source inspector of debug builds (Alt+Shift+I), never part of a release build.
#[cfg(debug_assertions)]
const INSPECT_JS: &str = include_str!("../../assets/inspect.js");

/// The stylesheet of the source inspector, which `inspect.js` links on first open.
#[cfg(debug_assertions)]
const INSPECT_CSS: &str = include_str!("../../assets/inspect.css");

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
    #[cfg(debug_assertions)]
    {
        router = router
            .route(
                "/_zeyn/inspect.js",
                get(|| async { asset(INSPECT_JS, "text/javascript; charset=utf-8") }),
            )
            .route(
                "/_zeyn/inspect.css",
                get(|| async { asset(INSPECT_CSS, "text/css; charset=utf-8") }),
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
