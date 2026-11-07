//! [`VariantPicker`]: color swatches and size chips that choose a product variant.

use super::layout::into_node;
use super::{PageCheck, dom_id, topics_attr};
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(VariantPicker);

/// Most options one group of a [`VariantPicker`] shows; more are left out.
pub const MAX_VARIANT_OPTIONS: usize = 40;

/// Most groups one [`VariantPicker`] holds; more are left out.
pub const MAX_VARIANT_GROUPS: usize = 4;

/// How a group draws its options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    Swatch,
    Chip,
}

struct Choice {
    value: String,
    label: Text,
    color: Option<String>,
    reason: Option<Text>,
}

/// One choice of a [`VariantPicker`], such as the color or the size, sent under `name`: a group
/// of radio buttons drawn as color swatches ([`VariantGroup::swatches`]) or as chips with text
/// ([`VariantGroup::chips`]).
///
/// ```
/// use zeyn::ui::VariantGroup;
/// let colors = VariantGroup::swatches("color", "Color")
///     .swatch("red", "Red", "#c0392b")
///     .swatch("navy", "Navy", "#1f3a68")
///     .unavailable("navy", "Sold out")
///     .current(Some("red"));
/// ```
pub struct VariantGroup {
    name: &'static str,
    label: Text,
    look: Look,
    options: Vec<Choice>,
    current: Option<String>,
}

impl VariantGroup {
    /// Color swatches: round marks of a color with the name for screen readers and as a tooltip.
    pub fn swatches(name: &'static str, label: impl Into<Text>) -> Self {
        VariantGroup {
            name,
            label: label.into(),
            look: Look::Swatch,
            options: Vec::new(),
            current: None,
        }
    }

    /// Chips with a short text each, such as sizes.
    pub fn chips(name: &'static str, label: impl Into<Text>) -> Self {
        VariantGroup {
            look: Look::Chip,
            ..Self::swatches(name, label)
        }
    }

    /// A swatch: the value it sends, its name and its color, a hex color (`#c0392b`) or a theme
    /// token (`--z-surface-2`). Any other color draws an empty swatch, and `App::check_page`
    /// reports it.
    pub fn swatch(
        mut self,
        value: impl Into<String>,
        label: impl Into<Text>,
        color: impl Into<String>,
    ) -> Self {
        self.push(value.into(), label.into(), Some(color.into()));
        self
    }

    /// A chip: the value it sends and its text.
    pub fn option(mut self, value: impl Into<String>, label: impl Into<Text>) -> Self {
        self.push(value.into(), label.into(), None);
        self
    }

    fn push(&mut self, value: String, label: Text, color: Option<String>) {
        if self.options.len() < MAX_VARIANT_OPTIONS
            && !self
                .options
                .iter()
                .any(|o| o.value == value)
        {
            self.options.push(Choice {
                value,
                label,
                color,
                reason: None,
            });
        }
    }

    /// Turns an option off with the reason screen readers and the tooltip give, such as
    /// "Sold out" or "Not in this color". Call it after the option is added.
    pub fn unavailable(mut self, value: &str, reason: impl Into<Text>) -> Self {
        if let Some(o) = self
            .options
            .iter_mut()
            .find(|o| o.value == value)
        {
            o.reason = Some(reason.into());
        }
        self
    }

    /// The value chosen now, from the screen params. A value that is no option chooses nothing.
    pub fn current(mut self, value: Option<&str>) -> Self {
        self.current = value.map(str::to_string);
        self
    }

    fn render(&self, out: &mut String, r: &Render, id: &str) {
        let chosen = self
            .options
            .iter()
            .find(|o| self.current.as_deref() == Some(o.value.as_str()));
        let class = match self.look {
            Look::Swatch => "z-variant-group z-variant-swatches",
            Look::Chip => "z-variant-group z-variant-chips",
        };
        let shown = chosen
            .map(|o| {
                format!(
                    r#"<span class="z-variant-chosen">{}</span>"#,
                    r.text(&o.label)
                )
            })
            .unwrap_or_default();
        out.push_str(&format!(
            r#"<fieldset class="{class}"><legend class="z-variant-legend"><span class="z-variant-name">{}</span>{shown}</legend><div class="z-variant-options">"#,
            r.text(&self.label)
        ));
        let gid = format!("{id}-{}", escape(self.name));
        for (i, o) in self.options.iter().enumerate() {
            let checked = if chosen.is_some_and(|c| std::ptr::eq(c, o)) {
                " checked"
            } else {
                ""
            };
            let label = r.text(&o.label);
            let (off, reason, title) = match &o.reason {
                Some(why) => {
                    let why = r.text(why);
                    (
                        format!(r#" disabled aria-describedby="{gid}-{i}-why""#),
                        format!(r#"<span class="z-sr-only" id="{gid}-{i}-why">{why}</span>"#),
                        format!("{label}: {why}"),
                    )
                }
                None => (String::new(), String::new(), label.clone()),
            };
            let mark = match (
                self.look,
                o.color
                    .as_deref()
                    .and_then(swatch_color),
            ) {
                (Look::Swatch, Some(c)) => format!(
                    r#"<span class="z-variant-mark" style="--z-swatch:{c}" aria-hidden="true"></span><span class="z-sr-only">{label}</span>"#
                ),
                (Look::Swatch, None) => format!(
                    r#"<span class="z-variant-mark" aria-hidden="true"></span><span class="z-sr-only">{label}</span>"#
                ),
                (Look::Chip, _) => format!(r#"<span class="z-variant-mark">{label}</span>"#),
            };
            let class = if o.reason.is_some() {
                "z-variant z-variant-off"
            } else {
                "z-variant"
            };
            out.push_str(&format!(
                r#"<label class="{class}" title="{title}"><input type="radio" class="z-variant-input" id="{gid}-{i}" name="{}" value="{}" data-testid="{}.variant.{}.{}"{checked}{off}>{mark}{reason}</label>"#,
                escape(self.name),
                escape(&o.value),
                r.scope,
                escape(self.name),
                escape(&o.value),
            ));
        }
        out.push_str("</div></fieldset>");
    }
}

/// A swatch color as CSS: a checked hex color or a theme token.
fn swatch_color(color: &str) -> Option<String> {
    let hex = color
        .strip_prefix('#')
        .is_some_and(|h| {
            matches!(h.len(), 3 | 4 | 6 | 8) && h.bytes().all(|b| b.is_ascii_hexdigit())
        });
    let token = color
        .strip_prefix("--z-")
        .is_some_and(|t| {
            !t.is_empty()
                && t.len() <= 40
                && t.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        });
    match (hex, token) {
        (true, _) => Some(color.to_string()),
        (_, true) => Some(format!("var({color})")),
        _ => None,
    }
}

/// The variant choice of a product page: color swatches and size chips as W3C APG radio groups.
/// Each group is a fieldset of native radio buttons named like a screen param, so Tab reaches the
/// chosen option, the arrow keys move between options and choose them, and the legend says the
/// group's name and the chosen option ("Color Red"). An option that cannot be had is turned off
/// with its reason, which screen readers read with it and the tooltip shows.
///
/// The picker is a filter form that drives `topics` and listens to them itself: choosing an
/// option puts its value into the screen params of the group's name, keeps the address in step
/// (so the URL of the chosen variant can be shared) and refreshes, in one request, every node
/// that listens to the topics: the price, the gallery, the stock line and the picker, whose
/// options the screen turns off again for the new choice. It needs no script beyond the topics
/// module.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::{VariantGroup, VariantPicker};
///
/// #[params]
/// pub struct ShirtParams {
///     pub color: Option<String>,
///     pub size: Option<String>,
/// }
///
/// #[screen(route = "/shirt", public)]
/// pub fn shirt(p: ShirtParams) -> Screen {
///     let price = if p.size.as_deref() == Some("xl") { 34_900 } else { 29_900 };
///     Screen::new("Linen shirt")
///         .body(
///             VariantPicker::new()
///                 .group(
///                     VariantGroup::swatches("color", "Color")
///                         .swatch("sand", "Sand", "#d8c3a5")
///                         .swatch("navy", "Navy", "#1f3a68")
///                         .current(p.color.as_deref()),
///                 )
///                 .group(
///                     VariantGroup::chips("size", "Size")
///                         .option("m", "M")
///                         .option("xl", "XL")
///                         .unavailable("xl", "Sold out")
///                         .current(p.size.as_deref()),
///                 )
///                 .drives("variant"),
///         )
///         .body(Price::new(Money::tl(price)).listens("variant"))
/// }
///
/// let app = App::new("Shop").feature(Feature::new("shop").screen(shirt));
/// assert!(app.check_page(&shirt::run(ShirtParams { color: None, size: None })).is_ok());
/// ```
pub struct VariantPicker {
    groups: Vec<VariantGroup>,
    drives: Vec<&'static str>,
}

impl Default for VariantPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl VariantPicker {
    pub fn new() -> Self {
        VariantPicker {
            groups: Vec::new(),
            drives: Vec::new(),
        }
    }

    /// A group of options, such as the colors. At most [`MAX_VARIANT_GROUPS`].
    pub fn group(mut self, group: VariantGroup) -> Self {
        if self.groups.len() < MAX_VARIANT_GROUPS {
            self.groups.push(group);
        }
        self
    }

    /// The topics a choice refreshes; the picker listens to them too.
    pub fn drives(mut self, topics: impl super::Topics) -> Self {
        self.drives = super::topic_list(topics);
        self
    }
}

impl Component for VariantPicker {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "variants");
        out.push_str(&format!(
            r#"<form class="z-variants"{}{} novalidate>"#,
            topics_attr("data-zeyn-drives", &self.drives),
            topics_attr("data-zeyn-listens", &self.drives),
        ));
        for g in &self.groups {
            g.render(out, r, &id);
        }
        out.push_str("</form>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "variant_picker",
            "drives": self.drives,
            "groups": self
                .groups
                .iter()
                .map(|g| json!({
                    "name": g.name,
                    "label": g.label,
                    "current": g.current,
                    "options": g
                        .options
                        .iter()
                        .map(|o| json!({ "value": o.value, "label": o.label, "available": o.reason.is_none() }))
                        .collect::<Vec<_>>(),
                }))
                .collect::<Vec<_>>(),
        })
    }

    fn page_check(&self, check: &mut PageCheck) {
        if self.drives.is_empty() {
            check.problem("a VariantPicker refreshes nothing; add `.drives(\"variant\")` and `.listens(\"variant\")` on the price, the gallery and the stock it changes");
        }
        for t in &self.drives {
            check.drives(t);
            check.listens(t);
        }
        for g in &self.groups {
            if g.name.is_empty() {
                check.problem("a VariantGroup needs a name, the screen param it sets");
            }
            for o in &g.options {
                if g.look == Look::Swatch
                    && o.color
                        .as_deref()
                        .and_then(swatch_color)
                        .is_none()
                {
                    check.problem(format!(
                        "swatch `{}` of `{}` has no color; give a hex color such as `#c0392b` or a theme token such as `--z-surface-2`",
                        o.value, g.name
                    ));
                }
            }
        }
    }
}
