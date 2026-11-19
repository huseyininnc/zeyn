//! [`Stepper`]: where a process stands, as a list of its steps.

use super::layout::into_node;
use crate::i18n::Text;
use crate::render::Render;
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

into_node!(Stepper);

/// Most steps a [`Stepper`] holds; steps added after that are left out.
pub const MAX_STEPS: usize = 12;

struct Step {
    title: Text,
    description: Option<Text>,
}

/// Where a process stands: its steps in order, the ones done, the current one and the ones still
/// to come, such as the way of an order from "Placed" to "Delivered". It only shows the state
/// the server gives it; a form filled in steps is `Form` with its own steps.
///
/// The steps are an ordered list named by the label. The current step carries
/// `aria-current="step"`, and a done step says "completed" to screen readers, so the state is
/// not given by color alone. `.current(index)` counts from 0; `.finished()` marks every step
/// done. `.vertical()` puts the steps one under the other, which is also how they show on
/// narrow screens.
///
/// ```
/// use zeyn::ui::Stepper;
/// let order = Stepper::new("Order progress")
///     .step("Placed")
///     .step("Packed")
///     .description("At the warehouse since Monday")
///     .step("Shipped")
///     .step("Delivered")
///     .current(1);
/// ```
pub struct Stepper {
    label: Text,
    steps: Vec<Step>,
    current: usize,
    vertical: bool,
}

impl Stepper {
    /// A stepper with an accessible name, such as "Order progress". The first step is current.
    pub fn new(label: impl Into<Text>) -> Self {
        Stepper {
            label: label.into(),
            steps: Vec::new(),
            current: 0,
            vertical: false,
        }
    }

    /// Adds a step. At most [`MAX_STEPS`].
    pub fn step(mut self, title: impl Into<Text>) -> Self {
        if self.steps.len() < MAX_STEPS {
            self.steps.push(Step {
                title: title.into(),
                description: None,
            });
        }
        self
    }

    /// A line under the title of the step added last.
    pub fn description(mut self, text: impl Into<Text>) -> Self {
        if let Some(last) = self.steps.last_mut() {
            last.description = Some(text.into());
        }
        self
    }

    /// The step the process is at, counted from 0. Steps before it are done.
    pub fn current(mut self, index: usize) -> Self {
        self.current = index;
        self
    }

    /// Marks every step done.
    pub fn finished(mut self) -> Self {
        self.current = usize::MAX;
        self
    }

    /// Steps one under the other.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }

    fn state(&self, index: usize) -> &'static str {
        match index.cmp(&self.current) {
            std::cmp::Ordering::Less => "done",
            std::cmp::Ordering::Equal => "current",
            std::cmp::Ordering::Greater => "todo",
        }
    }
}

impl Component for Stepper {
    fn render(&self, out: &mut String, r: &Render) {
        let done = r.text(&Text::key("zeyn.ui.stepper.done"));
        out.push_str(&format!(
            r#"<ol class="z-stepper{}" aria-label="{}">"#,
            if self.vertical {
                " z-stepper-vertical"
            } else {
                ""
            },
            r.text(&self.label)
        ));
        for (i, step) in self.steps.iter().enumerate() {
            let state = self.state(i);
            let (current, mark, spoken) = match state {
                "done" => (
                    "",
                    String::from("&#10003;"),
                    format!(r#"<span class="z-sr-only"> ({done})</span>"#),
                ),
                "current" => (
                    r#" aria-current="step""#,
                    (i + 1).to_string(),
                    String::new(),
                ),
                _ => ("", (i + 1).to_string(), String::new()),
            };
            let description = step
                .description
                .as_ref()
                .map(|d| format!(r#"<span class="z-stage-desc">{}</span>"#, r.text(d)))
                .unwrap_or_default();
            out.push_str(&format!(
                r#"<li class="z-stage z-stage-{state}"{current}><span class="z-stage-mark" aria-hidden="true">{mark}</span><span class="z-stage-text"><span class="z-stage-title">{}{spoken}</span>{description}</span></li>"#,
                r.text(&step.title)
            ));
        }
        out.push_str("</ol>");
    }

    fn schema(&self) -> Json {
        let steps: Vec<Json> = self
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| json!({ "title": s.title, "description": s.description, "state": self.state(i) }))
            .collect();
        json!({ "type": "stepper", "label": self.label, "vertical": self.vertical, "steps": steps })
    }
}
