//! Charts drawn on the server as SVG: bar, line and donut. Each chart has a title, a text
//! summary for screen readers, a legend, and its numbers as a table under "Show data". Colors
//! come from the `--z-chart-1` to `--z-chart-6` tokens, so they follow the theme.

use super::{dom_id, layout::into_node};
use crate::i18n::{NumberFormat, Text};
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde::Serialize;
use serde_json::{Value as Json, json};

into_node!(Chart);

const W: f64 = 640.0;
const H: f64 = 260.0;
const LEFT: f64 = 56.0;
const RIGHT: f64 = 12.0;
const TOP: f64 = 12.0;
const BOTTOM: f64 = 32.0;

/// Kind of chart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Bar,
    Line,
    /// One series as parts of a whole.
    Donut,
}

/// A chart with data from the screen.
///
/// ```
/// use zeyn::ui::Chart;
/// let c = Chart::bar("Sales").labels(["Jan", "Feb"]).series("2025", [120.0, 80.0]);
/// let mut out = String::new();
/// zeyn::Component::render(&c, &mut out, &zeyn::Render::english("s"));
/// assert!(out.contains(r#"role="img""#));
/// ```
pub struct Chart {
    kind: ChartKind,
    title: Text,
    labels: Vec<Text>,
    series: Vec<(Text, Vec<f64>)>,
    decimals: u8,
}

impl Chart {
    fn new(kind: ChartKind, title: impl Into<Text>) -> Self {
        Self { kind, title: title.into(), labels: Vec::new(), series: Vec::new(), decimals: 0 }
    }

    pub fn bar(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::Bar, title)
    }

    pub fn line(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::Line, title)
    }

    pub fn donut(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::Donut, title)
    }

    /// Category names along the bottom, or the parts of a donut.
    pub fn labels<I, T>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Text>,
    {
        self.labels = labels.into_iter().map(Into::into).collect();
        self
    }

    /// A named series, one value per label. A donut uses the first series only.
    pub fn series(mut self, name: impl Into<Text>, values: impl IntoIterator<Item = f64>) -> Self {
        self.series.push((name.into(), values.into_iter().map(|v| if v.is_finite() { v } else { 0.0 }).collect()));
        self
    }

    /// Decimals shown in labels and the table, 0 to 4.
    pub fn decimals(mut self, n: u8) -> Self {
        self.decimals = n.min(4);
        self
    }

    fn num(&self, v: f64, locale: &str) -> String {
        let f = NumberFormat::for_locale(locale);
        let text = format!("{:.*}", self.decimals as usize, v.abs());
        let (whole, frac) = text.split_once('.').unwrap_or((&text, ""));
        let mut grouped = String::new();
        for (i, c) in whole.chars().enumerate() {
            if i > 0 && (whole.len() - i) % 3 == 0 {
                grouped.push(f.group);
            }
            grouped.push(c);
        }
        let sign = if v < 0.0 { "-" } else { "" };
        if frac.is_empty() { format!("{sign}{grouped}") } else { format!("{sign}{grouped}{}{frac}", f.decimal) }
    }

    fn value(&self, s: usize, i: usize) -> f64 {
        self.series.get(s).and_then(|(_, v)| v.get(i)).copied().unwrap_or(0.0)
    }

    /// A round top for the value axis and the number of steps up to it: steps of 1, 2 or 5 times
    /// a power of ten, at most five of them.
    fn scale(&self) -> (f64, u32) {
        let max = self.series.iter().flat_map(|(_, v)| v.iter().copied()).fold(0.0_f64, f64::max);
        if max <= 0.0 {
            return (1.0, 1);
        }
        let power = 10f64.powf((max / 5.0).log10().floor());
        let step = [1.0, 2.0, 5.0, 10.0].iter().map(|m| m * power).find(|s| (max / s).ceil() <= 5.0).unwrap_or(max);
        let ticks = (max / step).ceil().max(1.0) as u32;
        (step * ticks as f64, ticks)
    }

    fn axes(&self, out: &mut String, r: &Render, (max, ticks): (f64, u32)) {
        let plot_h = H - TOP - BOTTOM;
        for k in 0..=ticks {
            let v = max * k as f64 / ticks as f64;
            let y = TOP + plot_h - plot_h * k as f64 / ticks as f64;
            out.push_str(&format!(
                r#"<line class="z-chart-grid" x1="{LEFT}" x2="{}" y1="{y:.1}" y2="{y:.1}"/><text class="z-chart-axis" x="{}" y="{:.1}" text-anchor="end">{}</text>"#,
                W - RIGHT,
                LEFT - 8.0,
                y + 4.0,
                escape(&self.num(v, r.locale))
            ));
        }
        let n = self.labels.len().max(1) as f64;
        let step = (W - LEFT - RIGHT) / n;
        for (i, label) in self.labels.iter().enumerate() {
            let x = LEFT + step * (i as f64 + 0.5);
            out.push_str(&format!(
                r#"<text class="z-chart-axis" x="{x:.1}" y="{}" text-anchor="middle">{}</text>"#,
                H - 10.0,
                r.text(label)
            ));
        }
    }

    fn bars(&self, out: &mut String, r: &Render) {
        let scale = self.scale();
        let max = scale.0;
        self.axes(out, r, scale);
        let plot_h = H - TOP - BOTTOM;
        let n = self.labels.len().max(1);
        let step = (W - LEFT - RIGHT) / n as f64;
        let count = self.series.len().max(1) as f64;
        let width = step * 0.7 / count;
        for i in 0..n {
            for (s, (name, _)) in self.series.iter().enumerate() {
                let v = self.value(s, i).max(0.0);
                let h = plot_h * v / max;
                let x = LEFT + step * i as f64 + step * 0.15 + width * s as f64;
                out.push_str(&format!(
                    r#"<rect class="z-chart-bar z-series-{}" x="{x:.1}" y="{:.1}" width="{:.1}" height="{h:.1}" rx="2"><title>{}, {}: {}</title></rect>"#,
                    s % 6,
                    TOP + plot_h - h,
                    (width - 2.0).max(1.0),
                    self.labels.get(i).map(|l| r.text(l)).unwrap_or_default(),
                    r.text(name),
                    escape(&self.num(v, r.locale))
                ));
            }
        }
    }

    fn lines(&self, out: &mut String, r: &Render) {
        let scale = self.scale();
        let max = scale.0;
        self.axes(out, r, scale);
        let plot_h = H - TOP - BOTTOM;
        let n = self.labels.len().max(1);
        let step = (W - LEFT - RIGHT) / n as f64;
        for (s, (name, values)) in self.series.iter().enumerate() {
            let points: Vec<(f64, f64)> = (0..n.min(values.len()))
                .map(|i| (LEFT + step * (i as f64 + 0.5), TOP + plot_h - plot_h * values[i].max(0.0) / max))
                .collect();
            let path: Vec<String> = points.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect();
            out.push_str(&format!(
                r#"<polyline class="z-chart-line z-series-{}" points="{}"/>"#,
                s % 6,
                path.join(" ")
            ));
            for (i, (x, y)) in points.iter().enumerate() {
                out.push_str(&format!(
                    r#"<circle class="z-chart-point z-series-{}" cx="{x:.1}" cy="{y:.1}" r="4"><title>{}, {}: {}</title></circle>"#,
                    s % 6,
                    self.labels.get(i).map(|l| r.text(l)).unwrap_or_default(),
                    r.text(name),
                    escape(&self.num(values[i], r.locale))
                ));
            }
        }
    }

    fn ring(&self, out: &mut String, r: &Render) {
        let values: Vec<f64> =
            self.series.first().map(|(_, v)| v.iter().map(|x| x.max(0.0)).collect()).unwrap_or_default();
        let total: f64 = values.iter().sum();
        let (cx, cy, radius) = (W / 2.0, H / 2.0, 90.0);
        let circumference = 2.0 * std::f64::consts::PI * radius;
        out.push_str(&format!(r#"<circle class="z-chart-track" cx="{cx}" cy="{cy}" r="{radius}"/>"#));
        let mut offset = 0.0;
        for (i, v) in values.iter().enumerate() {
            if total <= 0.0 || *v <= 0.0 {
                continue;
            }
            let len = circumference * v / total;
            out.push_str(&format!(
                r#"<circle class="z-chart-slice z-series-{}" cx="{cx}" cy="{cy}" r="{radius}" stroke-dasharray="{len:.2} {:.2}" stroke-dashoffset="{:.2}" transform="rotate(-90 {cx} {cy})"><title>{}: {} ({:.0}%)</title></circle>"#,
                i % 6,
                circumference - len,
                -offset,
                self.labels.get(i).map(|l| r.text(l)).unwrap_or_default(),
                escape(&self.num(*v, r.locale)),
                v * 100.0 / total
            ));
            offset += len;
        }
        out.push_str(&format!(
            r#"<text class="z-chart-total" x="{cx}" y="{}" text-anchor="middle">{}</text>"#,
            cy + 8.0,
            escape(&self.num(total, r.locale))
        ));
    }

    /// A sentence describing the data, for screen readers.
    fn summary(&self, r: &Render) -> String {
        let mut best: Option<(f64, String)> = None;
        for (name, values) in &self.series {
            for (i, v) in values.iter().enumerate() {
                if best.as_ref().is_none_or(|(b, _)| v > b) {
                    let label = self.labels.get(i).map(|l| r.i18n.text(r.locale, l)).unwrap_or_default();
                    let who =
                        if self.series.len() > 1 { format!("{label}, {}", r.i18n.text(r.locale, name)) } else { label };
                    best = Some((*v, who));
                }
            }
        }
        let kind = match self.kind {
            ChartKind::Bar => "bar",
            ChartKind::Line => "line",
            ChartKind::Donut => "donut",
        };
        let text = Text::key(format!("zeyn.ui.chart.summary_{kind}"))
            .arg("items", self.labels.len().to_string())
            .arg("series", self.series.len().to_string())
            .arg("max", best.as_ref().map(|(_, w)| w.clone()).unwrap_or_default())
            .arg("value", best.map(|(v, _)| self.num(v, r.locale)).unwrap_or_default());
        r.text(&text)
    }
}

impl Component for Chart {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "chart");
        let title = r.text(&self.title);
        out.push_str(&format!(
            r#"<figure class="z-chart"><figcaption class="z-chart-title" id="{id}-title">{title}</figcaption><svg class="z-chart-svg" viewBox="0 0 {W} {H}" role="img" aria-labelledby="{id}-title {id}-desc" focusable="false"><desc id="{id}-desc">{}</desc>"#,
            self.summary(r)
        ));
        match self.kind {
            ChartKind::Bar => self.bars(out, r),
            ChartKind::Line => self.lines(out, r),
            ChartKind::Donut => self.ring(out, r),
        }
        out.push_str(r#"</svg><ul class="z-chart-legend">"#);
        let names: Vec<&Text> = if self.kind == ChartKind::Donut {
            self.labels.iter().collect()
        } else {
            self.series.iter().map(|(n, _)| n).collect()
        };
        for (i, name) in names.iter().enumerate() {
            out.push_str(&format!(
                r#"<li><span class="z-chart-swatch z-series-{}" aria-hidden="true"></span>{}</li>"#,
                i % 6,
                r.text(name)
            ));
        }
        out.push_str(&format!(
            r#"</ul><details class="z-chart-data"><summary>{}</summary><table class="z-chart-table"><caption class="z-sr-only">{}</caption><thead><tr><th scope="col"></th>"#,
            r.text(&Text::key("zeyn.ui.chart.data")),
            r.text(&Text::key("zeyn.ui.chart.table").arg("title", self.title.clone()))
        ));
        for (name, _) in &self.series {
            out.push_str(&format!(r#"<th scope="col" class="z-num">{}</th>"#, r.text(name)));
        }
        out.push_str("</tr></thead><tbody>");
        for (i, label) in self.labels.iter().enumerate() {
            out.push_str(&format!(r#"<tr><th scope="row">{}</th>"#, r.text(label)));
            for s in 0..self.series.len() {
                out.push_str(&format!(r#"<td class="z-num">{}</td>"#, escape(&self.num(self.value(s, i), r.locale))));
            }
            out.push_str("</tr>");
        }
        out.push_str("</tbody></table></details></figure>");
    }

    fn schema(&self) -> Json {
        let series: Vec<Json> = self.series.iter().map(|(n, v)| json!({ "name": n, "values": v })).collect();
        json!({ "type": "chart", "kind": self.kind, "title": self.title, "labels": self.labels, "series": series })
    }
}
