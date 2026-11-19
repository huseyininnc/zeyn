//! [`Funnel`] and [`ActivityRings`]: stages that narrow, and shares of several goals as rings.

use super::layout::into_node;
use crate::grid::Tone;
use crate::i18n::Text;
use crate::render::{Render, push_grouped};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Funnel, ActivityRings);

/// Most stages a [`Funnel`] holds.
pub const MAX_FUNNEL_STAGES: usize = 10;

/// Most rings an [`ActivityRings`] holds.
pub const MAX_RINGS: usize = 4;

/// The stages of a process that loses some at each step, such as visits, carts and orders. Each
/// stage is a centered bar as wide as its share of the first stage, with its number and that
/// share as text.
///
/// ```
/// use zeyn::ui::Funnel;
/// let sales = Funnel::new("Sales funnel")
///     .stage("Visits", 12_400)
///     .stage("Carts", 3_100)
///     .stage("Orders", 870);
/// ```
pub struct Funnel {
    label: Text,
    stages: Vec<(Text, u64)>,
    tone: Tone,
}

impl Funnel {
    pub fn new(label: impl Into<Text>) -> Self {
        Funnel {
            label: label.into(),
            stages: Vec::new(),
            tone: Tone::Brand,
        }
    }

    /// Adds a stage. At most [`MAX_FUNNEL_STAGES`].
    pub fn stage(mut self, title: impl Into<Text>, value: u64) -> Self {
        if self.stages.len() < MAX_FUNNEL_STAGES {
            self.stages.push((title.into(), value));
        }
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// The share of the first stage a value is, in percent, at most 100.
    fn share(&self, value: u64) -> u64 {
        let first = self
            .stages
            .first()
            .map_or(0, |(_, v)| *v)
            .max(1);
        (u128::from(value) * 100 / u128::from(first)).min(100) as u64
    }
}

impl Component for Funnel {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<ol class="z-funnel z-tone-{}" aria-label="{}">"#,
            self.tone.code(),
            r.text(&self.label)
        ));
        for (title, value) in &self.stages {
            let share = self.share(*value);
            let mut number = String::new();
            push_grouped(
                &mut number,
                i64::try_from(*value).unwrap_or(i64::MAX),
                r.locale,
            );
            out.push_str(&format!(
                r#"<li class="z-funnel-stage"><span class="z-funnel-name">{}</span><span class="z-funnel-track"><span class="z-funnel-bar" style="--z-funnel:{share}%"></span></span><span class="z-funnel-value">{number}</span><span class="z-funnel-share">{share}%</span></li>"#,
                r.text(title)
            ));
        }
        out.push_str("</ol>");
    }

    fn schema(&self) -> Json {
        let stages: Vec<Json> = self
            .stages
            .iter()
            .map(|(t, v)| json!({ "title": t, "value": v, "share": self.share(*v) }))
            .collect();
        json!({ "type": "funnel", "label": self.label, "tone": self.tone, "stages": stages })
    }
}

struct Ring {
    title: Text,
    value: u64,
    max: u64,
    tone: Tone,
}

/// Up to four shares of a whole as rings inside one another, such as the goals of a day, with
/// each name and percent written next to them.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::ActivityRings;
/// let day = ActivityRings::new("Today")
///     .ring("Orders packed", 42, 60, Tone::Brand)
///     .ring("Calls answered", 18, 20, Tone::Success);
/// ```
pub struct ActivityRings {
    label: Text,
    rings: Vec<Ring>,
    size: u16,
}

impl ActivityRings {
    pub fn new(label: impl Into<Text>) -> Self {
        ActivityRings {
            label: label.into(),
            rings: Vec::new(),
            size: 120,
        }
    }

    /// Adds a ring, from the outside in. At most [`MAX_RINGS`].
    pub fn ring(mut self, title: impl Into<Text>, value: u64, max: u64, tone: Tone) -> Self {
        if self.rings.len() < MAX_RINGS {
            let max = max.max(1);
            self.rings.push(Ring {
                title: title.into(),
                value: value.min(max),
                max,
                tone,
            });
        }
        self
    }

    /// The diameter in CSS pixels, 64 to 240. The default is 120.
    pub fn size(mut self, px: u16) -> Self {
        self.size = px.clamp(64, 240);
        self
    }
}

impl Component for ActivityRings {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="z-rings" role="group" aria-label="{}"><svg class="z-rings-svg" viewBox="0 0 36 36" width="{1}" height="{1}" aria-hidden="true" focusable="false">"#,
            r.text(&self.label),
            self.size
        ));
        for (i, ring) in self.rings.iter().enumerate() {
            let radius = 16.0 - 4.0 * i as f32;
            let percent = ring.value * 100 / ring.max;
            out.push_str(&format!(
                r#"<circle class="z-rings-track" cx="18" cy="18" r="{radius}" pathLength="100"/><circle class="z-rings-bar z-tone-{}" cx="18" cy="18" r="{radius}" pathLength="100" stroke-dasharray="{percent} 100"/>"#,
                ring.tone.code()
            ));
        }
        out.push_str(r#"</svg><ul class="z-rings-legend">"#);
        for ring in &self.rings {
            out.push_str(&format!(
                r#"<li class="z-tone-{}"><span class="z-rings-dot" aria-hidden="true"></span><span class="z-rings-name">{}</span><span class="z-rings-value">{}%</span></li>"#,
                ring.tone.code(),
                r.text(&ring.title),
                ring.value * 100 / ring.max
            ));
        }
        out.push_str("</ul></div>");
    }

    fn schema(&self) -> Json {
        let rings: Vec<Json> = self
            .rings
            .iter()
            .map(|g| json!({ "title": g.title, "value": g.value, "max": g.max, "tone": g.tone }))
            .collect();
        json!({ "type": "activity_rings", "label": self.label, "size": self.size, "rings": rings })
    }
}
