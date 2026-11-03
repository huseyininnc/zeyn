//! Stock: products, their list and their editor.

pub mod actions;
pub mod models;
pub mod screens;
pub mod store;

use screens::{product_form, product_list};
use zeyn::prelude::*;

/// The stock feature: routes under `/stock`, signed in users only.
pub fn feature() -> Feature {
    Feature::new("stock")
        .route_prefix("/stock")
        .guard(SignedIn)
        .screen(product_list)
        .screen(product_form)
        .menu(|m| {
            m.group(t!("stock.menu"), Icon::Box)
                .item(t!("stock.products"), product_list)
        })
}
