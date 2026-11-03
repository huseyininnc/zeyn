//! Drop down and split buttons (APG menu button pattern).
//!
//! Every command of the menu is either an effect (open a window, go to a screen) or an action.
//! Action commands are child nodes with their own path, so they post to `/_zeyn/form` like any
//! [`ActionButton`](super::ActionButton): the screen's access, the feature's guards, the action's
//! permission, the interceptors, CSRF and the write rate limit are checked on every call, and the
//! input never comes from the browser. Commands the user may not run are left out of the menu.

use super::layout::into_node;
use crate::action::{Action, ActionMeta, Result};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::i18n::Text;
use crate::render::{Render, attr, slug};
use crate::screen::{Component, Effect, FormSource, Node, UiCtx};
use serde_json::{Map, Value as Json, json};
use std::sync::{Arc, OnceLock};

into_node!(MenuButton);

type SuccessFn<O> = Arc<dyn Fn(&O, &mut UiCtx) + Send + Sync>;
type Run = Arc<dyn Fn(Ctx) -> BoxFuture<Result<Vec<Effect>>> + Send + Sync>;

/// One command that runs an action. It becomes a child node of the menu button.
struct Command {
    label: Text,
    meta: ActionMeta,
    run: Run,
    confirm: Option<Text>,
    /// The main part of a split button rather than a menu item.
    primary: bool,
}

impl Command {
    fn new<A>(
        label: Text,
        input: A::Input,
        on_success: Option<SuccessFn<A::Output>>,
        primary: bool,
    ) -> Self
    where
        A: Action,
        A::Input: Clone + Sync,
        A::Output: Outcome,
    {
        let run: Run = Arc::new(move |ctx| {
            let input = input.clone();
            let on_success = on_success.clone();
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
        });
        Command {
            label,
            meta: A::META,
            run,
            confirm: None,
            primary,
        }
    }
}

/// The node of one command.
struct CommandNode(Arc<Command>);

impl FormSource for CommandNode {
    fn action(&self) -> ActionMeta {
        self.0.meta
    }

    fn submit(&self, ctx: Ctx, _values: Map<String, Json>) -> BoxFuture<Result<Vec<Effect>>> {
        (self.0.run)(ctx)
    }

    fn tool_schema(&self) -> Json {
        json!({ "type": "object", "properties": {}, "description": self.0.confirm.as_ref().map(Text::english) })
    }
}

impl Component for CommandNode {
    fn render(&self, out: &mut String, r: &Render) {
        let c = &self.0;
        let label = r.text(&c.label);
        let confirm = match &c.confirm {
            Some(m) => format!(
                r#" data-zeyn-confirm="{}" data-text-title="{}" data-text-ok="{label}" data-text-cancel="{}""#,
                r.text(m),
                r.text(&Text::key("zeyn.ui.confirm.title")),
                r.text(&Text::key("zeyn.ui.confirm.cancel"))
            ),
            None => String::new(),
        };
        let (class, role) = if c.primary {
            ("z-button z-split-main", "")
        } else {
            ("z-menu-entry", r#" role="menuitem" tabindex="-1""#)
        };
        let disabled = if r.can(&c.meta.access) {
            ""
        } else {
            " disabled"
        };
        out.push_str(&format!(
            r#"<form class="z-action" data-zeyn-submit="{}"{confirm} novalidate><button type="submit" class="{class}"{role}{disabled} data-testid="{}.menu.{}">{label}</button></form>"#,
            c.meta.name,
            r.scope,
            slug(&c.label.english())
        ));
    }

    fn schema(&self) -> Json {
        json!({ "type": "menu_command", "label": self.0.label, "action": self.0.meta, "confirm": self.0.confirm })
    }

    fn form(&self) -> Option<&dyn FormSource> {
        Some(self)
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "nearly every entry of the list is the large variant, so boxing it would add an allocation per entry and save no memory"
)]
enum Entry {
    /// An action command: the index of its node.
    Command(usize),
    Effect(Text, Effect),
    Separator,
}

/// A drop down button that opens a menu of commands, or a split button whose main part runs one
/// action and whose arrow opens the menu (APG menu button pattern).
///
/// Keyboard: Enter, Space and Arrow Down on the button open the menu on the first command, Arrow
/// Up on the last; in the menu the arrows move (and wrap), Home and End go to the ends, letters
/// jump to the next command that starts with them, Enter and Space run the command, Escape
/// closes the menu and returns focus to the button, Tab closes it and moves on.
///
/// ```ignore
/// MenuButton::new(t!("orders.more"))
///     .item(t!("orders.open"), Window::open(order_editor, OrderParams { id: Some(id) }))
///     .separator()
///     .action(t!("orders.archive"), archive_order, id)
///     .confirm(t!("orders.archive_confirm"))
///
/// MenuButton::split(t!("orders.save"), save_draft, id)
///     .action(t!("orders.save_and_send"), save_and_send, id)
/// ```
pub struct MenuButton {
    label: Text,
    split: bool,
    entries: Vec<Entry>,
    commands: Vec<Arc<Command>>,
    nodes: OnceLock<Vec<Node>>,
}

impl MenuButton {
    /// A drop down button labelled `label`.
    pub fn new(label: impl Into<Text>) -> Self {
        Self {
            label: label.into(),
            split: false,
            entries: Vec::new(),
            commands: Vec::new(),
            nodes: OnceLock::new(),
        }
    }

    /// A split button: pressing `label` runs `action` with `input`; the arrow beside it opens the
    /// menu.
    pub fn split<A>(label: impl Into<Text>, _action: A, input: A::Input) -> Self
    where
        A: Action,
        A::Input: Clone + Sync,
        A::Output: Outcome,
    {
        let label = label.into();
        let mut b = Self::new(label.clone());
        b.split = true;
        b.commands
            .push(Arc::new(Command::new::<A>(label, input, None, true)));
        b
    }

    /// A command that runs `action` with `input` on the server.
    pub fn action<A>(self, label: impl Into<Text>, _action: A, input: A::Input) -> Self
    where
        A: Action,
        A::Input: Clone + Sync,
        A::Output: Outcome,
    {
        self.push(Command::new::<A>(label.into(), input, None, false))
    }

    /// A command that runs `action` with `input`, then the effects `on_success` asks for.
    pub fn action_then<A>(
        self,
        label: impl Into<Text>,
        _action: A,
        input: A::Input,
        on_success: impl Fn(&A::Output, &mut UiCtx) + Send + Sync + 'static,
    ) -> Self
    where
        A: Action,
        A::Input: Clone + Sync,
        A::Output: Outcome,
    {
        self.push(Command::new::<A>(
            label.into(),
            input,
            Some(Arc::new(on_success)),
            false,
        ))
    }

    fn push(mut self, command: Command) -> Self {
        self.entries
            .push(Entry::Command(self.commands.len()));
        self.commands.push(Arc::new(command));
        self
    }

    /// A command that runs an effect in the browser, such as opening a window.
    pub fn item(mut self, label: impl Into<Text>, effect: Effect) -> Self {
        self.entries
            .push(Entry::Effect(label.into(), effect));
        self
    }

    /// A line between groups of commands.
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }

    /// Asks the user to confirm with this message before the last action command runs (the main
    /// action of a split button when no command was added yet).
    pub fn confirm(mut self, message: impl Into<Text>) -> Self {
        if let Some(c) = self
            .commands
            .last_mut()
            .and_then(Arc::get_mut)
        {
            c.confirm = Some(message.into());
        }
        self
    }

    fn nodes(&self) -> &[Node] {
        self.nodes.get_or_init(|| {
            self.commands
                .iter()
                .map(|c| Node::new(CommandNode(c.clone())))
                .collect()
        })
    }

    /// Whether the user may run the command at `index`.
    fn allowed(&self, r: &Render, index: usize) -> bool {
        self.commands
            .get(index)
            .is_some_and(|c| r.can(&c.meta.access))
    }
}

impl Component for MenuButton {
    fn render(&self, out: &mut String, r: &Render) {
        let id = super::dom_id(r, "menu");
        let label = r.text(&self.label);
        let nodes = self.nodes();
        let path = |i: usize| {
            if r.path.is_empty() {
                i.to_string()
            } else {
                format!("{}.{i}", r.path)
            }
        };
        let child = |out: &mut String, i: usize, tag: &str, attrs: &str| {
            let p = path(i);
            out.push_str(&format!(
                r#"<{tag} class="z-node" data-zeyn-node="{p}"{attrs}>"#
            ));
            nodes[i].render(out, &Render { path: &p, ..*r });
            out.push_str(&format!("</{tag}>"));
        };
        let trigger_label = if self.split {
            format!(
                r#" aria-label="{}""#,
                r.text(&Text::key("zeyn.ui.menu.more").arg("label", self.label.clone()))
            )
        } else {
            String::new()
        };
        if self.split {
            out.push_str(&format!(
                r#"<div class="z-split-button" role="group" aria-label="{label}">"#
            ));
            child(out, 0, "div", "");
        } else {
            out.push_str(r#"<div class="z-menu-button">"#);
        }
        out.push_str(&format!(
            r#"<button type="button" class="z-button z-menu-trigger" id="{id}-button" aria-haspopup="menu" aria-expanded="false" aria-controls="{id}"{trigger_label} data-testid="{}.menubutton.{}">"#,
            r.scope,
            slug(&self.label.english())
        ));
        if !self.split {
            out.push_str(&format!(r#"<span>{label}</span>"#));
        }
        out.push_str(&format!(
            r#"<span class="z-select-arrow" aria-hidden="true"></span></button><ul class="z-menu-list" role="menu" id="{id}" aria-labelledby="{id}-button" hidden>"#
        ));
        let mut separator = false;
        let mut any = false;
        for entry in &self.entries {
            match entry {
                Entry::Separator => separator = any,
                Entry::Command(i) if !self.allowed(r, *i) => {}
                Entry::Command(i) => {
                    if std::mem::take(&mut separator) {
                        out.push_str(r#"<li role="separator" class="z-menu-sep"></li>"#);
                    }
                    child(out, *i, "li", r#" role="none""#);
                    any = true;
                }
                Entry::Effect(text, effect) => {
                    if std::mem::take(&mut separator) {
                        out.push_str(r#"<li role="separator" class="z-menu-sep"></li>"#);
                    }
                    out.push_str(&format!(
                        r#"<li role="none"><button type="button" class="z-menu-entry" role="menuitem" tabindex="-1" data-zeyn-effect="{}" data-testid="{}.menu.{}">{}</button></li>"#,
                        attr(&effect.to_json(r)),
                        r.scope,
                        slug(&text.english()),
                        r.text(text)
                    ));
                    any = true;
                }
            }
        }
        out.push_str("</ul></div>");
    }

    fn schema(&self) -> Json {
        let items: Vec<Json> = self
            .entries
            .iter()
            .map(|e| match e {
                Entry::Command(i) => self.nodes()[*i].schema(),
                Entry::Effect(label, effect) => {
                    json!({ "type": "menu_item", "label": label, "effect": effect })
                }
                Entry::Separator => json!({ "type": "separator" }),
            })
            .collect();
        let primary = self
            .split
            .then(|| self.nodes()[0].schema());
        json!({ "type": "menu_button", "label": self.label, "split": primary, "items": items })
    }

    fn children(&self) -> &[Node] {
        self.nodes()
    }
}
