//! Reactions examples: a set of reaction buttons with counts from an action.

use crate::{Entry, ex};
use std::sync::atomic::{AtomicU64, Ordering};
use zeyn::prelude::*;
use zeyn::ui::{ReactionPress, ReactionState};

/// The reactions of the examples: key, name and symbol.
const SET: [(&str, &str, &str); 3] =
    [("helpful", "Helpful", "\u{1f44d}"), ("thanks", "Thanks", "\u{1f64f}"), ("idea", "Good idea", "\u{1f4a1}")];

/// Counts shared by every visitor, kept in memory; one per reaction, so they never grow.
static COUNTS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

/// The counts now, with `mine` as the user's reaction.
fn state(mine: Option<&str>) -> ReactionState {
    let counted = SET
        .iter()
        .zip(&COUNTS)
        .fold(ReactionState::default(), |s, ((key, _, _), n)| s.count(*key, n.load(Ordering::Relaxed)));
    match mine {
        Some(m) => counted.mine(m),
        None => counted,
    }
}

/// Counts a press. The catalog keeps no users, so "mine" is the reaction just pressed.
#[action(public)]
pub async fn react(press: ReactionPress) -> Result<ReactionState> {
    let i = SET
        .iter()
        .position(|(k, _, _)| *k == press.reaction)
        .ok_or(Error::NotFound)?;
    let _ = COUNTS[i].fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
        Some(if press.pressed { n.saturating_add(1) } else { n.saturating_sub(1) })
    });
    Ok(state(
        press
            .pressed
            .then_some(press.reaction.as_str()),
    ))
}

pub fn overview(_ctx: &Ctx) -> Node {
    SET.iter()
        .fold(Reactions::of(react, "catalog"), |row, (key, name, symbol)| row.reaction(key, *name, symbol))
        .state(state(None))
        .into()
}

/// What the catalog shows of `Reactions`.
pub const ENTRY: Entry = Entry {
    name: "Reactions",
    docs: "components/content#reactions",
    api: &["Reactions", "ReactionState"],
    overview: &[ex!(overview)],
    examples: &[],
};
