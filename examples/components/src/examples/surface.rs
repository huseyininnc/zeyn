//! Surface examples: items placed freely inside bounds, on a grid and on an axis, and resized.

use crate::{Entry, ex};
use zeyn::prelude::*;
use zeyn::ui::Axis;

pub fn overview(_ctx: &Ctx) -> Node {
    Surface::new("Floor plan", 560, 320)
        .snap(20)
        .item(
            Draggable::new("t1", Paragraph::new("Table 1"))
                .label("Table 1")
                .at(40, 40)
                .size(120, 80),
        )
        .item(
            Draggable::new("t2", Paragraph::new("Table 2"))
                .label("Table 2")
                .at(220, 40)
                .size(120, 80),
        )
        .item(
            Draggable::new("bar", Paragraph::new("Bar"))
                .label("Bar")
                .at(40, 200)
                .size(300, 60),
        )
        .into()
}

pub fn on_one_axis(_ctx: &Ctx) -> Node {
    Surface::new("Timeline", 560, 120)
        .snap(40)
        .item(
            Draggable::new("start", Paragraph::new("Start"))
                .label("Start")
                .at(0, 20)
                .size(120, 60)
                .axis(Axis::X),
        )
        .item(
            Draggable::new("review", Paragraph::new("Review"))
                .label("Review")
                .at(200, 20)
                .size(120, 60)
                .axis(Axis::X),
        )
        .into()
}

pub fn resizable(_ctx: &Ctx) -> Node {
    Surface::new("Dashboard", 560, 320)
        .snap(20)
        .item(
            Draggable::new("notes", Paragraph::new("Notes"))
                .label("Notes")
                .at(20, 20)
                .size(200, 120)
                .resizable()
                .min_size(120, 60),
        )
        .item(
            Draggable::new("video", Paragraph::new("Video 16:9"))
                .label("Video")
                .at(260, 20)
                .size(240, 135)
                .min_size(160, 90)
                .max_size(480, 270)
                .aspect(16, 9),
        )
        .into()
}

/// What the catalog shows of `Surface`.
pub const ENTRY: Entry = Entry {
    name: "Surface",
    docs: "components/layouts#surface",
    api: &["Surface", "Draggable"],
    overview: &[ex!(overview)],
    examples: &[ex!(on_one_axis), ex!(resizable)],
};
