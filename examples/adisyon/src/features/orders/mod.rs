//! Orders: the floor with every table, a table's order, sending it to the kitchen and payment.

pub mod actions;
pub mod models;
pub mod screens;

use zeyn::prelude::*;

pub fn feature() -> Feature {
    Feature::new("orders")
        .screen(screens::floor)
        .screen(screens::order)
        .screen(screens::payment)
        .api(actions::find_tables)
        .menu(|m| m.group(t!("orders.menu"), Icon::Table).item(t!("floor.title"), screens::floor))
}
