//! Menu: what the restaurant sells, its prices and the portions left in stock.

pub mod actions;
pub mod models;
pub mod screens;

use zeyn::prelude::*;

pub fn feature() -> Feature {
    Feature::new("menu")
        .screen(screens::menu_list)
        .api(actions::find_menu)
        .menu(|m| {
            m.group(t!("menu.menu"), Icon::Box)
                .item(t!("menu.list"), screens::menu_list)
        })
}
