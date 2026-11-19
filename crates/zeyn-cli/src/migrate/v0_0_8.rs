//! 0.0.8 renamed the lifetime of saved preferences.

use super::{Change, Step};

pub const STEP: Step = Step {
    version: "0.0.8",
    changes: &[Change::Rename("MAX_EMBED_CONSENT", "MAX_PREF_LIFETIME")],
};
