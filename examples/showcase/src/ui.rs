//! Component library tour: layout containers, editors and display components.

use crate::stock::{Product, search_products};
use zeyn::prelude::*;

fn products() -> Node {
    Grid::of(search_products)
        .paging(Paging::pages(8))
        .column(Product::code)
        .width(110)
        .filter()
        .column(Product::name)
        .flex(1)
        .column(Product::price)
        .width(130)
        .into()
}

#[screen(route = "/layout", permission = "stock.product.read")]
pub fn layout() -> Screen {
    Screen::new(t!("ui.layout")).body(
        Stack::new()
            .add(
                Row::new()
                    .col(Card::new().title(t!("ui.card_orders")).add(Card::new().title(t!("ui.card_hint"))))
                    .col(Card::new().title(t!("ui.card_stock")))
                    .col(Card::new().title(t!("ui.card_people"))),
            )
            .add(
                Panel::new(t!("ui.panel_products"))
                    .collapsible()
                    .toolbar(|t| t.export(Export::Csv))
                    .add(
                        Tabs::new(t!("ui.tabs_label"))
                            .tab(t!("ui.tab_all"), products())
                            .tab(t!("ui.tab_split"), Split::new(products(), Card::new().title(t!("ui.card_detail"))).ratio(55))
                            .tab(t!("ui.tab_columns"), Row::new().col(products()).weight(2).col(Card::new().title(t!("ui.card_hint")))),
                    ),
            )
            .add(Panel::new(t!("ui.panel_collapsed")).collapsed().add(Card::new().title(t!("ui.card_hint")))),
    )
}

pub fn feature() -> Feature {
    Feature::new("ui")
        .route_prefix("/ui")
        .guard(SignedIn)
        .screen(layout)
        .menu(|m| m.group(t!("menu.ui"), Icon::Table).item(t!("ui.layout"), layout))
}
