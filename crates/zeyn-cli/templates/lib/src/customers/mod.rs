//! Customers: a shared entity with its services, actions, screens and feature.

pub mod actions;
pub mod models;
pub mod screens;
pub mod services;

pub use services::CustomerStore;

use zeyn::prelude::*;

/// The customers feature. Apps add it with `.feature(customers::feature())`.
pub fn feature() -> Feature {
    Feature::new("customers")
        .route_prefix("/customers")
        .guard(SignedIn)
        .screen(screens::customer_list)
        .screen(screens::customer_editor)
        .api(actions::search_customers)
        .menu(|m| {
            m.group(t!("customers.menu"), Icon::Box)
                .item(t!("customers.list"), screens::customer_list)
        })
}
