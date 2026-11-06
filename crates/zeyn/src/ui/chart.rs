//! Charts drawn on the server as SVG: bar, stacked bar, line, area, combined bar and line,
//! scatter, donut, gauge and funnel. Each chart has a title, a text summary for screen readers,
//! a legend, and its numbers as a table under "Show data". Every data point is a mark that the
//! keyboard reaches (one tab stop, arrow keys between marks, see `charts.js`) with its label and
//! value as its name; hover and focus show the same text in a tooltip. Colors come from the
//! `--z-chart-1` to `--z-chart-6` tokens, so they follow the theme.
//!
//! Sparklines and bullets are small SVGs for grid cells ([`SparklineCell`], [`BulletCell`]) and
//! the [`Kpi`] card.

use super::{dom_id, layout::into_node};
use crate::grid::CellRenderer;
use crate::i18n::{NumberFormat, Text};
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use crate::value::{Currency, FieldKind, FieldType, Money, SupportsSum, Value};
use serde::Serialize;
use serde_json::{Value as Json, json};
use std::fmt::Write as _;

into_node!(Chart);
into_node!(Kpi);

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
    /// Bars of every series stacked on one another per label.
    StackedBar,
    Line,
    /// Lines with the space under them filled.
    Area,
    /// Bars, with the series added by [`Chart::line_series`] drawn as lines.
    Combo,
    /// Points placed by two numbers, from [`Chart::points`].
    Scatter,
    /// One series as parts of a whole.
    Donut,
    /// One value on a half circle between a minimum and a maximum.
    Gauge,
    /// One series as stages that narrow, such as visits, carts and orders.
    Funnel,
}

struct Series {
    name: Text,
    values: Vec<f64>,
    line: bool,
}

/// A chart with data from the screen.
///
/// ```
/// use zeyn::ui::Chart;
/// let c = Chart::bar("Sales").labels(["Jan", "Feb"]).series("2025", [120.0, 80.0]);
/// let mut out = String::new();
/// zeyn::Component::render(&c, &mut out, &zeyn::Render::english("s"));
/// assert!(out.contains(r#"aria-label="Jan, 2025: 120""#));
/// ```
pub struct Chart {
    kind: ChartKind,
    title: Text,
    labels: Vec<Text>,
    series: Vec<Series>,
    points: Vec<(Text, Vec<(f64, f64)>)>,
    decimals: u8,
    right_axis: bool,
    range: (f64, f64),
    target: Option<f64>,
}

fn finite(v: f64) -> f64 {
    if v.is_finite() { v } else { 0.0 }
}

/// A number in the locale's format with `decimals` decimals.
pub(crate) fn format_number(v: f64, decimals: u8, locale: &str) -> String {
    let f = NumberFormat::for_locale(locale);
    let text = format!("{:.*}", decimals as usize, v.abs());
    let (whole, frac) = text
        .split_once('.')
        .unwrap_or((&text, ""));
    let mut grouped = String::with_capacity(whole.len() + 4);
    for (i, c) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i) % 3 == 0 {
            grouped.push(f.group);
        }
        grouped.push(c);
    }
    let sign = if v < 0.0
        && text
            .bytes()
            .any(|b| (b'1'..=b'9').contains(&b))
    {
        "-"
    } else {
        ""
    };
    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}{}{frac}", f.decimal)
    }
}

/// A round top for an axis and the number of steps up to it: steps of 1, 2 or 5 times a power
/// of ten, at most five of them.
fn nice(max: f64) -> (f64, u32) {
    if max <= 0.0 || !max.is_finite() {
        return (1.0, 1);
    }
    let power = 10f64.powf((max / 5.0).log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * power)
        .find(|s| (max / s).ceil() <= 5.0)
        .unwrap_or(max);
    let ticks = (max / step).ceil().max(1.0) as u32;
    (step * ticks as f64, ticks)
}

/// Attributes of a mark the keyboard reaches: one tab stop for the chart, a name for screen
/// readers and the tooltip.
fn mark(first: &mut bool, label: &str) -> String {
    let tab = if std::mem::take(first) { "0" } else { "-1" };
    format!(r#" tabindex="{tab}" role="img" aria-label="{label}" data-zeyn-tip="{label}""#)
}

impl Chart {
    fn new(kind: ChartKind, title: impl Into<Text>) -> Self {
        Self {
            kind,
            title: title.into(),
            labels: Vec::new(),
            series: Vec::new(),
            points: Vec::new(),
            decimals: 0,
            right_axis: false,
            range: (0.0, 100.0),
            target: None,
        }
    }

    pub fn bar(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::Bar, title)
    }

    pub fn stacked_bar(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::StackedBar, title)
    }

    pub fn line(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::Line, title)
    }

    pub fn area(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::Area, title)
    }

    /// Bars from [`Chart::series`] with lines from [`Chart::line_series`] over them.
    ///
    /// ```ignore
    /// Chart::combo("Sales and margin").labels(months).series("Sales", sales)
    ///     .line_series("Margin %", margin).right_axis()
    /// ```
    pub fn combo(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::Combo, title)
    }

    /// Points from [`Chart::points`], placed by their x and y.
    pub fn scatter(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::Scatter, title)
    }

    pub fn donut(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::Donut, title)
    }

    /// One value between the ends of [`Chart::range`] (0 to 100 unless set). The value is the
    /// first one of the first series; [`Chart::value`] sets it.
    pub fn gauge(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::Gauge, title)
    }

    /// Stages of the first series, each with its share of the first stage.
    pub fn funnel(title: impl Into<Text>) -> Self {
        Self::new(ChartKind::Funnel, title)
    }

    /// Category names along the bottom, or the parts of a donut or a funnel.
    pub fn labels<I, T>(mut self, labels: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Text>,
    {
        self.labels = labels
            .into_iter()
            .map(Into::into)
            .collect();
        self
    }

    /// A named series, one value per label. A donut, gauge and funnel use the first series only.
    pub fn series(mut self, name: impl Into<Text>, values: impl IntoIterator<Item = f64>) -> Self {
        self.series.push(Series {
            name: name.into(),
            values: values.into_iter().map(finite).collect(),
            line: false,
        });
        self
    }

    /// A series drawn as a line over the bars of a [`Chart::combo`].
    pub fn line_series(
        mut self,
        name: impl Into<Text>,
        values: impl IntoIterator<Item = f64>,
    ) -> Self {
        self.series.push(Series {
            name: name.into(),
            values: values.into_iter().map(finite).collect(),
            line: true,
        });
        self
    }

    /// Gives the lines of a combo chart their own scale on the right.
    pub fn right_axis(mut self) -> Self {
        self.right_axis = true;
        self
    }

    /// A named group of `(x, y)` points for a scatter chart.
    pub fn points(
        mut self,
        name: impl Into<Text>,
        points: impl IntoIterator<Item = (f64, f64)>,
    ) -> Self {
        self.points.push((
            name.into(),
            points
                .into_iter()
                .map(|(x, y)| (finite(x), finite(y)))
                .collect(),
        ));
        self
    }

    /// The value of a gauge.
    pub fn value(self, value: f64) -> Self {
        let name = self.title.clone();
        let mut c = self;
        c.series = vec![Series {
            name,
            values: vec![finite(value)],
            line: false,
        }];
        c
    }

    /// The ends of a gauge.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        let (min, max) = (finite(min), finite(max));
        self.range = if max > min {
            (min, max)
        } else {
            (min, min + 1.0)
        };
        self
    }

    /// A target marked on a gauge.
    pub fn target(mut self, target: f64) -> Self {
        self.target = Some(finite(target));
        self
    }

    /// Decimals shown in labels and the table, 0 to 4.
    pub fn decimals(mut self, n: u8) -> Self {
        self.decimals = n.min(4);
        self
    }

    fn num(&self, v: f64, locale: &str) -> String {
        format_number(v, self.decimals, locale)
    }

    fn value_at(&self, s: usize, i: usize) -> f64 {
        self.series
            .get(s)
            .and_then(|x| x.values.get(i))
            .copied()
            .unwrap_or(0.0)
    }

    fn label(&self, r: &Render, i: usize) -> String {
        self.labels
            .get(i)
            .map(|l| r.text(l))
            .unwrap_or_default()
    }

    /// Width of one category band in the stretched plot of bar, line and area charts.
    fn band(&self) -> f64 {
        W / self.labels.len().max(1) as f64
    }

    /// A tick value, shortened from ten thousand on (`600K`, `1.2M`) by the user's language.
    fn tick(&self, v: f64, max: f64, r: &Render) -> String {
        let (div, key) = if max >= 1e9 {
            (1e9, "billion")
        } else if max >= 1e6 {
            (1e6, "million")
        } else if max >= 1e4 {
            (1e3, "thousand")
        } else {
            return self.num(v, r.locale);
        };
        let n = format_number(v / div, 1, r.locale);
        let n = n
            .strip_suffix("0")
            .and_then(|t| t.strip_suffix(['.', ',']))
            .unwrap_or(&n);
        r.i18n.text(
            r.locale,
            &Text::key(format!("zeyn.ui.chart.{key}")).arg("n", n.to_string()),
        )
    }

    /// Horizontal gridlines in the plot and the tick labels as HTML beside it, `0` to `max`.
    /// `side` is `start` or `end`.
    fn y_axis(
        &self,
        svg: &mut String,
        html: &mut String,
        r: &Render,
        (max, ticks): (f64, u32),
        side: &str,
        grid: bool,
    ) {
        let _ = write!(
            html,
            r#"<div class="z-chart-y z-chart-y-{side}" aria-hidden="true">"#
        );
        for k in 0..=ticks {
            let v = max * k as f64 / ticks as f64;
            let y = H - H * k as f64 / ticks as f64;
            if grid {
                let class = if k == 0 {
                    "z-chart-base"
                } else {
                    "z-chart-grid"
                };
                let _ = write!(
                    svg,
                    r#"<line class="{class}" x1="0" x2="{W}" y1="{y:.1}" y2="{y:.1}"/>"#
                );
            }
            let _ = write!(
                html,
                r#"<span style="top:{:.2}%">{}</span>"#,
                y / H * 100.0,
                escape(&self.tick(v, max, r))
            );
        }
        html.push_str("</div>");
    }

    /// Category labels as HTML under the plot, thinned so at most twelve show; every other one
    /// hides in narrow charts.
    fn x_labels(&self, html: &mut String, r: &Render) {
        let n = self.labels.len();
        let every = n.div_ceil(12).max(1);
        html.push_str(r#"<div class="z-chart-x" aria-hidden="true">"#);
        for (i, label) in self.labels.iter().enumerate() {
            if i % every != 0 {
                continue;
            }
            let thin = if (i / every) % 2 == 1 {
                r#" class="z-tick-thin""#
            } else {
                ""
            };
            let _ = write!(
                html,
                r#"<span{thin} style="left:{:.2}%">{}</span>"#,
                (i as f64 + 0.5) / n.max(1) as f64 * 100.0,
                r.text(label)
            );
        }
        html.push_str("</div>");
    }

    fn max_of(&self, line: bool) -> f64 {
        self.series
            .iter()
            .filter(|s| s.line == line)
            .flat_map(|s| s.values.iter().copied())
            .fold(0.0_f64, f64::max)
    }

    fn point_label(&self, r: &Render, s: usize, i: usize, v: f64) -> String {
        let name = self
            .series
            .get(s)
            .map(|x| r.text(&x.name))
            .unwrap_or_default();
        format!(
            "{}, {name}: {}",
            self.label(r, i),
            escape(&self.num(v, r.locale))
        )
    }

    fn bars(&self, svg: &mut String, html: &mut String, r: &Render, first: &mut bool) {
        let combo = self.kind == ChartKind::Combo;
        let bar_max = if combo && !self.right_axis {
            self.max_of(false)
                .max(self.max_of(true))
        } else {
            self.max_of(false)
        };
        let scale = nice(bar_max);
        self.y_axis(svg, html, r, scale, "start", true);
        let band = self.band();
        let bars: Vec<usize> = (0..self.series.len())
            .filter(|s| !self.series[*s].line)
            .collect();
        let width = band * 0.6 / bars.len().max(1) as f64;
        for i in 0..self.labels.len() {
            for (k, s) in bars.iter().enumerate() {
                let v = self.value_at(*s, i).max(0.0);
                let h = H * v / scale.0;
                let x = band * i as f64 + band * 0.2 + width * k as f64;
                let _ = write!(
                    svg,
                    r#"<rect class="z-chart-bar z-chart-mark z-series-{}" x="{x:.1}" y="{:.1}" width="{:.1}" height="{h:.1}" data-s="{s}" data-i="{i}"{}/>"#,
                    s % 8,
                    H - h,
                    (width - 1.0).max(1.0),
                    mark(first, &self.point_label(r, *s, i, v))
                );
            }
        }
        if combo {
            let line_scale = if self.right_axis {
                nice(self.max_of(true))
            } else {
                scale
            };
            if self.right_axis {
                self.y_axis(svg, html, r, line_scale, "end", false);
            }
            for s in (0..self.series.len()).filter(|s| self.series[*s].line) {
                self.polyline(svg, html, r, s, line_scale.0, false, first);
            }
        }
    }

    fn stacked(&self, svg: &mut String, html: &mut String, r: &Render, first: &mut bool) {
        let n = self.labels.len();
        let totals: Vec<f64> = (0..n)
            .map(|i| {
                (0..self.series.len())
                    .map(|s| self.value_at(s, i).max(0.0))
                    .sum()
            })
            .collect();
        let scale = nice(
            totals
                .iter()
                .copied()
                .fold(0.0, f64::max),
        );
        self.y_axis(svg, html, r, scale, "start", true);
        let band = self.band();
        for i in 0..n {
            let mut base = 0.0;
            for s in 0..self.series.len() {
                let v = self.value_at(s, i).max(0.0);
                let h = H * v / scale.0;
                let y = H - H * base / scale.0 - h;
                base += v;
                let _ = write!(
                    svg,
                    r#"<rect class="z-chart-bar z-chart-mark z-series-{}" x="{:.1}" y="{y:.1}" width="{:.1}" height="{h:.1}" data-s="{s}" data-i="{i}"{}/>"#,
                    s % 8,
                    band * i as f64 + band * 0.2,
                    band * 0.6,
                    mark(first, &self.point_label(r, s, i, v))
                );
            }
        }
    }

    /// A line (and its area) in the stretched plot. The points are HTML marks over it, so they
    /// stay round at any width; they show on hover and focus, the last one always.
    #[allow(
        clippy::too_many_arguments,
        reason = "the plot, the overlay, the render, the series, its scale, the area flag and the tab stop are all needed"
    )]
    fn polyline(
        &self,
        svg: &mut String,
        html: &mut String,
        r: &Render,
        s: usize,
        max: f64,
        fill: bool,
        first: &mut bool,
    ) {
        let band = self.band();
        let values = &self.series[s].values;
        let points: Vec<(f64, f64)> = (0..self.labels.len().min(values.len()))
            .map(|i| (band * (i as f64 + 0.5), H - H * values[i].max(0.0) / max))
            .collect();
        let path: Vec<String> = points
            .iter()
            .map(|(x, y)| format!("{x:.1},{y:.1}"))
            .collect();
        let dash = match s % 8 {
            6 => " z-dash-1",
            7 => " z-dash-2",
            _ => "",
        };
        if fill && let (Some((x0, _)), Some((x1, _))) = (points.first(), points.last()) {
            let _ = write!(
                svg,
                r#"<polygon class="z-chart-area z-series-{}" points="{x0:.1},{H} {} {x1:.1},{H}"/>"#,
                s % 8,
                path.join(" ")
            );
        }
        let _ = write!(
            svg,
            r#"<polyline class="z-chart-line z-series-{}{dash}" points="{}"/>"#,
            s % 8,
            path.join(" ")
        );
        let last = points.len().saturating_sub(1);
        for (i, (x, y)) in points.iter().enumerate() {
            let end = if i == last { " z-chart-pt-last" } else { "" };
            let _ = write!(
                html,
                r#"<span class="z-chart-pt z-chart-mark z-series-{}{end}" style="left:{:.2}%;top:{:.2}%" data-s="{s}" data-i="{i}"{}></span>"#,
                s % 8,
                x / W * 100.0,
                y / H * 100.0,
                mark(first, &self.point_label(r, s, i, values[i]))
            );
        }
    }

    fn lines(&self, svg: &mut String, html: &mut String, r: &Render, first: &mut bool) {
        let scale = nice(self.max_of(false));
        self.y_axis(svg, html, r, scale, "start", true);
        for s in 0..self.series.len() {
            self.polyline(
                svg,
                html,
                r,
                s,
                scale.0,
                self.kind == ChartKind::Area,
                first,
            );
        }
    }

    fn scatter_plot(&self, out: &mut String, r: &Render, first: &mut bool) {
        let all = || {
            self.points
                .iter()
                .flat_map(|(_, p)| p.iter())
        };
        let y_scale = nice(all().map(|p| p.1).fold(0.0, f64::max));
        let (x_max, x_ticks) = nice(all().map(|p| p.0).fold(0.0, f64::max));
        let plot_w = W - LEFT - RIGHT;
        let plot_h = H - TOP - BOTTOM;
        for k in 0..=y_scale.1 {
            let v = y_scale.0 * k as f64 / y_scale.1 as f64;
            let y = TOP + plot_h - plot_h * k as f64 / y_scale.1 as f64;
            let _ = write!(
                out,
                r#"<line class="z-chart-grid" x1="{LEFT}" x2="{}" y1="{y:.1}" y2="{y:.1}"/><text class="z-chart-axis" x="{}" y="{:.1}" text-anchor="end">{}</text>"#,
                W - RIGHT,
                LEFT - 8.0,
                y + 4.0,
                escape(&self.num(v, r.locale))
            );
        }
        for k in 0..=x_ticks {
            let v = x_max * k as f64 / x_ticks as f64;
            let _ = write!(
                out,
                r#"<text class="z-chart-axis" x="{:.1}" y="{}" text-anchor="middle">{}</text>"#,
                LEFT + plot_w * k as f64 / x_ticks as f64,
                H - 10.0,
                escape(&self.num(v, r.locale))
            );
        }
        for (s, (name, points)) in self.points.iter().enumerate() {
            for (i, (x, y)) in points.iter().enumerate() {
                let cx = LEFT + plot_w * x.max(0.0) / x_max;
                let cy = TOP + plot_h - plot_h * y.max(0.0) / y_scale.0;
                let label = format!(
                    "{}: {}, {}",
                    r.text(name),
                    escape(&self.num(*x, r.locale)),
                    escape(&self.num(*y, r.locale))
                );
                let _ = write!(
                    out,
                    r#"<circle class="z-chart-dot z-chart-mark z-series-{}" cx="{cx:.1}" cy="{cy:.1}" r="5" data-s="{s}" data-i="{i}"{}/>"#,
                    s % 8,
                    mark(first, &label)
                );
            }
        }
    }

    fn ring(&self, out: &mut String, r: &Render, first: &mut bool) {
        let values: Vec<f64> = self
            .series
            .first()
            .map(|s| {
                s.values
                    .iter()
                    .map(|x| x.max(0.0))
                    .collect()
            })
            .unwrap_or_default();
        let total: f64 = values.iter().sum();
        let (cx, cy, radius) = (W / 2.0, H / 2.0, 90.0);
        let circumference = 2.0 * std::f64::consts::PI * radius;
        let _ = write!(
            out,
            r#"<circle class="z-chart-track" cx="{cx}" cy="{cy}" r="{radius}"/>"#
        );
        let mut offset = 0.0;
        for (i, v) in values.iter().enumerate() {
            if total <= 0.0 || *v <= 0.0 {
                continue;
            }
            let len = circumference * v / total;
            let label = format!(
                "{}: {} ({:.0}%)",
                self.label(r, i),
                escape(&self.num(*v, r.locale)),
                v * 100.0 / total
            );
            let _ = write!(
                out,
                r#"<circle class="z-chart-slice z-chart-mark z-series-{}" cx="{cx}" cy="{cy}" r="{radius}" stroke-dasharray="{len:.2} {:.2}" stroke-dashoffset="{:.2}" transform="rotate(-90 {cx} {cy})" data-s="0" data-i="{i}"{}/>"#,
                i % 8,
                circumference - len,
                -offset,
                mark(first, &label)
            );
            offset += len;
        }
        let _ = write!(
            out,
            r#"<text class="z-chart-total" x="{cx}" y="{}" text-anchor="middle">{}</text>"#,
            cy + 8.0,
            escape(&self.num(total, r.locale))
        );
    }

    fn gauge_value(&self) -> f64 {
        self.value_at(0, 0)
    }

    fn dial(&self, out: &mut String, r: &Render, first: &mut bool) {
        let (min, max) = self.range;
        let v = self.gauge_value();
        let share = ((v - min) / (max - min)).clamp(0.0, 1.0);
        let (cx, cy, radius) = (W / 2.0, H - 40.0, 160.0);
        let arc = format!(
            "M {:.1} {cy:.1} A {radius} {radius} 0 0 1 {:.1} {cy:.1}",
            cx - radius,
            cx + radius
        );
        let _ = write!(
            out,
            r#"<path class="z-chart-gauge-track" d="{arc}" pathLength="100"/>"#
        );
        let label = r.text(
            &Text::key("zeyn.ui.chart.gauge_point")
                .arg("value", self.num(v, r.locale))
                .arg("max", self.num(max, r.locale)),
        );
        let _ = write!(
            out,
            r#"<path class="z-chart-gauge-value z-chart-mark z-series-0" d="{arc}" pathLength="100" stroke-dasharray="{:.2} 100" data-s="0" data-i="0"{}/>"#,
            share * 100.0,
            mark(first, &label)
        );
        if let Some(t) = self.target {
            let a = std::f64::consts::PI * (1.0 - ((t - min) / (max - min)).clamp(0.0, 1.0));
            let (c, s) = (a.cos(), -a.sin());
            let _ = write!(
                out,
                r#"<line class="z-chart-target" x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}"/>"#,
                cx + c * (radius - 26.0),
                cy + s * (radius - 26.0),
                cx + c * (radius + 26.0),
                cy + s * (radius + 26.0)
            );
        }
        let _ = write!(
            out,
            r#"<text class="z-chart-total" x="{cx}" y="{:.1}" text-anchor="middle">{}</text><text class="z-chart-axis" x="{:.1}" y="{:.1}" text-anchor="middle">{}</text><text class="z-chart-axis" x="{:.1}" y="{:.1}" text-anchor="middle">{}</text>"#,
            cy - 10.0,
            escape(&self.num(v, r.locale)),
            cx - radius,
            cy + 24.0,
            escape(&self.num(min, r.locale)),
            cx + radius,
            cy + 24.0,
            escape(&self.num(max, r.locale)),
        );
    }

    fn stages(&self, out: &mut String, r: &Render, first: &mut bool) {
        let values: Vec<f64> = self
            .series
            .first()
            .map(|s| {
                s.values
                    .iter()
                    .map(|x| x.max(0.0))
                    .collect()
            })
            .unwrap_or_default();
        let top = values
            .iter()
            .copied()
            .fold(0.0, f64::max);
        let n = self.labels.len().max(1) as f64;
        let row = (H - TOP * 2.0) / n;
        let full = W - 2.0 * 180.0;
        let lead = values.first().copied().unwrap_or(0.0);
        for (i, label) in self.labels.iter().enumerate() {
            let v = values.get(i).copied().unwrap_or(0.0);
            let w = if top > 0.0 {
                (full * v / top).max(2.0)
            } else {
                2.0
            };
            let y = TOP + row * i as f64;
            let pct = if lead > 0.0 { v * 100.0 / lead } else { 0.0 };
            let text = format!(
                "{}: {} ({:.0}%)",
                r.text(label),
                escape(&self.num(v, r.locale)),
                pct
            );
            let _ = write!(
                out,
                r#"<rect class="z-chart-bar z-chart-mark z-series-{}" x="{:.1}" y="{:.1}" width="{w:.1}" height="{:.1}" rx="3" data-s="0" data-i="{i}"{}/><text class="z-chart-axis" x="{:.1}" y="{:.1}" text-anchor="end">{}</text><text class="z-chart-axis" x="{:.1}" y="{:.1}">{} ({:.0}%)</text>"#,
                i % 8,
                (W - w) / 2.0,
                y + 2.0,
                (row - 4.0).max(2.0),
                mark(first, &text),
                (W - w) / 2.0 - 8.0,
                y + row / 2.0 + 4.0,
                r.text(label),
                (W + w) / 2.0 + 8.0,
                y + row / 2.0 + 4.0,
                escape(&self.num(v, r.locale)),
                pct
            );
        }
    }

    /// A sentence describing the data, for screen readers.
    fn summary(&self, r: &Render) -> String {
        let name = match self.kind {
            ChartKind::Bar => "bar",
            ChartKind::StackedBar => "stacked_bar",
            ChartKind::Line => "line",
            ChartKind::Area => "area",
            ChartKind::Combo => "combo",
            ChartKind::Scatter => "scatter",
            ChartKind::Donut => "donut",
            ChartKind::Gauge => "gauge",
            ChartKind::Funnel => "funnel",
        };
        let key = Text::key(format!("zeyn.ui.chart.summary_{name}"));
        match self.kind {
            ChartKind::Gauge => {
                let v = self.gauge_value();
                let target = self
                    .target
                    .map(|t| self.num(t, r.locale))
                    .unwrap_or_default();
                return r.text(
                    &key.arg("value", self.num(v, r.locale))
                        .arg("min", self.num(self.range.0, r.locale))
                        .arg("max", self.num(self.range.1, r.locale))
                        .arg("target", target),
                );
            }
            ChartKind::Scatter => {
                let count: usize = self
                    .points
                    .iter()
                    .map(|(_, p)| p.len())
                    .sum();
                return r.text(
                    &key.arg("items", count.to_string())
                        .arg("series", self.points.len().to_string()),
                );
            }
            _ => {}
        }
        let mut best: Option<(f64, String)> = None;
        let stacked = self.kind == ChartKind::StackedBar;
        if stacked {
            for i in 0..self.labels.len() {
                let total: f64 = (0..self.series.len())
                    .map(|s| self.value_at(s, i).max(0.0))
                    .sum();
                if best
                    .as_ref()
                    .is_none_or(|(b, _)| total > *b)
                {
                    best = Some((
                        total,
                        self.labels
                            .get(i)
                            .map(|l| r.i18n.text(r.locale, l))
                            .unwrap_or_default(),
                    ));
                }
            }
        } else {
            for s in &self.series {
                for (i, v) in s.values.iter().enumerate() {
                    if best.as_ref().is_none_or(|(b, _)| v > b) {
                        let label = self
                            .labels
                            .get(i)
                            .map(|l| r.i18n.text(r.locale, l))
                            .unwrap_or_default();
                        let who = if self.series.len() > 1 {
                            format!("{label}, {}", r.i18n.text(r.locale, &s.name))
                        } else {
                            label
                        };
                        best = Some((*v, who));
                    }
                }
            }
        }
        let text = key
            .arg("items", self.labels.len().to_string())
            .arg("series", self.series.len().to_string())
            .arg(
                "max",
                best.as_ref()
                    .map(|(_, w)| w.clone())
                    .unwrap_or_default(),
            )
            .arg(
                "value",
                best.map(|(v, _)| self.num(v, r.locale))
                    .unwrap_or_default(),
            );
        r.text(&text)
    }

    fn legend(&self, out: &mut String, r: &Render) {
        let names: Vec<(usize, &Text, bool)> = match self.kind {
            ChartKind::Donut | ChartKind::Funnel => self
                .labels
                .iter()
                .enumerate()
                .map(|(i, l)| (i, l, false))
                .collect(),
            ChartKind::Gauge => Vec::new(),
            ChartKind::Scatter => self
                .points
                .iter()
                .enumerate()
                .map(|(i, (n, _))| (i, n, false))
                .collect(),
            _ => self
                .series
                .iter()
                .enumerate()
                .map(|(i, s)| (i, &s.name, s.line))
                .collect(),
        };
        if names.is_empty() {
            return;
        }
        out.push_str(r#"<ul class="z-chart-legend">"#);
        for (i, name, line) in names {
            let shape = if line { " z-chart-swatch-line" } else { "" };
            let _ = write!(
                out,
                r#"<li><span class="z-chart-swatch{shape} z-series-{}" aria-hidden="true"></span>{}</li>"#,
                i % 8,
                r.text(name)
            );
        }
        out.push_str("</ul>");
    }

    fn table(&self, out: &mut String, r: &Render) {
        let _ = write!(
            out,
            r#"<details class="z-chart-data"><summary>{}</summary><table class="z-chart-table"><caption class="z-sr-only">{}</caption><thead><tr>"#,
            r.text(&Text::key("zeyn.ui.chart.data")),
            r.text(&Text::key("zeyn.ui.chart.table").arg("title", self.title.clone()))
        );
        let th = |out: &mut String, t: &str| {
            let _ = write!(out, r#"<th scope="col" class="z-num">{t}</th>"#);
        };
        let num = |v: f64| escape(&self.num(v, r.locale));
        match self.kind {
            ChartKind::Scatter => {
                out.push_str(r#"<th scope="col"></th>"#);
                th(out, "x");
                th(out, "y");
                out.push_str("</tr></thead><tbody>");
                for (name, points) in &self.points {
                    for (x, y) in points {
                        let _ = write!(
                            out,
                            r#"<tr><th scope="row">{}</th><td class="z-num">{}</td><td class="z-num">{}</td></tr>"#,
                            r.text(name),
                            num(*x),
                            num(*y)
                        );
                    }
                }
            }
            ChartKind::Gauge => {
                for k in ["value", "min", "max", "target"] {
                    th(out, &r.text(&Text::key(format!("zeyn.ui.chart.{k}"))));
                }
                let _ = write!(
                    out,
                    r#"</tr></thead><tbody><tr><td class="z-num">{}</td><td class="z-num">{}</td><td class="z-num">{}</td><td class="z-num">{}</td></tr>"#,
                    num(self.gauge_value()),
                    num(self.range.0),
                    num(self.range.1),
                    self.target.map(num).unwrap_or_default()
                );
            }
            _ => {
                out.push_str(r#"<th scope="col"></th>"#);
                for s in &self.series {
                    th(out, &r.text(&s.name));
                }
                out.push_str("</tr></thead><tbody>");
                for (i, label) in self.labels.iter().enumerate() {
                    let _ = write!(out, r#"<tr><th scope="row">{}</th>"#, r.text(label));
                    for s in 0..self.series.len() {
                        let _ = write!(
                            out,
                            r#"<td class="z-num">{}</td>"#,
                            num(self.value_at(s, i))
                        );
                    }
                    out.push_str("</tr>");
                }
            }
        }
        out.push_str("</tbody></table></details>");
    }
}

impl Component for Chart {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "chart");
        let title = r.text(&self.title);
        let _ = write!(
            out,
            r#"<figure class="z-chart" data-zeyn-chart><figcaption class="z-chart-title" id="{id}-title">{title}</figcaption>"#
        );
        if self.series.len() > 1
            || !matches!(
                self.kind,
                ChartKind::Bar
                    | ChartKind::StackedBar
                    | ChartKind::Line
                    | ChartKind::Area
                    | ChartKind::Combo
            )
        {
            self.legend(out, r);
        }
        let mut first = true;
        let stretched = matches!(
            self.kind,
            ChartKind::Bar
                | ChartKind::Combo
                | ChartKind::StackedBar
                | ChartKind::Line
                | ChartKind::Area
        );
        if stretched {
            let mut svg = String::new();
            let mut html = String::new();
            match self.kind {
                ChartKind::StackedBar => self.stacked(&mut svg, &mut html, r, &mut first),
                ChartKind::Line | ChartKind::Area => self.lines(&mut svg, &mut html, r, &mut first),
                _ => self.bars(&mut svg, &mut html, r, &mut first),
            }
            let (ticks, marks) = match html.find("<span class=\"z-chart-pt") {
                Some(at) => html.split_at(at),
                None => (html.as_str(), ""),
            };
            let right = if self.kind == ChartKind::Combo && self.right_axis {
                " z-chart-two-axes"
            } else {
                ""
            };
            let _ = write!(
                out,
                r#"<div class="z-chart-plot{right}" role="group" aria-roledescription="{}" aria-labelledby="{id}-title {id}-desc"><p class="z-sr-only" id="{id}-desc">{}</p>{ticks}<div class="z-chart-canvas"><svg class="z-chart-svg" viewBox="0 0 {W} {H}" preserveAspectRatio="none" focusable="false">{svg}</svg>{marks}<span class="z-chart-cross" hidden></span></div>"#,
                r.text(&Text::key("zeyn.ui.chart.role")),
                self.summary(r)
            );
            self.x_labels(out, r);
            out.push_str("</div>");
        } else {
            let round = if matches!(self.kind, ChartKind::Donut | ChartKind::Gauge) {
                " z-chart-round"
            } else {
                ""
            };
            let _ = write!(
                out,
                r#"<svg class="z-chart-svg{round}" viewBox="0 0 {W} {H}" role="group" aria-roledescription="{}" aria-labelledby="{id}-title {id}-desc" focusable="false"><desc id="{id}-desc">{}</desc>"#,
                r.text(&Text::key("zeyn.ui.chart.role")),
                self.summary(r)
            );
            match self.kind {
                ChartKind::Scatter => self.scatter_plot(out, r, &mut first),
                ChartKind::Donut => self.ring(out, r, &mut first),
                ChartKind::Gauge => self.dial(out, r, &mut first),
                _ => self.stages(out, r, &mut first),
            }
            out.push_str("</svg>");
        }
        out.push_str(r#"<div class="z-chart-tip" aria-hidden="true" hidden></div>"#);
        self.table(out, r);
        out.push_str("</figure>");
    }

    fn schema(&self) -> Json {
        let series: Vec<Json> = self
            .series
            .iter()
            .map(|s| json!({ "name": s.name, "values": s.values, "line": s.line }))
            .collect();
        let points: Vec<Json> = self
            .points
            .iter()
            .map(|(n, p)| json!({ "name": n, "points": p }))
            .collect();
        json!({
            "type": "chart",
            "kind": self.kind,
            "title": self.title,
            "labels": self.labels,
            "series": series,
            "points": points,
            "range": [self.range.0, self.range.1],
            "target": self.target,
        })
    }
}

/// A series of numbers kept in one field, such as the last weeks of sales of a product, for
/// sparkline cells. Stored as JSON.
impl FieldType for Vec<f64> {
    const KIND: FieldKind = FieldKind::Custom { editor: "series" };

    fn to_value(&self) -> Value {
        let parts: Vec<String> = self
            .iter()
            .map(|v| v.to_string())
            .collect();
        Value::Text(parts.join(" "))
    }

    fn __is_empty(&self) -> bool {
        self.is_empty()
    }
}

/// A sparkline: a line of `values` in a small box, with the first, last, lowest and highest
/// value as its name. Cheap enough for a grid cell on every row.
/// A shown value with the currency symbol or unit around the digits set smaller and muted:
/// `$` in `$574,672`, `₺` in `10,50 ₺`.
fn value_parts(shown: &str) -> String {
    let start = shown
        .find(|c: char| c.is_ascii_digit() || c == '-' || c == '\u{2212}')
        .unwrap_or(0);
    let end = shown
        .rfind(|c: char| c.is_ascii_digit() || c == '%')
        .map_or(shown.len(), |i| i + 1)
        .max(start);
    let part = |s: &str| {
        let core = s.trim();
        if core.is_empty() {
            return escape(s);
        }
        let at = s.find(core).unwrap_or(0);
        format!(
            r#"{}<span class="z-kpi-unit">{}</span>{}"#,
            escape(&s[..at]),
            escape(core),
            escape(&s[at + core.len()..])
        )
    };
    format!(
        "{}{}{}",
        part(&shown[..start]),
        escape(&shown[start..end]),
        part(&shown[end..])
    )
}

/// The trend of a KPI card: a full width line in the delta's tone over an area that fades to
/// the bottom, drawn without axes. The SVG stretches with the card (`preserveAspectRatio`
/// `none`, strokes that do not scale); the end point is an HTML dot so it stays round.
fn kpi_trend(out: &mut String, values: &[f64], r: &Render, decimals: u8, id: &str) {
    const W: f64 = 100.0;
    const H: f64 = 40.0;
    let n = values.len();
    let (lo, hi) = values
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let span = if hi > lo { hi - lo } else { 1.0 };
    let point = |i: usize, v: f64| {
        let x = if n > 1 {
            W * i as f64 / (n - 1) as f64
        } else {
            W
        };
        (x, 2.0 + (H - 4.0) * (1.0 - (v - lo) / span))
    };
    let f = |v: f64| format_number(v, decimals, r.locale);
    let label = r.text(
        &Text::key("zeyn.ui.chart.sparkline")
            .arg("n", n.to_string())
            .arg("first", f(values[0]))
            .arg("last", f(values[n - 1]))
            .arg("low", f(lo))
            .arg("high", f(hi)),
    );
    let mut line = String::new();
    for (i, v) in values.iter().enumerate() {
        let (x, y) = point(i, *v);
        let _ = write!(line, "{}{x:.1},{y:.1}", if i > 0 { " " } else { "" });
    }
    let (_, end_y) = point(n - 1, values[n - 1]);
    let _ = write!(
        out,
        r#"<div class="z-kpi-spark-box"><svg class="z-kpi-spark" viewBox="0 0 {W} {H}" preserveAspectRatio="none" role="img" aria-label="{label}"><defs><linearGradient id="{id}-fade" x1="0" y1="0" x2="0" y2="1"><stop offset="0"/><stop offset="1"/></linearGradient></defs><polygon class="z-kpi-spark-area" fill="url(#{id}-fade)" points="0,{H} {line} {W},{H}"/><polyline points="{line}"/></svg><span class="z-kpi-spark-end" style="top:{:.1}%" aria-hidden="true"></span></div>"#,
        end_y / H * 100.0
    );
}

pub(crate) fn sparkline(out: &mut String, values: &[f64], r: &Render, decimals: u8) {
    const SW: f64 = 80.0;
    const SH: f64 = 20.0;
    let values: Vec<f64> = values
        .iter()
        .copied()
        .map(finite)
        .collect();
    if values.is_empty() {
        return;
    }
    let (lo, hi) = values
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let span = if hi > lo { hi - lo } else { 1.0 };
    let n = values.len();
    let dx = if n > 1 {
        (SW - 4.0) / (n - 1) as f64
    } else {
        0.0
    };
    let f = |v: f64| format_number(v, decimals, r.locale);
    let label = r.text(
        &Text::key("zeyn.ui.chart.sparkline")
            .arg("n", n.to_string())
            .arg("first", f(values[0]))
            .arg("last", f(values[n - 1]))
            .arg("low", f(lo))
            .arg("high", f(hi)),
    );
    let _ = write!(
        out,
        r#"<svg class="z-spark" viewBox="0 0 {SW} {SH}" role="img" aria-label="{label}"><polyline points=""#
    );
    let mut last = (0.0, 0.0);
    for (i, v) in values.iter().enumerate() {
        let (x, y) = (2.0 + dx * i as f64, SH - 2.0 - (SH - 4.0) * (v - lo) / span);
        let _ = write!(out, "{}{x:.1},{y:.1}", if i > 0 { " " } else { "" });
        last = (x, y);
    }
    let _ = write!(
        out,
        r#""/><circle cx="{:.1}" cy="{:.1}" r="2"/></svg>"#,
        last.0, last.1
    );
}

/// Draws a `Vec<f64>` column as a sparkline.
///
/// ```ignore
/// DataGrid::of(search_products).column(Product::trend).cell(SparklineCell::new())
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct SparklineCell {
    decimals: u8,
}

impl SparklineCell {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decimals of the numbers in the sparkline's name, 0 to 4.
    pub fn decimals(mut self, n: u8) -> Self {
        self.decimals = n.min(4);
        self
    }
}

impl CellRenderer<Vec<f64>> for SparklineCell {
    fn render(&self, value: &Vec<f64>, r: &Render) -> String {
        let mut out = String::with_capacity(64 + value.len() * 12);
        sparkline(&mut out, value, r, self.decimals);
        out
    }
}

/// Draws a number column as a bullet: a bar for the value against a target mark, with the
/// number beside it. Compiles only on numeric and money columns; money is compared in major
/// units.
///
/// ```ignore
/// DataGrid::of(search_reps).column(Rep::sales).cell(BulletCell::new(80_000.0, 100_000.0))
/// ```
#[derive(Clone, Copy, Debug)]
pub struct BulletCell {
    target: f64,
    max: f64,
}

impl BulletCell {
    /// A bullet up to `max` with a mark at `target`.
    pub fn new(target: f64, max: f64) -> Self {
        Self {
            target: finite(target),
            max: if finite(max) > 0.0 { max } else { 1.0 },
        }
    }
}

impl<T: FieldType + SupportsSum> CellRenderer<T> for BulletCell {
    fn render(&self, value: &T, r: &Render) -> String {
        let v = value.to_value();
        let (amount, plain) = match &v {
            Value::Int(n) => (*n as f64, format_number(*n as f64, 0, r.locale)),
            Value::Money(m) => (m.minor as f64 / 100.0, m.format(r.locale)),
            _ => return String::new(),
        };
        let pct = |x: f64| (x / self.max * 100.0).clamp(0.0, 100.0);
        let shown = escape(&plain);
        let label = r.text(
            &Text::key("zeyn.ui.chart.bullet")
                .arg("value", plain)
                .arg("target", format_number(self.target, 0, r.locale)),
        );
        format!(
            r#"<span class="z-bullet"><svg class="z-bullet-svg" viewBox="0 0 100 12" preserveAspectRatio="none" role="img" aria-label="{label}"><rect class="z-bullet-track" width="100" height="12"/><rect class="z-bullet-bar" y="3" width="{:.1}" height="6"/><rect class="z-bullet-target" x="{:.1}" y="0" width="1.5" height="12"/></svg><span class="z-bullet-text">{shown}</span></span>"#,
            pct(amount),
            (pct(self.target) - 0.75).max(0.0)
        )
    }
}

/// Which way a change of a [`Kpi`] is good.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Better {
    #[default]
    Higher,
    Lower,
}

/// A key figure: a value, its change against the previous period with a direction, a target and
/// a sparkline of recent values.
///
/// ```
/// use zeyn::ui::Kpi;
/// let k = Kpi::new("Revenue", 1234.5).decimals(1).delta(12.5).target(1500.0).trend([1.0, 3.0, 2.0]);
/// let mut out = String::new();
/// zeyn::Component::render(&k, &mut out, &zeyn::Render::english("s"));
/// assert!(out.contains("1,234.5") && out.contains("Up 12.5%"));
/// ```
///
/// Kpis in one [`crate::ui::Row`] or [`crate::ui::AutoGrid`] are as tall as the tallest, with or
/// without a trend. A row of four fills the width on a desktop and wraps to two and then one per
/// line as it narrows.
pub struct Kpi {
    title: Text,
    value: f64,
    currency: Option<Currency>,
    decimals: u8,
    unit: Option<Text>,
    delta: Option<f64>,
    better: Better,
    target: Option<f64>,
    trend: Vec<f64>,
}

impl Kpi {
    pub fn new(title: impl Into<Text>, value: f64) -> Self {
        Self {
            title: title.into(),
            value: finite(value),
            currency: None,
            decimals: 0,
            unit: None,
            delta: None,
            better: Better::Higher,
            target: None,
            trend: Vec::new(),
        }
    }

    /// An amount of money, shown with its currency in the user's locale; the target is read in
    /// the same currency.
    ///
    /// ```
    /// use zeyn::ui::Kpi;
    /// let k = Kpi::money("Revenue", zeyn::Money::tl(18_425_050)).delta(4.0);
    /// let mut out = String::new();
    /// zeyn::Component::render(&k, &mut out, &zeyn::Render::english("s"));
    /// assert!(out.contains(r#"<span class="z-kpi-unit">₺</span>184,250.50"#));
    /// ```
    pub fn money(title: impl Into<Text>, amount: Money) -> Self {
        Self {
            currency: Some(amount.currency),
            decimals: 2,
            ..Self::new(title, amount.minor as f64 / 100.0)
        }
    }

    /// The value or the target as shown: money with its currency, other numbers with the
    /// decimals asked for.
    fn shown(&self, v: f64, locale: &str) -> String {
        match self.currency {
            Some(c) => Money::new((v * 100.0).round() as i64, c).format(locale),
            None => format_number(v, self.decimals, locale),
        }
    }

    /// The value shortened from seven digits on, such as `$1.24M`, or `None` when it is shown
    /// in full.
    fn abbreviated(&self, r: &Render) -> Option<String> {
        let v = self.value.abs();
        let (div, key) = if v >= 1e9 {
            (1e9, "zeyn.ui.kpi.billion")
        } else if v >= 1e6 {
            (1e6, "zeyn.ui.kpi.million")
        } else {
            return None;
        };
        let short = r.text(&Text::key(key).arg("n", format_number(self.value / div, 2, r.locale)));
        let full = self.shown(self.value, r.locale);
        let start = full
            .find(|c: char| c.is_ascii_digit() || c == '-' || c == '\u{2212}')
            .unwrap_or(0);
        let end = full
            .rfind(|c: char| c.is_ascii_digit())
            .map_or(full.len(), |i| i + 1);
        Some(format!("{}{short}{}", &full[..start], &full[end..]))
    }

    /// Decimals of the value and the target, 0 to 4.
    pub fn decimals(mut self, n: u8) -> Self {
        self.decimals = n.min(4);
        self
    }

    /// A unit shown after the value, such as `kg` or `%`.
    pub fn unit(mut self, unit: impl Into<Text>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    /// Change against the previous period, in percent.
    pub fn delta(mut self, percent: f64) -> Self {
        self.delta = Some(finite(percent));
        self
    }

    /// Lower values are better (costs, wait times): a fall is shown as good.
    pub fn lower_is_better(mut self) -> Self {
        self.better = Better::Lower;
        self
    }

    pub fn target(mut self, target: f64) -> Self {
        self.target = Some(finite(target));
        self
    }

    /// Recent values, oldest first, drawn as a sparkline.
    pub fn trend(mut self, values: impl IntoIterator<Item = f64>) -> Self {
        self.trend = values.into_iter().map(finite).collect();
        self
    }
}

impl Component for Kpi {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "kpi");
        let unit = self
            .unit
            .as_ref()
            .map(|u| format!(r#" <span class="z-kpi-unit">{}</span>"#, r.text(u)))
            .unwrap_or_default();
        let full = self.shown(self.value, r.locale);
        let _ = write!(
            out,
            r#"<div class="z-kpi" role="group" aria-labelledby="{id}-title"><p class="z-kpi-title" id="{id}-title">{}</p>"#,
            r.text(&self.title)
        );
        match self.abbreviated(r) {
            Some(short) => {
                let _ = write!(
                    out,
                    r#"<p class="z-kpi-value" title="{full}"><span aria-hidden="true">{}{unit}</span><span class="z-sr-only">{full}</span></p>"#,
                    value_parts(&short),
                    full = escape(&full)
                );
            }
            None => {
                let _ = write!(
                    out,
                    r#"<p class="z-kpi-value">{}{unit}</p>"#,
                    value_parts(&full)
                );
            }
        }
        let tone = match self.delta {
            Some(d) if d.abs() >= 0.5 => {
                if (d > 0.0) == (self.better == Better::Higher) {
                    "good"
                } else {
                    "bad"
                }
            }
            _ => "neutral",
        };
        if let Some(d) = self.delta {
            let dir = if d > 0.0 {
                "up"
            } else if d < 0.0 {
                "down"
            } else {
                "flat"
            };
            let icon = match dir {
                "up" => crate::screen::Icon::TrendUp.html(),
                "down" => crate::screen::Icon::TrendDown.html(),
                _ => String::new(),
            };
            let sign = match dir {
                "up" => "+",
                "down" => "\u{2212}",
                _ => "",
            };
            let _ = write!(
                out,
                r#"<p class="z-kpi-delta"><span class="z-kpi-chip z-kpi-{tone}" aria-hidden="true">{icon}{sign}{}%</span> <span class="z-kpi-compare" aria-hidden="true">{}</span><span class="z-sr-only">{}</span></p>"#,
                format_number(d.abs(), 1, r.locale),
                r.text(&Text::key("zeyn.ui.kpi.compare")),
                r.text(
                    &Text::key(format!("zeyn.ui.kpi.{dir}"))
                        .arg("pct", format_number(d.abs(), 1, r.locale))
                )
            );
        }
        if let Some(t) = self.target {
            let share = if t != 0.0 {
                self.value / t * 100.0
            } else {
                0.0
            };
            let _ = write!(
                out,
                r#"<p class="z-kpi-target">{}</p>"#,
                r.text(
                    &Text::key("zeyn.ui.kpi.target")
                        .arg("target", self.shown(t, r.locale))
                        .arg("pct", format_number(share, 0, r.locale))
                )
            );
        }
        if !self.trend.is_empty() {
            let _ = write!(out, r#"<div class="z-kpi-trend z-kpi-{tone}">"#);
            kpi_trend(out, &self.trend, r, self.decimals, &id);
            out.push_str("</div>");
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "kpi",
            "title": self.title,
            "value": self.value,
            "currency": self.currency,
            "delta": self.delta,
            "better": self.better,
            "target": self.target,
            "trend": self.trend,
        })
    }
}
