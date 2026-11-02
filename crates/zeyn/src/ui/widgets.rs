//! Display components: alerts, badges, empty states, skeletons, progress bars, description lists,
//! breadcrumbs, timelines, and buttons that run an action after a confirmation.

use super::{dom_id, layout::into_node};
use crate::action::{Action, ActionMeta, Result};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::i18n::Text;
use crate::model::{Field, Model};
use crate::render::{Render, attr, escape, slug};
use crate::screen::{Component, Effect, FormSource, Node, ScreenDef, UiCtx};
use crate::value::{FieldKind, FieldType, Value};
use serde_json::{Map, Value as Json, json};
use std::sync::Arc;

into_node!(Alert, Badge, EmptyState, Skeleton, Progress, Breadcrumb, Timeline);

pub use crate::grid::Tone;

/// Spoken before a message, so the tone is not given by color alone.
fn spoken(tone: Tone, r: &Render) -> String {
    r.text(&Text::key(format!("zeyn.ui.tone.{}", tone.code())))
}

/// A callout: a message in a tinted box with an icon, optional title and close button.
pub struct Alert {
    tone: Tone,
    title: Option<Text>,
    text: Text,
    dismissible: bool,
}

impl Alert {
    pub fn new(tone: Tone, text: impl Into<Text>) -> Self {
        Self { tone, title: None, text: text.into(), dismissible: false }
    }

    pub fn info(text: impl Into<Text>) -> Self {
        Self::new(Tone::Info, text)
    }

    pub fn success(text: impl Into<Text>) -> Self {
        Self::new(Tone::Success, text)
    }

    pub fn warning(text: impl Into<Text>) -> Self {
        Self::new(Tone::Warning, text)
    }

    pub fn danger(text: impl Into<Text>) -> Self {
        Self::new(Tone::Danger, text)
    }

    pub fn title(mut self, title: impl Into<Text>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Adds a close button. Closing only hides the message on this page.
    pub fn dismissible(mut self) -> Self {
        self.dismissible = true;
        self
    }
}

impl Component for Alert {
    fn render(&self, out: &mut String, r: &Render) {
        let tone = self.tone.code();
        out.push_str(&format!(r#"<div class="z-callout z-tone-{tone}" data-testid="{}.alert.{tone}"><span class="z-callout-icon" aria-hidden="true"></span><div class="z-callout-body">"#, r.scope));
        let spoken = format!(r#"<span class="z-sr-only">{}: </span>"#, spoken(self.tone, r));
        match &self.title {
            Some(t) => out.push_str(&format!(
                r#"<p class="z-callout-title">{spoken}{}</p><p>{}</p>"#,
                r.text(t),
                r.text(&self.text)
            )),
            None => out.push_str(&format!(r#"<p>{spoken}{}</p>"#, r.text(&self.text))),
        }
        out.push_str("</div>");
        if self.dismissible {
            out.push_str(&format!(
                r#"<button type="button" class="z-icon-button" data-zeyn-dismiss aria-label="{}"><span aria-hidden="true">&#215;</span></button>"#,
                r.text(&Text::key("zeyn.ui.dismiss"))
            ));
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "alert", "tone": self.tone, "title": self.title, "text": self.text, "dismissible": self.dismissible })
    }
}

/// A short status label, such as "Paid" or "Overdue".
pub struct Badge {
    text: Text,
    tone: Tone,
}

impl Badge {
    pub fn new(text: impl Into<Text>) -> Self {
        Self { text: text.into(), tone: Tone::Neutral }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// Badge markup, for use inside other components.
    pub fn html(&self, r: &Render) -> String {
        format!(r#"<span class="z-badge z-tone-{}">{}</span>"#, self.tone.code(), r.text(&self.text))
    }
}

impl Component for Badge {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&self.html(r));
    }

    fn schema(&self) -> Json {
        json!({ "type": "badge", "text": self.text, "tone": self.tone })
    }
}

/// What a list or area shows when it has nothing yet, with an optional button.
pub struct EmptyState {
    title: Text,
    text: Option<Text>,
    button: Option<(Text, Effect)>,
}

impl EmptyState {
    pub fn new(title: impl Into<Text>) -> Self {
        Self { title: title.into(), text: None, button: None }
    }

    pub fn text(mut self, text: impl Into<Text>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// A button that opens a screen as a window, such as "New product".
    pub fn opens<S: ScreenDef>(mut self, label: impl Into<Text>, screen: S, params: S::Params) -> Self {
        self.button = Some((label.into(), crate::screen::Window::open(screen, params)));
        self
    }
}

impl Component for EmptyState {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(r#"<div class="z-empty"><span class="z-empty-icon" aria-hidden="true"></span><p class="z-empty-title">{}</p>"#, r.text(&self.title)));
        if let Some(t) = &self.text {
            out.push_str(&format!(r#"<p class="z-empty-text">{}</p>"#, r.text(t)));
        }
        if let Some((label, effect)) = &self.button {
            out.push_str(&format!(
                r#"<button type="button" class="z-button z-primary" data-zeyn-effect="{}" data-testid="{}.empty.{}">{}</button>"#,
                attr(&effect.to_json(r)),
                r.scope,
                slug(&label.english()),
                r.text(label)
            ));
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "empty_state", "title": self.title, "text": self.text, "button": self.button.as_ref().map(|(l, e)| json!({ "label": l, "effect": e })) })
    }
}

/// Grey placeholder lines shown while content loads. Hidden from screen readers, which hear
/// "Loading" instead.
pub struct Skeleton {
    lines: u8,
}

impl Skeleton {
    pub fn lines(lines: u8) -> Self {
        Self { lines: lines.clamp(1, 20) }
    }
}

impl Component for Skeleton {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<div class="z-skeleton" aria-busy="true"><span class="z-sr-only">{}</span>"#,
            r.text(&Text::key("zeyn.ui.loading"))
        ));
        for i in 0..self.lines {
            out.push_str(&format!(
                r#"<span class="z-skeleton-line" aria-hidden="true" style="--z-skeleton-width:{}%"></span>"#,
                100 - (i as u32 * 17) % 45
            ));
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({ "type": "skeleton", "lines": self.lines })
    }
}

/// A progress bar with a label and the value as text (ARIA progressbar).
pub struct Progress {
    label: Text,
    value: u64,
    max: u64,
    tone: Tone,
}

impl Progress {
    pub fn new(label: impl Into<Text>, value: u64, max: u64) -> Self {
        Self { label: label.into(), value: value.min(max.max(1)), max: max.max(1), tone: Tone::Info }
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

impl Component for Progress {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "progress");
        let percent = self.value * 100 / self.max;
        out.push_str(&format!(
            r#"<div class="z-progress z-tone-{}"><div class="z-progress-head"><span id="{id}">{}</span><span class="z-progress-value">{percent}%</span></div><div class="z-progress-track" role="progressbar" aria-labelledby="{id}" aria-valuemin="0" aria-valuemax="{}" aria-valuenow="{}" aria-valuetext="{percent}%"><span class="z-progress-bar" style="--z-progress:{percent}%"></span></div></div>"#,
            self.tone.code(),
            r.text(&self.label),
            self.max,
            self.value
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "progress", "label": self.label, "value": self.value, "max": self.max, "tone": self.tone })
    }
}

/// Read only details of a record as label and value pairs.
///
/// ```ignore
/// DescriptionList::of(&order).field(Order::customer).field(Order::placed_at).item("Status", Badge::new("Open"))
/// ```
pub struct DescriptionList {
    items: Vec<(Text, Value, FieldKind)>,
    columns: u8,
}

/// Builds a description list from one record.
pub struct DescriptionOf<'a, M> {
    model: &'a M,
    list: DescriptionList,
}

impl DescriptionList {
    pub fn of<M: Model>(model: &M) -> DescriptionOf<'_, M> {
        DescriptionOf { model, list: DescriptionList { items: Vec::new(), columns: 2 } }
    }

    pub fn new() -> Self {
        DescriptionList { items: Vec::new(), columns: 2 }
    }

    /// A free label and text.
    pub fn item(mut self, label: impl Into<Text>, text: impl Into<String>) -> Self {
        self.items.push((label.into(), Value::Text(text.into()), FieldKind::Text));
        self
    }

    /// Number of columns on wide screens, 1 to 4. The default is 2.
    pub fn columns(mut self, n: u8) -> Self {
        self.columns = n.clamp(1, 4);
        self
    }
}

impl Default for DescriptionList {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Model> DescriptionOf<'_, M> {
    pub fn field<T: FieldType + 'static>(mut self, field: Field<M, T>) -> Self {
        let meta = field.meta();
        self.list.items.push((meta.label_text(), field.get(self.model).to_value(), meta.kind));
        self
    }

    pub fn item(mut self, label: impl Into<Text>, text: impl Into<String>) -> Self {
        self.list = self.list.item(label, text);
        self
    }

    pub fn columns(mut self, n: u8) -> Self {
        self.list = self.list.columns(n);
        self
    }
}

impl<M> From<DescriptionOf<'_, M>> for Node {
    fn from(d: DescriptionOf<'_, M>) -> Self {
        Node::new(d.list)
    }
}

impl From<DescriptionList> for Node {
    fn from(d: DescriptionList) -> Self {
        Node::new(d)
    }
}

impl Component for DescriptionList {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(r#"<dl class="z-details" style="--z-columns:{}">"#, self.columns));
        for (label, value, kind) in &self.items {
            let shown = r.value(value, kind);
            let shown = if shown.is_empty() {
                r#"<span class="z-muted" aria-label="-">&#8212;</span>"#.to_string()
            } else {
                shown
            };
            out.push_str(&format!(r#"<div class="z-details-item"><dt>{}</dt><dd>{shown}</dd></div>"#, r.text(label)));
        }
        out.push_str("</dl>");
    }

    fn schema(&self) -> Json {
        let items: Vec<Json> = self.items.iter().map(|(l, v, _)| json!({ "label": l, "value": v })).collect();
        json!({ "type": "description_list", "columns": self.columns, "items": items })
    }
}

/// Where the screen sits: links to the screens above it, then the current one.
#[derive(Default)]
pub struct Breadcrumb {
    links: Vec<(Text, &'static str, Json)>,
    current: Option<Text>,
}

impl Breadcrumb {
    pub fn new() -> Self {
        Self::default()
    }

    /// A link to a screen. Params of the wrong type do not compile.
    pub fn link<S: ScreenDef>(mut self, label: impl Into<Text>, _screen: S, params: S::Params) -> Self {
        self.links.push((label.into(), S::META.name, serde_json::to_value(params).unwrap_or(Json::Null)));
        self
    }

    /// The current page, last in the trail.
    pub fn current(mut self, label: impl Into<Text>) -> Self {
        self.current = Some(label.into());
        self
    }
}

impl Component for Breadcrumb {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!(
            r#"<nav class="z-breadcrumb" aria-label="{}"><ol>"#,
            r.text(&Text::key("zeyn.ui.breadcrumb"))
        ));
        for (label, screen, params) in &self.links {
            let query =
                if params.is_null() { String::new() } else { format!("?params={}", encode(&params.to_string())) };
            out.push_str(&format!(
                r#"<li><a href="/_zeyn/route/{screen}{query}" data-zeyn-base-href>{}</a></li>"#,
                r.text(label)
            ));
        }
        if let Some(c) = &self.current {
            out.push_str(&format!(r#"<li><span aria-current="page">{}</span></li>"#, r.text(c)));
        }
        out.push_str("</ol></nav>");
    }

    fn schema(&self) -> Json {
        let links: Vec<Json> =
            self.links.iter().map(|(l, s, p)| json!({ "label": l, "screen": s, "params": p })).collect();
        json!({ "type": "breadcrumb", "links": links, "current": self.current })
    }
}

/// Percent encodes a query value.
pub(crate) fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Events in time order, such as the history of an order.
#[derive(Default)]
pub struct Timeline {
    events: Vec<(Value, Text, Option<Text>, Tone)>,
}

impl Timeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// An event at a date or moment, with a title and an optional detail.
    pub fn event<T: FieldType>(mut self, when: T, title: impl Into<Text>, detail: Option<Text>, tone: Tone) -> Self {
        self.events.push((when.to_value(), title.into(), detail, tone));
        self
    }
}

impl Component for Timeline {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(r#"<ol class="z-timeline">"#);
        for (when, title, detail, tone) in &self.events {
            let kind = match when {
                Value::Date(_) => FieldKind::Date,
                _ => FieldKind::DateTime,
            };
            out.push_str(&format!(
                r#"<li class="z-tone-{}"><span class="z-timeline-dot" aria-hidden="true"></span><div><p class="z-timeline-title">{}</p><p class="z-timeline-when">{}</p>"#,
                tone.code(),
                r.text(title),
                r.value(when, &kind)
            ));
            if let Some(d) = detail {
                out.push_str(&format!(r#"<p class="z-timeline-detail">{}</p>"#, r.text(d)));
            }
            out.push_str("</div></li>");
        }
        out.push_str("</ol>");
    }

    fn schema(&self) -> Json {
        let events: Vec<Json> = self
            .events
            .iter()
            .map(|(w, t, d, tone)| json!({ "when": w, "title": t, "detail": d, "tone": tone }))
            .collect();
        json!({ "type": "timeline", "events": events })
    }
}

type SuccessFn<O> = Arc<dyn Fn(&O, &mut UiCtx) + Send + Sync>;

/// A button that runs an action with an input fixed on the server, optionally after the user
/// confirms. The input never comes from the client; the action's permission is checked on the
/// server like any form.
///
/// ```ignore
/// ActionButton::new("Cancel order", cancel_order, params.id)
///     .confirm("The order will be cancelled. Continue?")
///     .tone(Tone::Danger)
///     .on_success(|_, ui| ui.close_and_refresh())
/// ```
pub struct ActionButton<A: Action> {
    label: Text,
    input: A::Input,
    confirm: Option<Text>,
    tone: Tone,
    tip: Option<Text>,
    on_success: Option<SuccessFn<A::Output>>,
}

impl<A: Action> ActionButton<A>
where
    A::Input: Clone + Sync,
{
    pub fn new(label: impl Into<Text>, _action: A, input: A::Input) -> Self {
        Self { label: label.into(), input, confirm: None, tone: Tone::Neutral, tip: None, on_success: None }
    }

    /// Asks the user to confirm with this message before running.
    pub fn confirm(mut self, message: impl Into<Text>) -> Self {
        self.confirm = Some(message.into());
        self
    }

    /// A tooltip: a short description shown on hover, focus and long press, and read by screen
    /// readers as the button's description. Never put essential information only here.
    pub fn tip(mut self, text: impl Into<Text>) -> Self {
        self.tip = Some(text.into());
        self
    }

    /// `Danger` draws a red button for actions that remove or cancel things.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn on_success(mut self, handler: impl Fn(&A::Output, &mut UiCtx) + Send + Sync + 'static) -> Self {
        self.on_success = Some(Arc::new(handler));
        self
    }
}

impl<A> FormSource for ActionButton<A>
where
    A: Action,
    A::Input: Clone + Sync,
    A::Output: Outcome,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn submit(&self, ctx: Ctx, _values: Map<String, Json>) -> BoxFuture<Result<Vec<Effect>>> {
        let input = self.input.clone();
        let on_success = self.on_success.clone();
        Box::pin(async move {
            let output = A::call(ctx, input).await?;
            if let Some(error) = output.failure() {
                return Err(error);
            }
            let mut ui = UiCtx::default();
            if let Some(handler) = on_success {
                handler(&output, &mut ui);
            }
            Ok(ui.into_effects())
        })
    }

    fn tool_schema(&self) -> Json {
        json!({ "type": "object", "properties": {}, "description": self.confirm.as_ref().map(Text::english) })
    }
}

impl<A> Component for ActionButton<A>
where
    A: Action,
    A::Input: Clone + Sync,
    A::Output: Outcome,
{
    fn render(&self, out: &mut String, r: &Render) {
        let confirm = match &self.confirm {
            Some(m) => format!(
                r#" data-zeyn-confirm="{}" data-text-title="{}" data-text-ok="{}" data-text-cancel="{}""#,
                r.text(m),
                r.text(&Text::key("zeyn.ui.confirm.title")),
                escape(&self.label_text(r)),
                r.text(&Text::key("zeyn.ui.confirm.cancel"))
            ),
            None => String::new(),
        };
        let class = if self.tone == Tone::Danger { "z-button z-danger-button" } else { "z-button" };
        let (tip_attrs, tip) = match &self.tip {
            Some(t) => crate::ui::tip(r, &crate::ui::dom_id(r, "tip"), t),
            None => (String::new(), String::new()),
        };
        out.push_str(&format!(
            r#"<form class="z-action" data-zeyn-submit="{}"{confirm} novalidate><button type="submit" class="{class}" data-testid="{}.action.{}"{tip_attrs}>{}</button>{tip}</form>"#,
            A::META.name,
            r.scope,
            slug(&self.label.english()),
            r.text(&self.label)
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "action_button", "label": self.label, "action": A::META, "confirm": self.confirm, "tone": self.tone })
    }

    fn form(&self) -> Option<&dyn FormSource> {
        Some(self)
    }
}

impl<A: Action> ActionButton<A> {
    fn label_text(&self, r: &Render) -> String {
        r.i18n.text(r.locale, &self.label)
    }
}

impl<A> From<ActionButton<A>> for Node
where
    A: Action,
    A::Input: Clone + Sync,
    A::Output: Outcome,
{
    fn from(b: ActionButton<A>) -> Self {
        Node::new(b)
    }
}
