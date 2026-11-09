//! [`Reactions`]: a fixed set of reaction buttons with counts, backed by an action.

use super::DataSource;
use super::layout::into_node;
use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx};
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::marker::PhantomData;

into_node!(Reactions);

/// Most reactions one set offers; more are left out.
pub const MAX_REACTIONS: usize = 12;

/// What the action of [`Reactions`] gets when a reaction button is pressed: the item the
/// reactions are about (set by the screen, never by the browser), the reaction (always one of
/// the set) and whether the user turned it on or off.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReactionPress {
    pub target: String,
    pub reaction: String,
    pub pressed: bool,
}

/// Counts of the reactions of an item and the ones the user has given. The action of
/// [`Reactions`] returns it after a press; the screen passes it to [`Reactions::state`] for the
/// first render.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReactionState {
    pub counts: BTreeMap<String, u64>,
    pub mine: Vec<String>,
}

impl ReactionState {
    /// Sets the count of a reaction.
    pub fn count(mut self, reaction: impl Into<String>, count: u64) -> Self {
        self.counts
            .insert(reaction.into(), count);
        self
    }

    /// Marks a reaction as given by the user.
    pub fn mine(mut self, reaction: impl Into<String>) -> Self {
        self.mine.push(reaction.into());
        self
    }
}

/// Runs the action with the screen's target and a reaction of the set; anything else is
/// refused before the action runs.
struct ReactionSource<A> {
    target: String,
    allowed: Vec<&'static str>,
    _action: PhantomData<fn() -> A>,
}

#[derive(Deserialize)]
struct ClientPress {
    reaction: String,
    #[serde(default)]
    pressed: bool,
}

impl<A> DataSource for ReactionSource<A>
where
    A: Action<Input = ReactionPress, Output = Result<ReactionState>>,
{
    fn action(&self) -> ActionMeta {
        A::META
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        let press = serde_json::from_value::<ClientPress>(input)
            .ok()
            .filter(|p| {
                self.allowed
                    .contains(&p.reaction.as_str())
            })
            .map(|p| ReactionPress {
                target: self.target.clone(),
                reaction: p.reaction,
                pressed: p.pressed,
            });
        Box::pin(async move {
            let press = press.ok_or(Error::NotFound)?;
            let state = A::call(ctx.clone(), press).await??;
            crate::privacy::client_json(&state, &ctx).map_err(|e| Error::Internal(e.to_string()))
        })
    }
}

/// A row of reaction buttons chosen by the app, each with a symbol, a name and a count, such as
/// "Helpful" and "Thanks" under an answer. Pressing a button runs the action through
/// `POST /_zeyn/data` with the CSRF check and the write rate limit, as the user, with the
/// action's permission checked on the server; the action stores the press and returns the new
/// [`ReactionState`], and the counts and pressed states update in place without a reload.
///
/// The item (`target`) is set by the screen and never read from the browser, and a reaction
/// outside the set is refused before the action runs. Each button is a toggle button
/// (`aria-pressed`) named by the reaction's name and its count; the symbol is hidden from
/// screen readers. Without script the counts still show.
///
/// ```
/// use zeyn::prelude::*;
/// use zeyn::ui::{ReactionPress, ReactionState, Reactions};
///
/// #[action(permission = "answers.react")]
/// pub async fn react_to_answer(press: ReactionPress) -> Result<ReactionState> {
///     Ok(ReactionState::default().count(press.reaction.clone(), 1).mine(press.reaction))
/// }
///
/// let row = Reactions::of(react_to_answer, "answer-42")
///     .label("Reactions")
///     .reaction("helpful", "Helpful", "\u{1f44d}")
///     .reaction("thanks", "Thanks", "\u{1f64f}")
///     .state(ReactionState::default().count("helpful", 12).mine("helpful"));
/// ```
pub struct Reactions {
    label: Option<Text>,
    target: String,
    reactions: ReactionSet,
    state: ReactionState,
    source: Box<dyn DataSource>,
    make: fn(String, Vec<&'static str>) -> Box<dyn DataSource>,
    action: ActionMeta,
}

fn make_source<A>(target: String, allowed: Vec<&'static str>) -> Box<dyn DataSource>
where
    A: Action<Input = ReactionPress, Output = Result<ReactionState>>,
{
    Box::new(ReactionSource::<A> {
        target,
        allowed,
        _action: PhantomData,
    })
}

impl Reactions {
    /// Reactions to `target`, an id of the item they are about, stored by `action`.
    pub fn of<A>(_action: A, target: impl Into<String>) -> Self
    where
        A: Action<Input = ReactionPress, Output = Result<ReactionState>>,
    {
        let target = target.into();
        Reactions {
            label: None,
            source: make_source::<A>(target.clone(), Vec::new()),
            target,
            reactions: ReactionSet::default(),
            state: ReactionState::default(),
            make: make_source::<A>,
            action: A::META,
        }
    }

    /// A reaction: its key (what the action gets), its name and its symbol, such as an emoji.
    pub fn reaction(
        mut self,
        key: &'static str,
        name: impl Into<Text>,
        symbol: &'static str,
    ) -> Self {
        if self
            .reactions
            .add(key, name.into(), symbol)
        {
            self.source = (self.make)(self.target.clone(), self.reactions.keys());
        }
        self
    }

    /// The counts and the user's own reactions for the first render.
    pub fn state(mut self, state: ReactionState) -> Self {
        self.state = state;
        self
    }

    /// The name of the row for screen readers, "Reactions" unless set.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Component for Reactions {
    fn render(&self, out: &mut String, r: &Render) {
        reaction_row(
            out,
            r,
            self.label.as_ref(),
            &self.reactions,
            &self.state,
            false,
        );
    }

    fn schema(&self) -> Json {
        json!({
            "type": "reactions",
            "action": self.action,
            "target": self.target,
            "reactions": self.reactions.schema(),
            "state": self.state,
        })
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        (key == "react").then_some(self.source.as_ref())
    }
}

/// The reactions a row offers: key, name and symbol, each key once, at most [`MAX_REACTIONS`].
#[derive(Default)]
pub(crate) struct ReactionSet(Vec<(&'static str, Text, &'static str)>);

impl ReactionSet {
    /// Adds a reaction; `false` when its key is already there or the set is full.
    pub(crate) fn add(&mut self, key: &'static str, name: Text, symbol: &'static str) -> bool {
        let fits = self.0.len() < MAX_REACTIONS && !self.0.iter().any(|(k, _, _)| *k == key);
        if fits {
            self.0.push((key, name, symbol));
        }
        fits
    }

    pub(crate) fn keys(&self) -> Vec<&'static str> {
        self.0
            .iter()
            .map(|(k, _, _)| *k)
            .collect()
    }

    pub(crate) fn schema(&self) -> Json {
        self.0
            .iter()
            .map(|(k, n, s)| json!({ "key": k, "name": n, "symbol": s }))
            .collect::<Vec<_>>()
            .into()
    }
}

/// Writes a row of reaction buttons. The row of a feed post (`of_post`) carries
/// `data-zeyn-post`, so `reactions.js` sends the key of the post around it.
pub(crate) fn reaction_row(
    out: &mut String,
    r: &Render,
    label: Option<&Text>,
    set: &ReactionSet,
    state: &ReactionState,
    of_post: bool,
) {
    let label = match label {
        Some(l) => r.text(l),
        None => r.text(&Text::key("zeyn.ui.reactions.label")),
    };
    out.push_str(&format!(
        r#"<div class="z-reactions" role="group" aria-label="{label}" data-zeyn-reactions{} data-failed="{}">"#,
        if of_post { " data-zeyn-post" } else { "" },
        r.text(&Text::key("zeyn.ui.reactions.failed"))
    ));
    for (key, name, symbol) in &set.0 {
        let count = state
            .counts
            .get(*key)
            .copied()
            .unwrap_or(0);
        let mine = state.mine.iter().any(|m| m == key);
        out.push_str(&format!(
            r#"<button type="button" class="z-reaction" aria-pressed="{mine}" data-reaction="{}" data-testid="{}.reaction.{}"><span class="z-reaction-symbol" aria-hidden="true">{}</span><span class="z-sr-only">{}</span><span class="z-reaction-count">{count}</span></button>"#,
            escape(key),
            r.scope,
            escape(key),
            escape(symbol),
            r.text(name)
        ));
    }
    out.push_str("</div>");
}
