//! [`ComparisonTable`]: plans side by side, and [`ImageCompare`]: two pictures under a slider.

use super::dom_id;
use super::layout::into_node;
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(ComparisonTable, ImageCompare);

/// Most plans a [`ComparisonTable`] holds.
pub const MAX_COMPARE_PLANS: usize = 6;

/// Most feature rows a [`ComparisonTable`] holds.
pub const MAX_COMPARE_ROWS: usize = 100;

/// What a plan has of a feature: it, not it, or a text such as "10 GB".
#[derive(Clone)]
pub enum CompareCell {
    Yes,
    No,
    Text(Text),
}

impl From<bool> for CompareCell {
    fn from(has: bool) -> Self {
        if has {
            CompareCell::Yes
        } else {
            CompareCell::No
        }
    }
}

impl From<&'static str> for CompareCell {
    fn from(text: &'static str) -> Self {
        CompareCell::Text(text.into())
    }
}

impl From<String> for CompareCell {
    fn from(text: String) -> Self {
        CompareCell::Text(text.into())
    }
}

impl From<Text> for CompareCell {
    fn from(text: Text) -> Self {
        CompareCell::Text(text)
    }
}

/// Plans or products side by side, one column each and one row a feature, with a check or a
/// cross (read as "Included" and "Not included") or a text in each cell. `.featured()` sets the
/// plan added last apart.
///
/// ```
/// use zeyn::ui::{CompareCell, ComparisonTable};
/// let plans = ComparisonTable::new("Plans")
///     .plan("Free")
///     .plan("Team")
///     .featured()
///     .row("Users", ["3", "25"])
///     .row("Audit log", [false, true]);
/// ```
pub struct ComparisonTable {
    caption: Text,
    plans: Vec<Text>,
    featured: Option<usize>,
    rows: Vec<(Text, Vec<CompareCell>)>,
}

impl ComparisonTable {
    pub fn new(caption: impl Into<Text>) -> Self {
        ComparisonTable {
            caption: caption.into(),
            plans: Vec::new(),
            featured: None,
            rows: Vec::new(),
        }
    }

    /// Adds a column. At most [`MAX_COMPARE_PLANS`].
    pub fn plan(mut self, name: impl Into<Text>) -> Self {
        if self.plans.len() < MAX_COMPARE_PLANS {
            self.plans.push(name.into());
        }
        self
    }

    /// Sets the plan added last apart from the others.
    pub fn featured(mut self) -> Self {
        self.featured = self.plans.len().checked_sub(1);
        self
    }

    /// Adds a feature with a cell for each plan, in order. At most [`MAX_COMPARE_ROWS`].
    pub fn row<I, C>(mut self, feature: impl Into<Text>, cells: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<CompareCell>,
    {
        if self.rows.len() < MAX_COMPARE_ROWS {
            let cells = cells
                .into_iter()
                .take(MAX_COMPARE_PLANS)
                .map(Into::into)
                .collect();
            self.rows.push((feature.into(), cells));
        }
        self
    }

    fn class(&self, plan: usize) -> &'static str {
        if self.featured == Some(plan) {
            r#" class="z-compare-featured""#
        } else {
            ""
        }
    }
}

impl Component for ComparisonTable {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "compare");
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.compare.{key}")));
        out.push_str(&format!(
            r#"<div class="z-table-scroll" role="group" aria-labelledby="{id}" tabindex="0"><table class="z-table z-compare"><caption class="z-table-caption" id="{id}">{}</caption><thead><tr><td></td>"#,
            r.text(&self.caption)
        ));
        for (i, plan) in self.plans.iter().enumerate() {
            out.push_str(&format!(
                r#"<th scope="col"{}>{}</th>"#,
                self.class(i),
                r.text(plan)
            ));
        }
        out.push_str("</tr></thead><tbody>");
        for (feature, cells) in &self.rows {
            out.push_str(&format!(r#"<tr><th scope="row">{}</th>"#, r.text(feature)));
            for i in 0..self.plans.len() {
                let cell = match cells.get(i) {
                    Some(CompareCell::Yes) => format!(
                        r#"<span class="z-compare-yes" aria-hidden="true">&#10003;</span><span class="z-sr-only">{}</span>"#,
                        t("yes")
                    ),
                    Some(CompareCell::No) => format!(
                        r#"<span class="z-compare-no" aria-hidden="true">&#10005;</span><span class="z-sr-only">{}</span>"#,
                        t("no")
                    ),
                    Some(CompareCell::Text(text)) => r.text(text),
                    None => String::new(),
                };
                out.push_str(&format!("<td{}>{cell}</td>", self.class(i)));
            }
            out.push_str("</tr>");
        }
        out.push_str("</tbody></table></div>");
    }

    fn schema(&self) -> Json {
        let rows: Vec<Json> = self
            .rows
            .iter()
            .map(|(feature, cells)| {
                let cells: Vec<Json> = cells
                    .iter()
                    .map(|c| match c {
                        CompareCell::Yes => json!(true),
                        CompareCell::No => json!(false),
                        CompareCell::Text(t) => json!(t),
                    })
                    .collect();
                json!({ "feature": feature, "cells": cells })
            })
            .collect();
        json!({ "type": "comparison_table", "caption": self.caption, "plans": self.plans, "featured": self.featured, "rows": rows })
    }
}

/// Two pictures of the same size on top of one another with a slider that uncovers one over the
/// other, such as before and after. The pictures follow the [`super::Image`] rules; one that is
/// not allowed is left out. The `imgcompare.js` module shows the slider, so without script the
/// pictures show half and half.
///
/// ```
/// use zeyn::ui::ImageCompare;
/// let room = ImageCompare::new("Before and after the repair")
///     .before("/img/room-before.jpg", "The room with the old floor")
///     .after("/img/room-after.jpg", "The room with the new floor")
///     .size(1200, 800);
/// ```
pub struct ImageCompare {
    label: Text,
    before: Option<(String, Text)>,
    after: Option<(String, Text)>,
    size: (u32, u32),
}

impl ImageCompare {
    /// A comparison whose slider is named by `label`.
    pub fn new(label: impl Into<Text>) -> Self {
        ImageCompare {
            label: label.into(),
            before: None,
            after: None,
            size: (4, 3),
        }
    }

    /// The picture on the start side, with its alternative text.
    pub fn before(mut self, src: impl Into<String>, alt: impl Into<Text>) -> Self {
        self.before = Some((src.into(), alt.into()));
        self
    }

    /// The picture on the end side, with its alternative text.
    pub fn after(mut self, src: impl Into<String>, alt: impl Into<Text>) -> Self {
        self.after = Some((src.into(), alt.into()));
        self
    }

    /// The size of the pictures in pixels; it gives the box its shape before they load and is its
    /// largest width.
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.size = (width.max(1), height.max(1));
        self
    }
}

impl Component for ImageCompare {
    fn render(&self, out: &mut String, r: &Render) {
        let (w, h) = self.size;
        let policy = super::embed::policy(r);
        let image = |side: &Option<(String, Text)>, class: &str| {
            side.as_ref()
                .filter(|(src, _)| policy.allows_image(src))
                .map(|(src, alt)| {
                    format!(
                        r#"<img class="z-imgcompare-img {class}" src="{}" alt="{}" width="{w}" height="{h}" loading="lazy" decoding="async">"#,
                        escape(src),
                        r.text(alt)
                    )
                })
                .unwrap_or_default()
        };
        out.push_str(&format!(
            r#"<div class="z-imgcompare" data-zeyn-imgcompare style="--z-imgcompare-ratio:{w} / {h};--z-imgcompare-width:{w}px">{}{}<span class="z-imgcompare-line" aria-hidden="true"></span><input class="z-imgcompare-range" type="range" min="0" max="100" value="50" aria-label="{}" data-testid="{}.imgcompare" hidden></div>"#,
            image(&self.after, "z-imgcompare-after"),
            image(&self.before, "z-imgcompare-before"),
            r.text(&self.label),
            r.scope
        ));
    }

    fn schema(&self) -> Json {
        json!({
            "type": "image_compare",
            "label": self.label,
            "before": self.before.as_ref().map(|(src, _)| src),
            "after": self.after.as_ref().map(|(src, _)| src),
            "size": [self.size.0, self.size.1],
        })
    }
}
