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
    "--z-raised",
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
    "--z-chart-7",
    "--z-chart-8",
];

/// Values of the kit's light theme, as in `zeyn.css`.
const LIGHT: &[(&str, &str)] = &[
    ("--z-brand", "#0f6674"),
    ("--z-on-brand", "#ffffff"),
    ("--z-bg", "#f5f6f8"),
    ("--z-surface", "#ffffff"),
    ("--z-surface-2", "#f8f9fa"),
    ("--z-raised", "#ffffff"),
    ("--z-hover", "#eff1f4"),
    ("--z-border", "#e2e5e9"),
    ("--z-border-strong", "#858d97"),
    ("--z-text", "#15181d"),
    ("--z-text-muted", "#5a626c"),
    ("--z-danger", "#b42318"),
    ("--z-warning", "#8a5300"),
    ("--z-success", "#1a6b3c"),
    ("--z-info", "#275d93"),
    ("--z-chart-1", "#2563d9"),
    ("--z-chart-2", "#d67400"),
    ("--z-chart-3", "#a3369b"),
    ("--z-chart-4", "#14866d"),
    ("--z-chart-5", "#7a6a00"),
    ("--z-chart-6", "#6e56cf"),
    ("--z-chart-7", "#cc3b33"),
    ("--z-chart-8", "#56636f"),
];

/// Values the kit's dark theme sets on top of the light ones, as in `zeyn.css`.
const DARK: &[(&str, &str)] = &[
    ("--z-bg", "#0d0f12"),
    ("--z-surface", "#15181d"),
    ("--z-surface-2", "#1a1e24"),
    ("--z-raised", "#1f242b"),
    ("--z-hover", "#232830"),
    ("--z-border", "#2b3139"),
    ("--z-border-strong", "#6f7884"),
    ("--z-text", "#e7eaee"),
    ("--z-text-muted", "#9ba4af"),
    ("--z-danger", "#f28b82"),
    ("--z-warning", "#e6b45a"),
    ("--z-success", "#6fcf8f"),
    ("--z-info", "#8ab8e6"),
    ("--z-chart-1", "#5b9bff"),
    ("--z-chart-2", "#f08a3c"),
    ("--z-chart-3", "#e27bd6"),
    ("--z-chart-4", "#38c29f"),
    ("--z-chart-5", "#ece27a"),
    ("--z-chart-6", "#9d8cff"),
    ("--z-chart-7", "#ff7b70"),
    ("--z-chart-8", "#9aa7b5"),
];

/// Color pairs that must reach WCAG 2.2 AA: foreground, background, needed ratio. Derived
/// tokens (accent, primary, the subtle tints) are computed the way `zeyn.css` computes them.
const PAIRS: &[(&str, &str, f64)] = &[
    ("--z-text", "--z-bg", 4.5),
    ("--z-text", "--z-surface", 4.5),
    ("--z-text", "--z-surface-2", 4.5),
    ("--z-text", "--z-raised", 4.5),
    ("--z-text", "--z-hover", 4.5),
    ("--z-text", "--z-brand-subtle", 4.5),
    ("--z-text", "--z-danger-subtle", 4.5),
    ("--z-text", "--z-warning-subtle", 4.5),
    ("--z-text", "--z-success-subtle", 4.5),
    ("--z-text", "--z-info-subtle", 4.5),
    ("--z-text-muted", "--z-bg", 4.5),
    ("--z-text-muted", "--z-surface", 4.5),
    ("--z-text-muted", "--z-surface-2", 4.5),
    ("--z-text-muted", "--z-raised", 4.5),
    ("--z-text-muted", "--z-hover", 4.5),
    ("--z-text-muted", "--z-brand-subtle", 4.5),
    ("--z-border-strong", "--z-surface", 3.0),
    ("--z-border-strong", "--z-surface-2", 3.0),
    ("--z-border-strong", "--z-raised", 3.0),
    ("--z-accent", "--z-surface", 4.5),
    ("--z-accent", "--z-bg", 4.5),
    ("--z-accent", "--z-brand-subtle", 4.5),
    ("--z-focus", "--z-surface", 3.0),
    ("--z-focus", "--z-bg", 3.0),
    ("--z-on-primary", "--z-primary", 4.5),
    ("--z-on-primary", "--z-primary-hover", 4.5),
    ("--z-on-primary", "--z-primary-active", 4.5),
    ("--z-danger", "--z-surface", 4.5),
    ("--z-danger", "--z-danger-subtle", 4.5),
    ("--z-warning", "--z-surface", 4.5),
    ("--z-warning", "--z-warning-subtle", 4.5),
    ("--z-success", "--z-surface", 4.5),
    ("--z-success", "--z-success-subtle", 4.5),
    ("--z-info", "--z-surface", 4.5),
    ("--z-info", "--z-info-subtle", 4.5),
    ("--z-on-brand", "--z-brand", 4.5),
    ("--z-chart-1", "--z-surface", 3.0),
    ("--z-chart-2", "--z-surface", 3.0),
    ("--z-chart-3", "--z-surface", 3.0),
    ("--z-chart-4", "--z-surface", 3.0),
    ("--z-chart-5", "--z-surface", 3.0),
    ("--z-chart-6", "--z-surface", 3.0),
    ("--z-chart-7", "--z-surface", 3.0),
    ("--z-chart-8", "--z-surface", 3.0),
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
        Theme {
            name: Some("light".into()),
            scheme: Some("light"),
            ..Theme::default()
        }
    }

    /// The built in dark theme. Given to `App::theme` it locks the app to dark.
    pub fn dark() -> Self {
        Theme {
            name: Some("dark".into()),
            scheme: Some("dark"),
            ..Theme::default()
        }
    }

    /// An app theme built on the light theme: a name (`a` to `z`, digits and `-`, up to 32) and
    /// the tokens it changes, such as `[("--z-brand", "#0b5394"), ("--z-bg", "#eef4f8")]`. Bad
    /// names, unknown tokens, unsafe values and low contrast are reported by `App::check`.
    pub fn named<'a>(name: &str, tokens: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut theme = Theme {
            name: Some(name.to_string()),
            scheme: Some("light"),
            ..Theme::default()
        };
        let valid = !name.is_empty()
            && name.len() <= 32
            && name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !valid || matches!(name, "system" | "light" | "dark") {
            theme.problems.push(
                crate::Problem::new("Z0113", format!("theme name `{name}` is not allowed"))
                    .why("the name goes into a cookie and an HTML attribute, and `system`, `light` and `dark` are the built in choices")
                    .fix("use `a` to `z`, digits and `-` (up to 32), and not `system`, `light` or `dark`")
                    .docs("guides/theming#light-dark-and-app-themes")
                    .to_string(),
            );
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
        let owner = self
            .name
            .clone()
            .unwrap_or_else(|| "app".into());
        if !THEME_TOKENS.contains(&name) {
            self.problems.push(
                crate::Problem::new(
                    "Z0114",
                    format!("theme `{owner}`: `{name}` is not a theme token"),
                )
                .why("only the listed tokens are read by the components, so this value would change nothing")
                .fix("use one of the tokens in docs/theming.md")
                .docs("guides/theming#2-base-tokens")
                .to_string(),
            );
        } else if !safe_value(value) {
            self.problems.push(
                crate::Problem::new(
                    "Z0115",
                    format!("theme `{owner}`: the value of `{name}` is not allowed"),
                )
                .why("the value is written into the page's style sheet, where these characters could break out of the declaration or load files")
                .fix("use a color, length or font list without `;`, braces, `\\`, `<`, `>` or `url(`")
                .docs("guides/theming#2-base-tokens")
                .to_string(),
            );
        } else {
            self.tokens.retain(|(n, _)| n != name);
            self.tokens
                .push((name.to_string(), value.trim().to_string()));
        }
        self
    }

    /// Brand color as `#rgb` or `#rrggbb`. Other values are ignored.
    pub fn brand(mut self, color: &str) -> Self {
        let hex = color.strip_prefix('#').unwrap_or("");
        if matches!(hex.len(), 3 | 6)
            && hex
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        {
            self.brand = Some(format!("#{hex}"));
        }
        self
    }

    /// Font stack, such as `Inter, system-ui, sans-serif`. Fonts must be installed or served by
    /// the app; only letters, digits, spaces, commas, hyphens and quotes are allowed.
    pub fn font(mut self, stack: &str) -> Self {
        if !stack.is_empty()
            && stack.len() <= 200
            && stack
                .chars()
                .all(|c| c.is_alphanumeric() || " ,-'\"".contains(c))
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
        if token == "--z-brand"
            && let Some(b) = &self.brand
        {
            return Some(b);
        }
        self.tokens
            .iter()
            .find(|(n, _)| n == token)
            .map(|(_, v)| v.as_str())
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
        && v.chars()
            .all(|c| c.is_alphanumeric() || " #%.,()-+*/'\"".contains(c))
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
        Settings {
            themes: Vec::new(),
            default: None,
            picker: true,
        }
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

    /// Hides the theme picker: everyone sees the default theme.
    pub fn no_theme_picker(mut self) -> Self {
        self.looks.picker = false;
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
                problems.push(
                    crate::Problem::new(
                        "Z0116",
                        "`App::themes` got `Theme::new()`, which has no name",
                    )
                    .why("the theme picker and the theme cookie choose themes by name, so a theme without one cannot be picked")
                    .fix("use `Theme::light()`, `Theme::dark()` or `Theme::named(...)`")
                    .docs("guides/theming#light-dark-and-app-themes")
                    .to_string(),
                );
            } else if offered[..i]
                .iter()
                .any(|o| o.name == t.name)
            {
                problems.push(
                    crate::Problem::new("Z0117", format!("theme `{name}` is offered twice"))
                        .why("the theme picker would list it twice and only the first one would ever apply")
                        .fix("remove one of them from `App::themes`, or give it another name")
                        .docs("guides/theming#light-dark-and-app-themes")
                        .to_string(),
                );
            }
            problems.extend(contrast_problems(name, t, &base));
        }
        if let Some(d) = &self.looks.default {
            let system = d == "system" && has_both(&offered);
            if !system
                && !offered
                    .iter()
                    .any(|t| t.name.as_deref() == Some(d))
            {
                problems.push(
                    crate::Problem::new(
                        "Z0118",
                        format!("default theme `{d}` is not one of the offered themes"),
                    )
                    .why("a new visitor would start on a theme the picker does not list")
                    .fix("add it to `App::themes` or use `system`")
                    .docs("guides/theming#light-dark-and-app-themes")
                    .to_string(),
                );
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
        for t in offered
            .iter()
            .filter(|_| base.scheme.is_none())
        {
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
                    "light" | "dark" => t
                        .label
                        .clone()
                        .unwrap_or_else(|| Text::key(format!("zeyn.shell.theme_{name}"))),
                    _ => t
                        .label
                        .clone()
                        .unwrap_or_else(|| Text::from(name.clone())),
                };
                Some(Choice {
                    custom: !t.is_builtin(),
                    name: name.into(),
                    label,
                    scheme: t.scheme.unwrap_or("light"),
                })
            })
            .collect();
        let default: Arc<str> = match &self.looks.default {
            Some(d) => d.as_str().into(),
            None if system => "system".into(),
            None => choices
                .first()
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "system".into()),
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
    ["light", "dark"].iter().all(|n| {
        offered
            .iter()
            .any(|t| t.name.as_deref() == Some(n))
    })
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
        wanted
            .filter(|_| self.picker)
            .and_then(|w| self.allowed(w))
            .unwrap_or_else(|| self.default.clone())
    }

    fn allowed(&self, name: &str) -> Option<Arc<str>> {
        if name == "system" && self.system {
            return Some("system".into());
        }
        self.choices
            .iter()
            .find(|c| &*c.name == name)
            .map(|c| c.name.clone())
    }

    /// Attributes for `<html>` and the `color-scheme` meta value of a theme.
    pub(super) fn html(&self, name: &str) -> (String, &'static str) {
        let density = if self.compact {
            r#" data-density="compact""#
        } else {
            ""
        };
        match self
            .choices
            .iter()
            .find(|c| &*c.name == name)
        {
            Some(c) if c.custom => (
                format!(
                    r#" data-theme="{}" data-zeyn-theme="{}"{density}"#,
                    c.scheme,
                    escape(&c.name)
                ),
                c.scheme,
            ),
            Some(c) => (format!(r#" data-theme="{}"{density}"#, c.scheme), c.scheme),
            None => (density.to_string(), "light dark"),
        }
    }

    /// The theme picker for the menu, as the Zeyn select control. Empty when hidden.
    pub(super) fn picker(&self, r: &Render, current: &str) -> String {
        self.picker_as(r, current, "z-theme", "z-locale")
    }

    /// The theme picker with the select's DOM id and the class of its wrapper. Every picker on
    /// a page carries `data-zeyn-theme-picker`, so the runtime keeps them in step.
    pub(crate) fn picker_as(&self, r: &Render, current: &str, id: &str, class: &str) -> String {
        if !self.picker {
            return String::new();
        }
        let option = |value: &str, label: String, scheme: &str, custom: bool| {
            let selected = if value == current { " selected" } else { "" };
            let custom = if custom { " data-custom" } else { "" };
            format!(
                r#"<option value="{}" data-scheme="{scheme}"{custom}{selected}>{label}</option>"#,
                escape(value)
            )
        };
        let mut options = String::new();
        if self.system {
            options.push_str(&option(
                "system",
                r.text(&Text::key("zeyn.shell.theme_system")),
                "",
                false,
            ));
        }
        for c in &self.choices {
            options.push_str(&option(&c.name, r.text(&c.label), c.scheme, c.custom));
        }
        format!(
            r#"<div class="{class}"><span id="{id}-label">{}</span><div class="z-select" data-text-none=""><select id="{id}" data-zeyn-theme-picker>{options}</select></div></div>"#,
            r.text(&Text::key("zeyn.shell.theme"))
        )
    }
}

/// The app's themes as a service, so a [`crate::ui::ThemePicker`] in a screen offers the same
/// choices as the shell's picker.
#[derive(Clone)]
pub(crate) struct Looks(pub(crate) Arc<Look>);

impl crate::ctx::Service for Looks {}

/// The theme picker of a screen for the request's user, empty when the app offers no choice.
pub(crate) fn screen_picker(ctx: &crate::ctx::Ctx, r: &Render, id: &str) -> String {
    ctx.service::<Looks>()
        .map(|l| {
            l.0.picker_as(
                r,
                ctx.theme().unwrap_or("system"),
                id,
                "z-field z-theme-field",
            )
        })
        .unwrap_or_default()
}

/// Contrast problems of a theme, with the organization's look and the kit's values under it.
fn contrast_problems(name: &str, theme: &Theme, base: &Theme) -> Vec<String> {
    contrast_table(theme, base)
        .into_iter()
        .filter(|(_, _, ratio, needed)| ratio + 1e-9 < *needed)
        .map(|(fg, bg, ratio, needed)| {
crate::Problem::new(
    "Z0119",
    format!("theme `{name}`: `{fg}` on `{bg}` has contrast {ratio:.2}:1; WCAG 2.2 AA needs {needed}:1"),
)
.why("text or controls in these colors would be hard to read for many users, and WCAG 2.2 AA is the baseline")
.fix("change one of the two colors in the theme's tokens")
.docs("guides/theming#5-verified-contrast")
.to_string()
        })
        .collect()
}

/// Every pair of [`PAIRS`] with its measured ratio and the ratio it needs. Derived tokens the
/// theme does not set are computed as `zeyn.css` computes them; pairs whose colors are not
/// plain hex values (such as `var(...)` set by the app) are left out.
fn contrast_table(theme: &Theme, base: &Theme) -> Vec<(&'static str, &'static str, f64, f64)> {
    let dark = theme.scheme == Some("dark");
    let set = |token: &str| -> Option<[u8; 3]> {
        if let Some(v) = theme.get(token) {
            return hex(v);
        }
        if dark && let Some((_, v)) = DARK.iter().find(|(n, _)| *n == token) {
            return hex(v);
        }
        base.get(token)
            .or_else(|| {
                LIGHT
                    .iter()
                    .find(|(n, _)| *n == token)
                    .map(|(_, v)| *v)
            })
            .and_then(hex)
    };
    let tint = theme
        .get("--z-tint")
        .or_else(|| base.get("--z-tint"))
        .and_then(|v| {
            v.trim()
                .strip_suffix('%')?
                .trim()
                .parse::<f64>()
                .ok()
        })
        .unwrap_or(if dark { 16.0 } else { 9.0 })
        / 100.0;
    let resolve = |token: &str| -> Option<[u8; 3]> {
        if let Some(c) = set(token) {
            return Some(c);
        }
        let brand = set("--z-brand")?;
        let accent = theme
            .get("--z-accent")
            .and_then(hex)
            .unwrap_or_else(|| if dark { tone(brand, 0.8) } else { brand });
        let primary = theme
            .get("--z-primary")
            .and_then(hex)
            .unwrap_or(if dark { accent } else { brand });
        match token {
            "--z-accent" | "--z-focus" => Some(accent),
            "--z-primary" => Some(primary),
            "--z-on-primary" if dark => hex("#0d0f12"),
            "--z-on-primary" => set("--z-on-brand"),
            "--z-primary-hover" if dark => Some(mix(primary, [255; 3], 0.84)),
            "--z-primary-hover" => Some(mix(primary, [0; 3], 0.86)),
            "--z-primary-active" if dark => Some(mix(primary, [255; 3], 0.70)),
            "--z-primary-active" => Some(mix(primary, [0; 3], 0.74)),
            "--z-brand-subtle" => Some(mix(accent, set("--z-surface")?, tint)),
            _ => {
                let status = token.strip_suffix("-subtle")?;
                Some(mix(set(status)?, set("--z-surface")?, tint))
            }
        }
    };
    let mut pairs: Vec<(&'static str, &'static str, f64)> = PAIRS.to_vec();
    if !dark {
        pairs.push(("--z-brand", "--z-surface", 4.5));
    }
    pairs
        .into_iter()
        .filter_map(|(fg, bg, needed)| {
            let ratio = contrast(resolve(fg)?, resolve(bg)?);
            Some((fg, bg, ratio, needed))
        })
        .collect()
}

/// `#rgb` or `#rrggbb` as red, green and blue.
fn hex(v: &str) -> Option<[u8; 3]> {
    let h = v.trim().strip_prefix('#')?;
    let digits: Vec<u8> = h
        .chars()
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<_>>()?;
    match digits.len() {
        3 => Some([digits[0] * 17, digits[1] * 17, digits[2] * 17]),
        6 => Some([
            digits[0] * 16 + digits[1],
            digits[2] * 16 + digits[3],
            digits[4] * 16 + digits[5],
        ]),
        _ => None,
    }
}

/// An 8 bit sRGB channel in linear light.
fn linear(v: u8) -> f64 {
    let s = f64::from(v) / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// An sRGB color in Oklab.
fn oklab(c: [u8; 3]) -> [f64; 3] {
    let [r, g, b] = c.map(linear);
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    ]
}

/// An Oklab color back in 8 bit sRGB, clipped to the gamut.
fn from_oklab([l, a, b]: [f64; 3]) -> [u8; 3] {
    let l_ = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m_ = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s_ = (l - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    let lin = [
        4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_,
        -1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701 * s_,
    ];
    lin.map(|v| {
        let v = v.clamp(0.0, 1.0);
        let s = if v <= 0.003_130_8 {
            v * 12.92
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    })
}

/// `color-mix(in oklab, a share, b)`: `share` of `a` (0 to 1) over `b`.
fn mix(a: [u8; 3], b: [u8; 3], share: f64) -> [u8; 3] {
    let (x, y) = (oklab(a), oklab(b));
    from_oklab([0, 1, 2].map(|i| x[i] * share + y[i] * (1.0 - share)))
}

/// `oklch(from c l c h)`: the color's hue and chroma at another lightness.
fn tone(c: [u8; 3], lightness: f64) -> [u8; 3] {
    let [_, a, b] = oklab(c);
    from_oklab([lightness, a, b])
}

/// WCAG 2.x contrast ratio of two sRGB colors.
fn contrast(a: [u8; 3], b: [u8; 3]) -> f64 {
    let lum = |c: [u8; 3]| {
        let [r, g, b] = c.map(linear);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    };
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Key of the saved theme in the app's view store.
fn store_key(ctx: &Ctx) -> String {
    format!(
        "{}:~theme",
        ctx.tenant()
            .map(|t| t.id().to_string())
            .unwrap_or_default()
    )
}

const STORE_PREFIX: &str = "theme:";

/// On a full page load without a `zeyn_theme` cookie, the signed in user's theme from the view
/// store, with the cookie to set so later pages do not ask again.
pub(super) async fn remember(
    st: &AppState,
    ctx: Ctx,
    headers: &HeaderMap,
) -> (Ctx, Option<String>) {
    if !st.look.picker || cookie(headers, "zeyn_theme").is_some() {
        return (ctx, None);
    }
    let (Some(store), Some(user)) = (st.views.as_ref(), ctx.identity().user.clone()) else {
        return (ctx, None);
    };
    let saved = match st
        .timed(store.load(&user, &store_key(&ctx)))
        .await
    {
        Ok(v) => v,
        Err(_) => {
            eprintln!("warning: the view store failed to load a theme; using the default");
            return (ctx, None);
        }
    };
    let name = saved.and_then(|v| {
        v.columns.into_iter().find_map(|c| {
            c.field
                .strip_prefix(STORE_PREFIX)
                .map(str::to_string)
        })
    });
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
    format!(
        "zeyn_theme={name}; Path={}; Max-Age=31536000; SameSite=Lax{secure}",
        st.cookie_path()
    )
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
    let Some(theme) = st
        .look
        .allowed(&call.theme)
        .filter(|_| st.look.picker)
    else {
        return st.error_response(&ctx, Failure::App(Error::NotFound));
    };
    if let (Some(store), Some(user)) = (st.views.as_ref(), ctx.identity().user.clone()) {
        let view = DataGridView {
            columns: vec![ColumnView {
                field: format!("{STORE_PREFIX}{theme}"),
                width: None,
                pin: None,
            }],
            ..DataGridView::default()
        };
        if st
            .timed(store.save(&user, &store_key(&ctx), Some(view)))
            .await
            .is_err()
        {
            eprintln!("warning: the view store failed to save a theme; the cookie still keeps it");
        }
    }
    with_cookie(
        StatusCode::NO_CONTENT.into_response(),
        Some(theme_cookie(&st, &theme)),
    )
}

#[derive(Deserialize)]
struct DensityCall {
    density: String,
}

/// `POST /_zeyn/density {"density": "compact"}`: stores the density in the `zeyn_density` cookie
/// (`comfortable` or `compact`), which the server renders on `<html>` and from which it picks the
/// row height of virtual grids. `404` for any other value.
async fn set_density(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<DensityCall>,
) -> Response {
    let ctx = st.ctx(&headers);
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let value = match call.density.as_str() {
        "compact" => "compact",
        "comfortable" => "comfortable",
        _ => return st.error_response(&ctx, Failure::App(Error::NotFound)),
    };
    let secure = if st.secure_cookies { "; Secure" } else { "" };
    with_cookie(
        StatusCode::NO_CONTENT.into_response(),
        Some(format!(
            "zeyn_density={value}; Path={}; Max-Age=31536000; SameSite=Lax{secure}",
            st.cookie_path()
        )),
    )
}

#[derive(Deserialize)]
struct RailCall {
    rail: bool,
}

/// `POST /_zeyn/rail {"rail": true}`: stores whether the side menu is folded to an icon rail in
/// the `zeyn_rail` cookie, so the next page renders it that way without a jump.
async fn set_rail(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::Json(call): axum::Json<RailCall>,
) -> Response {
    let ctx = st.ctx(&headers);
    if let Err(f) = st.check_csrf(&headers) {
        return st.error_response(&ctx, f);
    }
    let secure = if st.secure_cookies { "; Secure" } else { "" };
    let (value, age) = if call.rail {
        ("1", 31_536_000)
    } else {
        ("", 0)
    };
    with_cookie(
        StatusCode::NO_CONTENT.into_response(),
        Some(format!(
            "zeyn_rail={value}; Path={}; Max-Age={age}; SameSite=Lax{secure}",
            st.cookie_path()
        )),
    )
}

/// The density picker of the account menu: comfortable or compact.
pub(super) fn density_picker(r: &Render, compact: bool) -> String {
    let t = |k: &str| r.text(&Text::key(format!("zeyn.shell.{k}")));
    let (a, b) = if compact {
        ("", " selected")
    } else {
        (" selected", "")
    };
    format!(
        r#"<div class="z-locale"><span id="z-density-label">{}</span><div class="z-select" data-text-none=""><select id="z-density" data-zeyn-density aria-labelledby="z-density-label"><option value="comfortable"{a}>{}</option><option value="compact"{b}>{}</option></select></div></div>"#,
        t("density"),
        t("density_comfortable"),
        t("density_compact")
    )
}

pub(super) fn routes(router: Router<Arc<AppState>>) -> Router<Arc<AppState>> {
    router
        .route("/_zeyn/theme", post(set_theme))
        .route("/_zeyn/density", post(set_density))
        .route("/_zeyn/rail", post(set_rail))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kit's own light and dark themes pass every pair of [`PAIRS`]. Run with
    /// `--nocapture` to print the table that `docs/theming.md` section 5 quotes.
    #[test]
    fn built_in_themes_reach_wcag_aa_on_every_pair() {
        for theme in [Theme::light(), Theme::dark()] {
            let table = contrast_table(&theme, &Theme::new());
            assert!(table.len() >= PAIRS.len());
            for (fg, bg, ratio, needed) in table {
                println!(
                    "{} | `{fg}` on `{bg}` | {ratio:.2} | {needed}",
                    theme.scheme.unwrap_or_default()
                );
                assert!(ratio >= needed, "{fg} on {bg}: {ratio:.2} < {needed}");
            }
        }
    }

    /// Mixing in Oklab gives what browsers compute for `color-mix` and relative colors.
    #[test]
    fn oklab_mix_and_tone_match_the_browser() {
        assert_eq!(mix([255; 3], [0; 3], 1.0), [255; 3]);
        let teal = hex("#0f6674").unwrap();
        let tinted = mix(teal, [255; 3], 0.09);
        assert!((0xe6..=0xec).contains(&tinted[0]), "{tinted:?}");
        let accent = tone(teal, 0.8);
        assert!(accent[1] >= 0xb8 && accent[2] >= 0xc0, "{accent:?}");
    }
}
