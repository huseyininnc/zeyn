//! Kitchen: the order lines waiting to be cooked, oldest first.

pub mod actions;
pub mod models;
pub mod screens;

use zeyn::prelude::*;

pub fn feature() -> Feature {
    Feature::new("kitchen")
        .screen(screens::kitchen)
        .api(actions::find_kitchen_lines)
        .menu(|m| {
            m.group(t!("kitchen.menu"), Icon::Cart)
                .item(t!("kitchen.title"), screens::kitchen)
        })
}
