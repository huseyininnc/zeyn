//! 0.0.6 removed the deprecated grid names.

use super::{Change, Step};

pub const STEP: Step = Step {
    version: "0.0.6",
    changes: &[
        Change::Rename("GridQuery", "DataQuery"),
        Change::Rename("GridMode", "DataMode"),
        Change::Rename("GridColumn", "DataGridColumn"),
        Change::Rename("Grid", "DataGrid"),
    ],
};
