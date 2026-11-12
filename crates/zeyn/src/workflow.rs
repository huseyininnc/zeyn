//! Typed approval workflows: states, the transitions between them and who may make each one.
//!
//! ```
//! use zeyn::prelude::*;
//! use zeyn::Workflow;
//!
//! #[choice]
//! pub enum LeaveState { Draft, Submitted, Approved, Rejected }
//!
//! fn leave_flow() -> Workflow<LeaveState> {
//!     Workflow::new()
//!         .transition("submit", LeaveState::Draft, LeaveState::Submitted, "people.leave.request")
//!         .transition("approve", LeaveState::Submitted, LeaveState::Approved, "people.leave.approve")
//!         .transition("reject", LeaveState::Submitted, LeaveState::Rejected, "people.leave.approve")
//! }
//!
//! let manager = zeyn::Identity::user("m").with_permissions(["people.leave.approve"]);
//! let names: Vec<&str> = leave_flow().allowed(&manager, LeaveState::Submitted).iter().map(|t| t.name).collect();
//! assert_eq!(names, ["approve", "reject"]);
//! assert!(leave_flow().check().is_ok());
//! ```

use crate::action::{Access, Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx, Identity, Outcome};
use crate::i18n::Text;
use crate::model::{FieldError, Rule, ValidationErrors};
use crate::render::{Render, escape};
use crate::screen::{Component, Effect, FormSource, Node, UiCtx};
use crate::value::{FieldType, Value};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json, json};
use std::marker::PhantomData;
use std::sync::Arc;

/// Longest comment a transition takes, in characters.
pub const MAX_COMMENT: usize = 1000;

/// A move from one state to another, and the permission it needs.
#[derive(Clone, Debug)]
pub struct Transition<S> {
    pub name: &'static str,
    pub label: Text,
    pub from: S,
    pub to: S,
    pub permission: &'static str,
}

/// A transition as written to the audit log: who is in the audit entry, this says from, to and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TransitionRecord {
    pub name: &'static str,
    pub from: String,
    pub to: String,
    pub comment: Option<String>,
}

/// States are `#[choice]` enums.
pub trait WorkflowState: FieldType + Copy + PartialEq + Send + Sync + 'static {}

impl<S: FieldType + Copy + PartialEq + Send + Sync + 'static> WorkflowState for S {}

fn state_text<S: WorkflowState>(state: S) -> String {
    match state.to_value() {
        Value::Choice(v) => v.to_string(),
        other => crate::audit::plain(&other).unwrap_or_default(),
    }
}

/// The states of a record and the allowed moves between them. Every move is checked on the
/// server: the record must be in the move's `from` state and the user must have its permission.
pub struct Workflow<S> {
    transitions: Vec<Transition<S>>,
}

impl<S: WorkflowState> Default for Workflow<S> {
    fn default() -> Self {
        Workflow {
            transitions: Vec::new(),
        }
    }
}

impl<S: WorkflowState> Workflow<S> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allows a move. Its button label is the catalog text `workflow.<name>`, or the name.
    pub fn transition(
        mut self,
        name: &'static str,
        from: S,
        to: S,
        permission: &'static str,
    ) -> Self {
        self.transitions.push(Transition {
            name,
            label: Text::key_or(format!("workflow.{name}"), name),
            from,
            to,
            permission,
        });
        self
    }

    /// Button label of the last added move.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        if let Some(t) = self.transitions.last_mut() {
            t.label = label.into();
        }
        self
    }

    pub fn transitions(&self) -> &[Transition<S>] {
        &self.transitions
    }

    /// Moves this user may make from `current`. Use it to show buttons; [`Workflow::apply`]
    /// checks again.
    pub fn allowed(&self, identity: &Identity, current: S) -> Vec<&Transition<S>> {
        self.transitions
            .iter()
            .filter(|t| t.from == current && identity.can(&Access::Permission(t.permission)))
            .collect()
    }

    /// Checks the definition: permissions are dotted lowercase names, and a move name leads to
    /// one target from each state.
    pub fn check(&self) -> std::result::Result<(), Vec<String>> {
        let mut problems = Vec::new();
        for (i, t) in self.transitions.iter().enumerate() {
            let parts: Vec<&str> = t.permission.split('.').collect();
            let valid = parts.len() >= 2
                && parts.iter().all(|p| {
                    !p.is_empty()
                        && p.chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                });
            if !valid {
                problems.push(
                    crate::Problem::new(
                        "Z0506",
                        format!("transition `{}` has an invalid permission `{}`", t.name, t.permission),
                    )
                    .why("permissions are matched by their dotted lower case name, so this one could never be granted and nobody could make the move")
                    .fix("use lower case dotted names such as `leave.approve`")
                    .docs("guides/enterprise#approval-workflows")
                    .to_string(),
                );
            }
            if self.transitions[..i]
                .iter()
                .any(|o| o.name == t.name && o.from == t.from)
            {
                problems.push(
                    crate::Problem::new(
                        "Z0507",
                        format!("transition `{}` is declared twice from the same state", t.name),
                    )
                    .why("a move name must lead to one target from each state, so the second declaration makes the move ambiguous")
                    .fix("remove one of them, or give it another name")
                    .docs("guides/enterprise#approval-workflows")
                    .to_string(),
                );
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }

    /// Makes a move on the server. Call it in the action that saves the record, with the state
    /// read from the store:
    ///
    /// ```ignore
    /// let mut leave = store.get(req.id)?;
    /// leave.state = leave_flow().apply(&ctx, leave.state, &req.transition, req.comment.as_deref())?;
    /// store.save(leave)
    /// ```
    ///
    /// Unknown moves are `NotFound`, a record in another state is `Conflict` (someone moved it
    /// first), a missing permission is `Forbidden`. The move is recorded for the audit log.
    pub fn apply(&self, ctx: &Ctx, current: S, name: &str, comment: Option<&str>) -> Result<S> {
        let mut named = self
            .transitions
            .iter()
            .filter(|t| t.name == name)
            .peekable();
        if named.peek().is_none() {
            return Err(Error::NotFound);
        }
        let t = named
            .find(|t| t.from == current)
            .ok_or(Error::Conflict)?;
        if !ctx
            .identity()
            .can(&Access::Permission(t.permission))
        {
            return Err(Error::Forbidden);
        }
        let comment = comment
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(str::to_string);
        if comment
            .as_ref()
            .is_some_and(|c| c.chars().count() > MAX_COMMENT)
        {
            let mut errors = ValidationErrors::default();
            errors.errors.push(FieldError {
                field: "comment",
                rule: Rule::MaxLen(MAX_COMMENT),
                message: Text::key("zeyn.validation.max_len")
                    .arg("label", Text::key("zeyn.workflow.comment"))
                    .arg("n", MAX_COMMENT.to_string()),
            });
            return Err(Error::Validation(errors));
        }
        let record = TransitionRecord {
            name: t.name,
            from: state_text(t.from),
            to: state_text(t.to),
            comment,
        };
        ctx.update_trail(|trail| trail.transition = Some(record));
        Ok(t.to)
    }
}

/// What a transition button sends to its action. The record key comes from the screen on the
/// server, never from the client.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransitionRequest<K> {
    pub id: K,
    pub transition: String,
    pub comment: Option<String>,
}

type SuccessFn<O> = Arc<dyn Fn(&O, &mut UiCtx) + Send + Sync>;

/// Buttons for the moves the user may make on one record, with an optional comment. Submitting
/// calls the action with a [`TransitionRequest`].
///
/// ```ignore
/// Transitions::of(leave_flow(), leave.state, leave.id, move_leave).on_success(|_, ui| ui.close_and_refresh())
/// ```
pub struct Transitions<S, K, A: Action> {
    flow: Workflow<S>,
    current: S,
    id: K,
    on_success: Option<SuccessFn<A::Output>>,
    /// Per transition name: the confirmation message and whether the button is destructive.
    asks: Vec<(&'static str, Option<Text>, bool)>,
    _action: PhantomData<fn() -> A>,
}

impl<S, K, A> Transitions<S, K, A>
where
    S: WorkflowState,
    K: Clone + Send + Sync + 'static,
    A: Action<Input = TransitionRequest<K>>,
{
    pub fn of(flow: Workflow<S>, current: S, id: K, _action: A) -> Self {
        Transitions {
            flow,
            current,
            id,
            on_success: None,
            asks: Vec::new(),
            _action: PhantomData,
        }
    }

    fn ask(&mut self, name: &'static str) -> &mut (&'static str, Option<Text>, bool) {
        let at = match self
            .asks
            .iter()
            .position(|a| a.0 == name)
        {
            Some(at) => at,
            None => {
                self.asks.push((name, None, false));
                self.asks.len() - 1
            }
        };
        &mut self.asks[at]
    }

    /// Asks the user to confirm with this message before the transition `name` runs, in the same
    /// dialog as [`crate::ui::ActionButton::confirm`]. The question is asked in the browser; the
    /// server still checks the move and its permission.
    ///
    /// ```ignore
    /// Transitions::of(order_flow(), order.state, order.id, move_order)
    ///     .confirm("cancel", t!("orders.cancel_confirm"))
    ///     .tone("cancel", Tone::Danger)
    /// ```
    pub fn confirm(mut self, name: &'static str, message: impl Into<Text>) -> Self {
        self.ask(name).1 = Some(message.into());
        self
    }

    /// `Tone::Danger` draws the button of transition `name`, and its confirmation, in red.
    pub fn tone(mut self, name: &'static str, tone: crate::grid::Tone) -> Self {
        self.ask(name).2 = tone == crate::grid::Tone::Danger;
        self
    }

    /// What happens after a successful move. Shows a short confirmation unless set.
    pub fn on_success(
        mut self,
        handler: impl Fn(&A::Output, &mut UiCtx) + Send + Sync + 'static,
    ) -> Self {
        self.on_success = Some(Arc::new(handler));
        self
    }
}

impl<S, K, A> Component for Transitions<S, K, A>
where
    S: WorkflowState,
    K: Clone + Send + Sync + 'static,
    A: Action<Input = TransitionRequest<K>>,
    A::Output: Outcome,
{
    fn render(&self, out: &mut String, r: &Render) {
        let scope = r.scope;
        let state = r.value(&self.current.to_value(), &S::KIND);
        out.push_str(&format!(
            r#"<div class="z-transitions" data-testid="{scope}.transitions"><p class="z-state">{}: <strong>{state}</strong></p>"#,
            r.text(&Text::key("zeyn.workflow.state"))
        ));
        let allowed = r
            .ctx
            .map(|c| {
                self.flow
                    .allowed(c.identity(), self.current)
            })
            .unwrap_or_default();
        if !allowed.is_empty() {
            out.push_str(&format!(
                r#"<form class="z-form" data-zeyn-submit="{action}" data-testid="{scope}.transitions.form" novalidate><input type="text" name="transition" hidden value=""><div class="z-field"><label for="{scope}-comment">{label}</label><textarea id="{scope}-comment" name="comment" maxlength="{MAX_COMMENT}" rows="2" aria-describedby="{scope}-comment-error"></textarea><p class="z-field-error" id="{scope}-comment-error" aria-live="polite"></p></div><div class="z-form-actions">"#,
                action = A::META.name,
                label = r.text(&Text::key("zeyn.workflow.comment")),
            ));
            for t in allowed {
                let (confirm, danger) = self
                    .asks
                    .iter()
                    .find(|a| a.0 == t.name)
                    .map(|a| (a.1.as_ref(), a.2))
                    .unwrap_or((None, false));
                let label = r.text(&t.label);
                let confirm = confirm
                    .map(|m| {
                        format!(
                            r#" data-zeyn-confirm="{}" data-text-title="{}" data-text-ok="{label}" data-text-cancel="{}""#,
                            r.text(m),
                            r.text(&Text::key("zeyn.ui.confirm.title")),
                            r.text(&Text::key("zeyn.ui.confirm.cancel"))
                        )
                    })
                    .unwrap_or_default();
                let class = if danger {
                    "z-button z-danger-button"
                } else {
                    "z-button"
                };
                out.push_str(&format!(
                    r#"<button type="submit" class="{class}" data-zeyn-transition="{name}" data-testid="{scope}.transition.{name}"{confirm}>{label}</button>"#,
                    name = escape(t.name),
                ));
            }
            out.push_str("</div></form>");
        }
        out.push_str("</div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "transitions",
            "action": A::META,
            "state": state_text(self.current),
            "transitions": self.flow.transitions.iter().map(|t| json!({
                "name": t.name, "label": t.label, "from": state_text(t.from), "to": state_text(t.to), "permission": t.permission,
            })).collect::<Vec<_>>(),
        })
    }

    fn form(&self) -> Option<&dyn FormSource> {
        Some(self)
    }
}

impl<S, K, A> FormSource for Transitions<S, K, A>
where
    S: WorkflowState,
    K: Clone + Send + Sync + 'static,
    A: Action<Input = TransitionRequest<K>>,
    A::Output: Outcome,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn submit(&self, ctx: Ctx, values: Map<String, Json>) -> BoxFuture<Result<Vec<Effect>>> {
        let text = |k: &str| {
            values
                .get(k)
                .and_then(Json::as_str)
                .map(str::to_string)
        };
        let request = TransitionRequest {
            id: self.id.clone(),
            transition: text("transition").unwrap_or_default(),
            comment: text("comment"),
        };
        let on_success = self.on_success.clone();
        Box::pin(async move {
            let output = A::call(ctx, request).await?;
            if let Some(error) = output.failure() {
                return Err(error);
            }
            let mut ui = UiCtx::default();
            match on_success {
                Some(handler) => handler(&output, &mut ui),
                None => ui.toast(Text::key("zeyn.workflow.done")),
            }
            Ok(ui.into_effects())
        })
    }

    fn tool_schema(&self) -> Json {
        let names: Vec<&str> = self
            .flow
            .transitions
            .iter()
            .filter(|t| t.from == self.current)
            .map(|t| t.name)
            .collect();
        json!({
            "type": "object",
            "properties": {
                "transition": { "type": "string", "enum": names, "description": "Move to make." },
                "comment": { "type": "string", "maxLength": MAX_COMMENT, "description": "Why, recorded in the audit log." }
            },
            "required": ["transition"]
        })
    }
}

impl<S, K, A> From<Transitions<S, K, A>> for Node
where
    S: WorkflowState,
    K: Clone + Send + Sync + 'static,
    A: Action<Input = TransitionRequest<K>>,
    A::Output: Outcome,
{
    fn from(t: Transitions<S, K, A>) -> Self {
        Node::new(t)
    }
}
