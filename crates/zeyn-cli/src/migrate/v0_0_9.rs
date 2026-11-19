//! 0.0.9 brought the new icon set with two renamed icons.

use super::{Change, Step};

pub const STEP: Step = Step {
    version: "0.0.9",
    changes: &[
        Change::Rename("Icon::Delete", "Icon::Trash"),
        Change::Rename("Icon::Chart", "Icon::ChartBar"),
    ],
};
