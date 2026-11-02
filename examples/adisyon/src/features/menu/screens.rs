use super::actions::{find_menu, save_menu_item};
use super::models::{Category, MenuItem};
use zeyn::prelude::*;

/// The menu with prices and stock, edited in place. Rows close to running out are highlighted.
#[screen(route = "/menu", permission = "pos.menu")]
pub fn menu_list() -> Screen {
    Screen::new(t!("menu.list")).toolbar(|t| t.export(Export::Csv)).body(
        DataGrid::of(find_menu)
            .mode(DataMode::Server)
            .exportable()
            .row_style(|m: &MenuItem| (m.stock < 5).then_some(RowStyle::Warning))
            .column(MenuItem::name)
            .filter()
            .column(MenuItem::category)
            .filter()
            .badge(|c| match c {
                Category::Soups => Tone::Warning,
                Category::Mains => Tone::Danger,
                Category::Desserts => Tone::Brand,
                Category::Drinks => Tone::Info,
            })
            .column(MenuItem::price)
            .editable()
            .column(MenuItem::stock)
            .editable()
            .sum()
            .column(MenuItem::active)
            .editable()
            .check_icon()
            .edit_with(save_menu_item),
    )
}
