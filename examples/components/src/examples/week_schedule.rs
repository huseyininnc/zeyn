//! WeekSchedule examples: seven day columns of rows.

use crate::data::{Event, week_events};
use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::ListItem;

pub fn overview(_ctx: &Ctx) -> Node {
    WeekSchedule::of(week_events)
        .title("This week")
        .day(|e: &Event| e.day)
        .item(|e: &Event| ListItem::new(e.title.clone()).meta(e.starts.clone()))
        .into()
}

/// What the catalog shows of `WeekSchedule`.
pub const ENTRY: Entry = Entry {
    name: "WeekSchedule",
    docs: "components/content#weekschedule",
    api: &["WeekSchedule"],
    overview: &[ex!(overview)],
    examples: &[],
};
