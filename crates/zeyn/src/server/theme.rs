//! Themes: the organization's look, the built in light and dark themes, app defined token sets,
//! the theme picker in the shell and `POST /_zeyn/theme`.
//!
//! Every theme is a set of the CSS custom properties in `docs/theming.md`. Tokens are checked
//! when the app starts (names, values and WCAG 2.2 AA contrast) and served as one stylesheet,
//! `/_zeyn/theme.css`, built once. The server renders the chosen theme on `<html>` from the
//! `zeyn_theme` cookie, so the first paint is already right and no inline script is needed.

use super::{App, AppState, Failure, cookie, with_cookie};
use crate::action::Error;
use crate::ctx::Ctx;
use crate::grid::{ColumnView, DataGridView};
use crate::i18n::Text;
use crate::render::{Render, escape};
use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde::Deserialize;
use std::sync::Arc;

/// Tokens a theme may set: the base and derived tokens of `docs/theming.md` and the chart colors.
const THEME_TOKENS: &[&str] = &[
    "--z-brand",
    "--z-on-brand",
    "--z-font",
    "--z-font-mono",
    "--z-font-size",
    "--z-font-size-sm",
    "--z-font-size-xs",
    "--z-radius",
    "--z-density",
    "--z-bg",
    "--z-surface",
    "--z-surface-2",
    "--z-hover",
    "--z-border",
    "--z-border-strong",
    "--z-text",
    "--z-text-muted",
    "--z-danger",
    "--z-warning",
    "--z-success",
    "--z-info",
    "--z-accent",
    "--z-primary",
    "--z-on-primary",
    "--z-primary-hover",
    "--z-primary-active",
    "--z-brand-subtle",
    "--z-focus",
    "--z-tint",
    "--z-backdrop",
    "--z-chart-1",
    "--z-chart-2",
    "--z-chart-3",
    "--z-chart-4",
    "--z-chart-5",
    "--z-chart-6",
];

/// Values of the kit's light theme, as in `zeyn.css`.
const LIGHT: &[(&str, &str)] = &[
    ("--z-brand", "#0f6674"),
    ("--z-on-brand", "#ffffff"),
    ("--z-bg", "#f4f5f6"),
    ("--z-surface", "#ffffff"),
    ("--z-surface-2", "#f7f8f9"),
    ("--z-hover", "#eceef1"),
    ("--z-border-strong", "#7d848d"),
    ("--z-text", "#1a1d21"),
    ("--z-text-muted", "#5b626b"),
    ("--z-danger", "#b3261e"),
    ("--z-warning", "#8a5300"),
    ("--z-success", "#1e6b3a"),
    ("--z-info", "#2b5d8a"),
];

/// Values the kit's dark theme sets on top of the light ones, as in `zeyn.css`.
const DARK: &[(&str, &str)] = &[
    ("--z-bg", "#121416"),
    ("--z-surface", "#1a1d20"),
    ("--z-surface-2", "#202326"),
    ("--z-hover", "#2a2e33"),
    ("--z-border-strong", "#7a818a"),
    ("--z-text", "#e4e6e9"),
    ("--z-text-muted", "#a2a9b1"),
    ("--z-danger", "#f28b82"),
    ("--z-warning", "#e6b45a"),
    ("--z-success", "#6fcf8f"),
    ("--z-info", "#8ab8e6"),
];

/// Color pairs that must reach WCAG 2.2 AA: foreground, background, needed ratio.
const PAIRS: &[(&str, &str, f64)] = &[
    ("--z-text", "--z-bg", 4.5),
    ("--z-text", "--z-surface", 4.5),
    ("--z-text", "--z-surface-2", 4.5),
    ("--z-text", "--z-hover", 4.5),
    ("--z-text-muted", "--z-bg", 4.5),
    ("--z-text-muted", "--z-surface", 4.5),
    ("--z-text-muted", "--z-surface-2", 4.5),
    ("--z-text-muted", "--z-hover", 4.5),
    ("--z-border-strong", "--z-surface", 3.0),
    ("--z-border-strong", "--z-surface-2", 3.0),
    ("--z-danger", "--z-surface", 4.5),
    ("--z-warning", "--z-surface", 4.5),
    ("--z-success", "--z-surface", 4.5),
    ("--z-info", "--z-surface", 4.5),
    ("--z-on-brand", "--z-brand", 4.5),
];

/// A look: the organization's brand, fonts, radius and density (`Theme::new`), one of the built
/// in themes (`Theme::light`, `Theme::dark`) or an app theme (`Theme::named`). Values are checked
/// so a theme can never inject CSS.
///
/// ```ignore
/// App::new("Billing")
///     .theme(Theme::new().brand("#7a2448").radius(4))
///     .themes([Theme::light(), Theme::dark(), Theme::named("ocean", [("--z-brand", "#0b5394"), ("--z-bg", "#eef4f8")])])
///     .default_theme("system")
/// ```
#[derive(Clone, Debug, Default)]
pub struct Theme {
    name: Option<String>,
    label: Option<Text>,
    brand: Option<String>,
    font: Option<String>,
    radius: Option<u8>,
    pub(super) compact: bool,
    pub(super) scheme: Option<&'static str>,
    tokens: Vec<(String, String)>,
    problems: Vec<String>,
}

impl Theme {
    /// The organization's look, shared by every theme: brand, fonts, radius and density.
    pub fn new() -> Self {
        Theme::default()
    }

    /// The built in light theme. Given to `App::theme` it locks the app to light.
    pub fn light() -> Self {
        Theme { name: Some("light".into()), scheme: Some("light"), ..Theme::default() }
    }

    /// The built in dark theme. Given to `App::theme` it locks the app to dark.
    pub fn dark() -> Self {
        Theme { name: Some("dark".into()), scheme: Some("dark"), ..Theme::default() }
    }

    /// An app theme built on the light theme: a name (`a` to `z`, digits and `-`, up to 32) and
    /// the tokens it changes, such as `[("--z-brand", "#0b5394"), ("--z-bg", "#eef4f8")]`. Bad
    /// names, unknown tokens, unsafe values and low contrast are reported by `App::check`.
    pub fn named<'a>(name: &str, tokens: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut theme = Theme { name: Some(name.to_string()), scheme: Some("light"), ..Theme::default() };
        let valid = !name.is_empty()
            && name.len() <= 32
            && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !valid || matches!(name, "system" | "light" | "dark") {
            theme.problems.push(format!(
                "theme name `{name}` is not allowed\n  = help: use `a` to `z`, digits and `-` (up to 32), and not `system`, `light` or `dark`"
            ));
        }
        for (token, value) in tokens {
            theme = theme.token(token, value);
        }
        theme
    }

    /// Builds this app theme on the dark theme instead of the light one.
    pub fn dark_base(mut self) -> Self {
        self.scheme = Some("dark");
        self
    }

    /// Name shown in the theme picker. App themes show their name unless set.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets one token, such as `token("--z-surface", "#fbfaf7")`. Tokens are listed in `docs/theming.md`.
    pub fn token(mut self, name: &str, value: &str) -> Self {
        let owner = self.name.clone().unwrap_or_else(|| "app".into());
        if !THEME_TOKENS.contains(&name) {
            self.problems.push(format!(
                "theme `{owner}`: `{name}` is not a theme token\n  = help: use one of the tokens in docs/theming.md"
            ));
        } else if !safe_value(value) {
            self.problems.push(format!(
                "theme `{owner}`: the value of `{name}` is not allowed\n  = help: use a color, length or font list without `;`, braces, `\\`, `<`, `>` or `url(`"
            ));
        } else {
            self.tokens.retain(|(n, _)| n != name);
            self.tokens.push((name.to_string(), value.trim().to_string()));
        }
        self
    }

    /// Brand color as `#rgb` or `#rrggbb`. Other values are ignored.
    pub fn brand(mut self, color: &str) -> Self {
        let hex = color.strip_prefix('#').unwrap_or("");
        if matches!(hex.len(), 3 | 6) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            self.brand = Some(format!("#{hex}"));
        }
        self
    }

    /// Font stack, such as `Inter, system-ui, sans-serif`. Fonts must be installed or served by
    /// the app; only letters, digits, spaces, commas, hyphens and quotes are allowed.
    pub fn font(mut self, stack: &str) -> Self {
        if !stack.is_empty() && stack.len() <= 200 && stack.chars().all(|c| c.is_alphanumeric() || " ,-'\"".contains(c))
        {
            self.font = Some(stack.to_string());
        }
        self
    }

    /// Corner radius in pixels, 0 to 12.
    pub fn radius(mut self, px: u8) -> Self {
        self.radius = Some(px.min(12));
        self
    }

    /// Tighter spacing for very dense screens.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// The name used in the `zeyn_theme` cookie and on `<html>`. `None` for `Theme::new()`.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Declarations of this theme, such as `--z-brand:#7a2448;`.
    fn declarations(&self, with_density: bool) -> String {
        let mut vars = String::new();
        if let Some(b) = &self.brand {
            vars.push_str(&format!("--z-brand:{b};"));
        }
        if let Some(f) = &self.font {
            vars.push_str(&format!("--z-font:{f};"));
        }
        if let Some(r) = self.radius {
            vars.push_str(&format!("--z-radius:{r}px;"));
        }
        if with_density && self.compact {
            vars.push_str("--z-density:0.75;");
        }
        for (n, v) in &self.tokens {
            vars.push_str(&format!("{n}:{v};"));
        }
        vars
    }

    /// The value this theme sets for a token, if any.
    fn get(&self, token: &str) -> Option<&str> {
        if token == "--z-brand" {
            if let Some(b) = &self.brand {
                return Some(b);
            }
        }
        self.tokens.iter().find(|(n, _)| n == token).map(|(_, v)| v.as_str())
    }

    fn is_builtin(&self) -> bool {
        matches!(self.name.as_deref(), Some("light" | "dark"))
    }
}

/// A token value that cannot leave its declaration or load anything.
fn safe_value(v: &str) -> bool {
    let lower = v.to_ascii_lowercase();
    !v.trim().is_empty()
        && v.len() <= 200
        && v.chars().all(|c| c.is_alphanumeric() || " #%.,()-+*/'\"".contains(c))
        && !lower.contains("url")
        && !lower.contains("expression")
        && !lower.contains("image")
}

/// Theme settings of an app, made with `App::themes`, `App::default_theme` and `App::theme_picker`.
pub(super) struct Settings {
    themes: Vec<Theme>,
    default: Option<String>,
    picker: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { themes: Vec::new(), default: None, picker: true }
    }
}

impl App {
    /// The themes users can choose in the shell. Without this call they are `Theme::light()` and
    /// `Theme::dark()`; with both, the picker also offers "System", which follows the device. A
    /// single theme locks the app to it.
    ///
    /// ```ignore
    /// App::new("Billing").themes([
    ///     Theme::light(),
    ///     Theme::dark(),
    ///     Theme::named("ocean", [("--z-brand", "#0b5394"), ("--z-bg", "#eef4f8")]).label(t!("theme.ocean")),
    /// ])
    /// ```
    pub fn themes(mut self, themes: impl IntoIterator<Item = Theme>) -> Self {
        self.looks.themes = themes.into_iter().collect();
        self
    }

    /// The theme of users who have not chosen one: `system` (the default when both built in
    /// themes are offered), `light`, `dark` or an app theme's name.
    pub fn default_theme(mut self, name: &str) -> Self {
        self.looks.default = Some(name.to_string());
        self
    }

    /// `false` hides the theme picker: everyone sees the default theme.
    pub fn theme_picker(mut self, show: bool) -> Self {
        self.looks.picker = show;
        self
    }

    /// The themes offered, after defaults.
    fn offered(&self) -> Vec<Theme> {
        match &self.theme {
            Some(t) if t.scheme.is_some() => vec![t.clone()],
            _ if self.looks.themes.is_empty() => vec![Theme::light(), Theme::dark()],
            _ => self.looks.themes.clone(),
        }
    }

    /// Theme problems for `App::check`: bad names and tokens, duplicates, an unknown default and
    /// contrast below WCAG 2.2 AA.
    pub(super) fn theme_problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let base = self.theme.clone().unwrap_or_default();
        problems.extend(base.problems.iter().cloned());
        let offered = self.offered();
        for (i, t) in offered.iter().enumerate() {
            problems.extend(t.problems.iter().cloned());
            let name = t.name.as_deref().unwrap_or("app");
            if t.name.is_none() {
                problems.push("`App::themes` got `Theme::new()`, which has no name\n  = help: use `Theme::light()`, `Theme::dark()` or `Theme::named(...)`".into());
            } else if offered[..i].iter().any(|o| o.name == t.name) {
                problems.push(format!("theme `{name}` is offered twice"));
            }
            problems.extend(contrast_problems(name, t, &base));
        }
        if let Some(d) = &self.looks.default {
            let system = d == "system" && has_both(&offered);
            if !system && !offered.iter().any(|t| t.name.as_deref() == Some(d)) {
                problems.push(format!("default theme `{d}` is not one of the offered themes\n  = help: add it to `App::themes` or use `system`"));
            }
        }
        problems
    }

    /// The resolved themes for the running app.
    pub(super) fn look(&self) -> Look {
        let base = self.theme.clone().unwrap_or_default();
        let offered = self.offered();
        let system = has_both(&offered);
        let mut css = String::new();
        let root = base.declarations(false);
        if !root.is_empty() {
            css.push_str(&format!(":root{{{root}}}\n"));
        }
        for t in offered.iter().filter(|_| base.scheme.is_none()) {
            let body = t.declarations(true);
            let name = t.name.clone().unwrap_or_default();
            if body.is_empty() {
                continue;
            }
            if t.is_builtin() {
                css.push_str(&format!(
                    "@media (prefers-color-scheme: {name}){{:root:not([data-theme]){{{body}}}}}\n:root[data-theme=\"{name}\"]:not([data-zeyn-theme]){{{body}}}\n"
                ));
            } else {
                css.push_str(&format!(":root[data-zeyn-theme=\"{name}\"]{{{body}}}\n"));
            }
        }
        let choices: Vec<Choice> = offered
            .iter()
            .filter_map(|t| {
                let name = t.name.clone()?;
                let label = match name.as_str() {
                    "light" | "dark" => {
                        t.label.clone().unwrap_or_else(|| Text::key(format!("zeyn.shell.theme_{name}")))
                    }
                    _ => t.label.clone().unwrap_or_else(|| Text::from(name.clone())),
                };
                Some(Choice { custom: !t.is_builtin(), name: name.into(), label, scheme: t.scheme.unwrap_or("light") })
            })
            .collect();
        let default: Arc<str> = match &self.looks.default {
            Some(d) => d.as_str().into(),
            None if system => "system".into(),
            None => choices.first().map(|c| c.name.clone()).unwrap_or_else(|| "system".into()),
        };
        let locked = choices.len() == 1;
        Look {
            version: format!("{:016x}", {
                use std::hash::Hasher;
                let mut h = std::hash::DefaultHasher::new();
                h.write(css.as_bytes());
                h.finish()
            }),
            css: css.into(),
            picker: self.looks.picker && !locked && choices.len() > 1,
            system,
            default,
            choices,
            compact: base.compact,
        }
    }
}

fn has_both(offered: &[Theme]) -> bool {
    ["light", "dark"].iter().all(|n| offered.iter().any(|t| t.name.as_deref() == Some(n)))
}

/// One entry of the theme picker.
pub(super) struct Choice {
    name: Arc<str>,
    label: Text,
    scheme: &'static str,
    custom: bool,
}

/// The themes of a running app: the stylesheet, built once, and what the picker offers.
pub(super) struct Look {
    pub(super) css: axum::body::Bytes,
    /// Hash of `css`, the `?v=` of the stylesheet URL.
    pub(super) version: String,
    picker: bool,
    system: bool,
    default: Arc<str>,
    choices: Vec<Choice>,
    compact: bool,
}

impl Look {
    /// The theme of a request: the cookie when the picker is on and it names an offered theme,
    /// otherwise the default.
    pub(super) fn pick(&self, wanted: Option<&str>) -> Arc<str> {
        wanted.filter(|_| self.picker).and_then(|w| self.allowed(w)).unwrap_or_else(|| self.default.clone())
    }

    fn allowed(&self, name: &str) -> Option<Arc<str>> {
        if name == "system" && self.system {
            return Some("system".into());
        }
        self.choices.iter().find(|c| &*c.name == name).map(|c| c.name.clone())
    }

    /// Attributes for `<html>` and the `color-scheme` meta value of a theme.
    pub(super) fn html(&self, name: &str) -> (String, &'static str) {
        let density = if self.compact { r#" data-density="compact""# } else { "" };
        match self.choices.iter().find(|c| &*c.name == name) {
            Some(c) if c.custom => {
                (format!(r#" data-theme="{}" data-zeyn-theme="{}"{density}"#, c.scheme, escape(&c.name)), c.scheme)
            }
            Some(c) => (format!(r#" data-theme="{}"{density}"#, c.scheme), c.scheme),
            None => (density.to_string(), "light dark"),
        }
    }

    /// The theme picker for the menu, as the Zeyn select control. Empty when hidden.
    pub(super) fn picker(&self, r: &Render, current: &str) -> String {
        if !self.picker {
            return String::new();
        }
        let option = |value: &str, label: String, scheme: &str, custom: bool| {
            let selected = if value == current { " selected" } else { "" };
            let custom = if custom { " data-custom" } else { "" };
            format!(r#"<option value="{}" data-scheme="{scheme}"{custom}{selected}>{label}</option>"#, escape(value))
        };
        let mut options = String::new();
        if self.system {
            options.push_str(&option("system", r.text(&Text::key("zeyn.shell.theme_system")), "", false));
        }
        for c in &self.choices {
            options.push_str(&option(&c.name, r.text(&c.label), c.scheme, c.custom));
        }
        format!(
            r#"<div class="z-locale"><span id="z-theme-label">{}</span><div class="z-select" data-text-none=""><select id="z-theme" data-zeyn-theme-picker>{options}</select></div></div>"#,
            r.text(&Text::key("zeyn.shell.theme"))
        )
    }
}

/// Contrast problems of a theme, with the organization's look and the kit's values under it.
fn contrast_problems(name: &str, theme: &Theme, base: &Theme) -> Vec<String> {
    let dark = theme.scheme == Some("dark");
    let value = |token: &str| -> Option<String> {
        if let Some(v) = theme.get(token) {
            return Some(v.to_string());
        }
        if dark {
            if let Some((_, v)) = DARK.iter().find(|(n, _)| *n == token) {
                return Some(v.to_string());
            }
        }
        base.get(token).or_else(|| LIGHT.iter().find(|(n, _)| *n == token).map(|(_, v)| *v)).map(str::to_string)
    };
    let mut problems = Vec::new();
    let mut pairs: Vec<(&str, &str, f64)> = PAIRS.to_vec();
    if !dark {
        pairs.push(("--z-brand", "--z-surface", 4.5));
    }
    for (fg, bg, needed) in pairs {
        let (Some(a), Some(b)) = (value(fg).and_then(|v| hex(&v)), value(bg).and_then(|v| hex(&v))) else { continue };
        let ratio = contrast(a, b);
        if ratio + 1e-9 < needed {
            problems.push(format!(
                "theme `{name}`: `{fg}` on `{bg}` has contrast {ratio:.2}:1; WCAG 2.2 AA needs {needed}:1\n  = help: change one of the two colors in the theme's tokens"
            ));
        }
    }
    problems
}

/// `#rgb` or `#rrggbb` as red, green and blue.
fn hex(v: &str) -> Option<[u8; 3]> {
    let h = v.trim().strip_prefix('#')?;
    let digits: Vec<u8> = h.chars().map(|c| c.to_digit(16).map(|d| d as u8)).collect::<Option<_>>()?;
    match digits.len() {
        3 => Some([digits[0] * 17, digits[1] * 17, digits[2] * 17]),
        6 => Some([digits[0] * 16 + digits[1], digits[2] * 16 + digits[3], digits[4] * 16 + digits[5]]),
        _ => None,
    }
}

/// WCAG 2.x contrast ratio of two sRGB colors.
fn contrast(a: [u8; 3], b: [u8; 3]) -> f64 {
    let lum = |c: [u8; 3]| {
        let ch = |v: u8| {
            let s = f64::from(v) / 255.0;
            if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2])
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Key of the saved theme in the app's view store.
fn store_key(ctx: &Ctx) -> String {
    format!("{}:~theme", ctx.tenant().map(|t| t.id().to_string()).unwrap_or_default())
}

const STORE_PREFIX: &str = "theme:";

/// On a full page load without a `zeyn_theme` cookie, the signed in user's theme from the view
/// store, with the cookie to set so later pages do not ask again.
pub(super) async fn remember(st: &AppState, ctx: Ctx, headers: &HeaderMap) -> (Ctx, Option<String>) {
    if !st.look.picker || cookie(headers, "zeyn_theme").is_some() {
        return (ctx, None);
    }
    let (Some(store), Some(user)) = (st.views.as_ref(), ctx.identity().user.clone()) else { return (ctx, None) };
    let saved = match st.timed(store.load(&user, &store_key(&ctx))).await {
        Ok(v) => v,
        Err(_) => {
            eprintln!("warning: the view store failed to load a theme; using the default");
            return (ctx, None);
        }
    };
    let name =
        saved.and_then(|v| v.columns.into_iter().find_map(|c| c.field.strip_prefix(STORE_PREFIX).map(str::to_string)));
    match name.and_then(|n| st.look.allowed(&n)) {
        Some(theme) => {
            let c = theme_cookie(st, &theme);
            (ctx.with_theme(theme), Some(c))
        }
        None => (ctx, None),
    }
}

fn theme_cookie(st: &AppState, name: &str) -> String {
    let secure = if st.secure_cookies { "; Secure" } else { "" };
    format!("zeyn_theme={name}; Path={}; Max-Age=31536000; SameSite=Lax{secure}", st.cookie_path())
}

#[derive(Deserialize)]
struct ThemeCall {
    theme: String,
}

/// `POST /_zeyn/theme {"theme": "dark"}`: stores the choice in the `zeyn_theme` cookie and, for a
/// signed in user in an app with a view store, in the store. `404` for a theme that is not offered
/// or when the picker is hidden.
async fn set_theme(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<ThemeCall>,
) -> Response {
    let ctx = st.ctx(&headers);
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let Some(theme) = st.look.allowed(&call.theme).filter(|_| st.look.picker) else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    if let (Some(store), Some(user)) = (st.views.as_ref(), ctx.identity().user.clone()) {
        let view = DataGridView {
            columns: vec![ColumnView { field: format!("{STORE_PREFIX}{theme}"), width: None, pin: None }],
            ..DataGridView::default()
        };
        if st.timed(store.save(&user, &store_key(&ctx), Some(view))).await.is_err() {
            eprintln!("warning: the view store failed to save a theme; the cookie still keeps it");
        }
    }
    with_cookie(StatusCode::NO_CONTENT.into_response(), Some(theme_cookie(&st, &theme)))
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router.route("/_zeyn/theme", post(set_theme))
}
