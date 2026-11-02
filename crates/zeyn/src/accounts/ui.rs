//! The default UI.

use crate::feature::Feature;

/// Builder of the default UI.
pub struct AccountsUi;

pub fn ui() -> AccountsUi {
    AccountsUi
}

pub fn feature() -> Feature {
    Feature::new("accounts")
}
