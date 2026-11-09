//! StoryRing examples: avatars in a ring that says whether their stories are new, alone and in a
//! row that scrolls sideways.

use crate::data::{people, picture};
use crate::{Entry, ex};
use zeyn::prelude::*;

/// Whether the sample person `id` has stories the visitor has not seen: every third one has.
pub fn story_state(id: i64) -> StoryState {
    match id % 3 {
        0 => StoryState::Seen,
        _ => StoryState::Unseen,
    }
}

pub fn overview(_ctx: &Ctx) -> Node {
    Row::new()
        .compact()
        .col(StoryRing::new("Ada Lovelace", StoryState::Unseen).image(picture(3)))
        .col(StoryRing::new("Alan Turing", StoryState::Seen).image(picture(5)))
        .col(StoryRing::new("İlknur Şahin", StoryState::Unseen))
        .into()
}

/// The sample people as a row of rings, the people with new stories first.
pub fn row(_ctx: &Ctx) -> Node {
    let mut everyone: Vec<_> = people().iter().collect();
    everyone.sort_by_key(|p| story_state(p.id.0) == StoryState::Seen);
    everyone
        .into_iter()
        .fold(StoryRow::new(), |row, p| {
            row.add(StoryRing::new(p.name.clone(), story_state(p.id.0)).image(picture(p.id.0)))
        })
        .into()
}

/// What the catalog shows of `StoryRing`.
pub const ENTRY: Entry = Entry {
    name: "StoryRing",
    docs: "components/social#storyring",
    api: &["StoryRing", "StoryRow"],
    overview: &[ex!(overview)],
    examples: &[ex!(row)],
};
