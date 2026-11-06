//! [`RangeSlider`]: two handles over one track, for price and date ranges.

use super::layout::into_node;
use super::{PageCheck, dom_id, topics_attr};
use crate::i18n::{NumberFormat, Text};
use crate::render::{Render, attr, escape};
use crate::screen::{Component, Node};
use crate::time::Date;
use crate::value::{Currency, FieldKind, Money, Value};
use serde_json::{Map, Value as Json, json};

into_node!(RangeSlider);

/// What the values of a slider are.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Scale {
    Number,
    Money(Currency),
    /// Days since 1970-01-01; the handles send ISO dates through hidden inputs.
    Date,
}

/// A range with two handles on one track, such as a price from ₺100 to ₺2,500 or the days of a
/// trip. Each handle is a native `<input type="range">`, so it follows the W3C APG slider pattern
/// by itself: one tab stop each, arrow keys move by the step, Home and End go to the ends, and
/// screen readers hear the formatted value (`aria-valuetext`). The small `range.js` module, loaded
/// only on pages with a slider, keeps the low handle below the high one, moves by a tenth of the
/// range on Page Up and Page Down, fills the track between the handles, writes the values next to
/// the label and moves the nearer handle when the track is pressed. It reads the track's size once
/// per press, never while a handle moves.
///
/// The handles carry the names of the two values, so the slider works in any form and as a filter:
/// `.drives(topic)` puts it in a filter form whose values go into the screen params of the same
/// names, and listeners of the topic refresh when a handle is let go. Inside a
/// [`super::FacetPanel`] the panel's form does that. A date slider sends ISO dates
/// (`2026-03-31`); a number or money slider sends whole numbers (money in whole units of the
/// currency).
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::RangeSlider;
///
/// #[params]
/// pub struct Listing {
///     pub min_price: Option<i64>,
///     pub max_price: Option<i64>,
/// }
///
/// #[screen(route = "/products", public)]
/// pub fn products(p: Listing) -> Screen {
///     Screen::new("Products")
///         .body(
///             RangeSlider::new("Price", ["min_price", "max_price"], 0, 5_000)
///                 .step(50)
///                 .money(Currency::Try)
///                 .value(p.min_price, p.max_price)
///                 .drives("products"),
///         )
///         .body(Kpi::new("Shown", 12.0).listens("products"))
/// }
///
/// let app = App::new("Shop").feature(Feature::new("shop").screen(products));
/// assert!(app.check_page(&products::run(Listing { min_price: None, max_price: None })).is_ok());
/// ```
pub struct RangeSlider {
    label: Text,
    names: [&'static str; 2],
    bounds: (i64, i64),
    step: i64,
    value: (i64, i64),
    scale: Scale,
    drives: Vec<&'static str>,
}

impl RangeSlider {
    /// A slider over whole numbers from `min` to `max`. `names` are the names the low and the
    /// high value are sent under, the fields of the screen params when it drives a topic.
    pub fn new(label: impl Into<Text>, names: [&'static str; 2], min: i64, max: i64) -> Self {
        let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
        RangeSlider {
            label: label.into(),
            names,
            bounds: (lo, hi),
            step: 1,
            value: (lo, hi),
            scale: Scale::Number,
            drives: Vec::new(),
        }
    }

    /// A slider over the days from `first` to `last`, sending ISO dates.
    pub fn dates(
        label: impl Into<Text>,
        names: [&'static str; 2],
        first: Date,
        last: Date,
    ) -> Self {
        let mut s = Self::new(
            label,
            names,
            first.days_since_epoch(),
            last.days_since_epoch(),
        );
        s.scale = Scale::Date;
        s
    }

    /// How far one arrow key moves a handle: 1 unless set; values snap to it from the minimum.
    pub fn step(mut self, step: i64) -> Self {
        self.step = step.max(1);
        self
    }

    /// Shows the values as money of `currency` in whole units, in the page's number format.
    pub fn money(mut self, currency: Currency) -> Self {
        if self.scale != Scale::Date {
            self.scale = Scale::Money(currency);
        }
        self
    }

    /// The chosen range, from the screen params; `None` keeps the end of the track. Values out of
    /// the bounds are clamped, and a low value above the high one is swapped.
    pub fn value(mut self, low: Option<i64>, high: Option<i64>) -> Self {
        let (min, max) = self.bounds;
        let lo = low.unwrap_or(min).clamp(min, max);
        let hi = high.unwrap_or(max).clamp(min, max);
        self.value = if lo <= hi { (lo, hi) } else { (hi, lo) };
        self
    }

    /// The chosen days of a date slider, from the screen params.
    pub fn date_value(self, from: Option<Date>, to: Option<Date>) -> Self {
        self.value(
            from.map(|d| d.days_since_epoch()),
            to.map(|d| d.days_since_epoch()),
        )
    }

    /// Puts the slider in a filter form: when a handle is let go, its values go into the screen
    /// params of its names and the listeners of `topics` refresh in place.
    pub fn drives(mut self, topics: impl super::Topics) -> Self {
        self.drives = super::topic_list(topics);
        self
    }

    /// The `data-zeyn-kinds` entries of the two names: numbers, unless the slider sends dates.
    pub(crate) fn kinds(&self, out: &mut Map<String, Json>) {
        if self.scale != Scale::Date {
            for n in self.names {
                out.insert(n.to_string(), Json::from("number"));
            }
        }
    }

    /// A value as the page shows it.
    fn shown(&self, r: &Render, v: i64) -> String {
        match self.scale {
            Scale::Number => r.value(&Value::Int(v), &FieldKind::Integer),
            Scale::Money(c) => escape(&Money::new(v.saturating_mul(100), c).format(r.locale)),
            Scale::Date => Date::from_days_since_epoch(v)
                .map(|d| r.value(&Value::Date(d), &FieldKind::Date))
                .unwrap_or_default(),
        }
    }

    /// The share of the track at `v`, in percent.
    fn at(&self, v: i64) -> f64 {
        let (min, max) = self.bounds;
        if max == min {
            return 0.0;
        }
        (v - min) as f64 * 100.0 / (max - min) as f64
    }

    /// The slider without its own form, also used inside a facet panel.
    pub(crate) fn html(&self, r: &Render) -> String {
        let id = dom_id(r, "range");
        let label = r.text(&self.label);
        let (min, max) = self.bounds;
        let (lo, hi) = self.value;
        let f = NumberFormat::for_locale(r.locale);
        let format = match self.scale {
            Scale::Number => json!({ "kind": "number", "group": f.group.to_string() }),
            Scale::Money(c) => json!({
                "kind": "money",
                "group": f.group.to_string(),
                "decimal": f.decimal.to_string(),
                "symbol": c.symbol(),
                "first": f.symbol_first,
            }),
            Scale::Date => json!({ "kind": "date" }),
        };
        let mut out = format!(
            r#"<div class="z-range" role="group" aria-labelledby="{id}-label" data-zeyn-range data-format="{}" style="--z-lo:{:.2}%;--z-hi:{:.2}%"><div class="z-range-head"><span class="z-range-label" id="{id}-label">{label}</span><span class="z-range-values" aria-hidden="true"><span class="z-range-lo">{}</span><span class="z-range-sep">{}</span><span class="z-range-hi">{}</span></span></div><div class="z-range-track"><span class="z-range-fill"></span>"#,
            attr(&format),
            self.at(lo),
            self.at(hi),
            self.shown(r, lo),
            r.text(&Text::key("zeyn.ui.shop.range.to")),
            self.shown(r, hi),
        );
        let dates = self.scale == Scale::Date;
        for (i, (v, key)) in [(lo, "low"), (hi, "high")]
            .into_iter()
            .enumerate()
        {
            let name = if dates {
                String::new()
            } else {
                format!(r#" name="{}""#, escape(self.names[i]))
            };
            out.push_str(&format!(
                r#"<input type="range" class="z-range-input z-range-{key}" id="{id}-{key}"{name} min="{min}" max="{max}" step="{}" value="{v}" aria-label="{}" aria-valuetext="{}" data-testid="{}.range.{}">"#,
                self.step,
                r.text(&Text::key(format!("zeyn.ui.shop.range.{key}")).arg("label", self.label.clone())),
                self.shown(r, v),
                r.scope,
                escape(self.names[i]),
            ));
        }
        out.push_str("</div>");
        if dates {
            for (i, v) in [lo, hi].into_iter().enumerate() {
                let iso = Date::from_days_since_epoch(v)
                    .map(|d| d.to_string())
                    .unwrap_or_default();
                out.push_str(&format!(
                    r#"<input type="hidden" class="z-range-date" name="{}" value="{iso}">"#,
                    escape(self.names[i])
                ));
            }
        }
        out.push_str("</div>");
        out
    }

    pub(crate) fn check(&self, check: &mut PageCheck) {
        let [a, b] = self.names;
        if a.is_empty() || b.is_empty() || a == b {
            check.problem(
                "a RangeSlider needs two different names, such as `[\"min_price\", \"max_price\"]`",
            );
        }
        if self.bounds.0 == self.bounds.1 {
            check.problem(
                "a RangeSlider's minimum and maximum are the same; give it a range to pick from",
            );
        }
    }
}

impl Component for RangeSlider {
    fn render(&self, out: &mut String, r: &Render) {
        if self.drives.is_empty() {
            out.push_str(&self.html(r));
            return;
        }
        let mut kinds = Map::new();
        self.kinds(&mut kinds);
        let kinds = if kinds.is_empty() {
            String::new()
        } else {
            format!(r#" data-zeyn-kinds="{}""#, attr(&kinds))
        };
        out.push_str(&format!(
            r#"<form class="z-range-form"{}{kinds} novalidate>{}</form>"#,
            topics_attr("data-zeyn-drives", &self.drives),
            self.html(r)
        ));
    }

    fn schema(&self) -> Json {
        json!({
            "type": "range_slider",
            "label": self.label,
            "names": self.names,
            "min": self.bounds.0,
            "max": self.bounds.1,
            "step": self.step,
            "value": [self.value.0, self.value.1],
            "drives": self.drives,
        })
    }

    fn page_check(&self, check: &mut PageCheck) {
        self.check(check);
        for t in &self.drives {
            check.drives(t);
        }
    }
}
