//! Desktop class shell: the command palette and the commands it runs.

use crate::stock::{ProductParams, product_editor, product_list, restock_low};
use zeyn::prelude::*;

/// Opens the product editor for a new product.
pub fn new_product() -> Command {
    Command::open("stock.new_product", t!("pro.cmd_new_product"), product_editor, ProductParams { id: None })
        .shortcut("Ctrl+Alt+N")
        .icon(Icon::Plus)
}

/// Restocks every product below 10 on the server.
pub fn restock() -> Command {
    Command::action("stock.restock_low", t!("pro.cmd_restock"), restock_low, ()).shortcut("Ctrl+Alt+R")
}

/// Goes to the product list.
pub fn go_products() -> Command {
    Command::navigate("stock.go_products", t!("pro.cmd_products"), product_list, ()).shortcut("Ctrl+Alt+P")
}

/// Shows a greeting; runs in the browser only.
pub fn hello() -> Command {
    Command::effect("pro.hello", t!("pro.cmd_hello"), Effect::Toast { message: t!("pro.hello") }).shortcut("Alt+H")
}

#[screen(route = "/palette", permission = "stock.product.read")]
pub fn palette() -> Screen {
    Screen::new(t!("pro.palette"))
        .toolbar(|t| t.button(t!("pro.tb_new")).icon(Icon::Plus).shortcut("Ctrl+Alt+E").opens(product_editor, ProductParams { id: None }))
        .context_menu(
            ContextMenu::new()
                .item(t!("pro.cmd_hello"), Effect::Toast { message: t!("pro.hello") })
                .action(t!("pro.cmd_restock"), restock_low, ()),
        )
        .body(
            Stack::new()
                .add(Alert::info(t!("pro.palette_intro")).title(t!("pro.palette_title")))
                .add(Alert::new(Tone::Neutral, t!("pro.palette_sources"))),
        )
}

pub fn feature() -> Feature {
    Feature::new("pro")
        .route_prefix("/pro")
        .guard(SignedIn)
        .screen(palette)
        .command(new_product())
        .command(restock())
        .command(go_products())
        .command(hello())
        .menu(|m| m.group(t!("menu.pro"), Icon::Search).item(t!("pro.palette"), palette))
}
