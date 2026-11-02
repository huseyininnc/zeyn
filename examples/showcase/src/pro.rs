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

/// A shape on the designer canvas, edited in the property inspector.
#[model]
pub struct Shape {
    #[rule(required, max_len = 40)]
    pub name: String,
    #[rule(min = 0, max = 2000)]
    pub x: i64,
    #[rule(min = 0, max = 2000)]
    pub y: i64,
    #[rule(min = 1, max = 2000)]
    pub width: i64,
    #[rule(min = 1, max = 2000)]
    pub height: i64,
    pub visible: bool,
}

#[action(public)]
pub async fn save_shape(shape: Valid<Shape>) -> Result<Shape> {
    Ok(shape.into_inner())
}

/// The designer's menu bar: the same commands the palette lists.
fn menubar() -> MenuBar {
    MenuBar::new(t!("pro.menubar"))
        .menu(t!("pro.menu_file"), |m| m.command(new_product()).separator().command(restock()))
        .menu(t!("pro.menu_go"), |m| m.command(go_products()))
        .menu(t!("pro.menu_help"), |m| m.command(hello()))
}

#[screen(route = "/designer", permission = "stock.product.read")]
pub fn designer() -> Screen {
    let shape = Shape { name: "Header".into(), x: 40, y: 24, width: 640, height: 80, visible: true };
    Screen::new(t!("pro.designer")).body(
        Stack::new()
            .gap(2)
            .add(menubar())
            .add(
                Row::new()
                    .col(
                        Toolbox::new(t!("pro.tools"))
                            .tool("select", t!("pro.tool_select"), Some(Icon::Search))
                            .tool("draw", t!("pro.tool_draw"), Some(Icon::Edit))
                            .tool("box", t!("pro.tool_box"), Some(Icon::Box))
                            .tool("text", t!("pro.tool_text"), None),
                    )
                    .col(Card::new().title(t!("pro.canvas")).add(Alert::new(Tone::Neutral, t!("pro.canvas_hint"))))
                    .col(
                        PropertyInspector::new(
                            t!("pro.properties"),
                            Form::of(shape)
                                .section(t!("pro.section_general"))
                                .field(Shape::name)
                                .field(Shape::visible)
                                .section(t!("pro.section_position"))
                                .row(|r| r.field(Shape::x).field(Shape::y))
                                .section(t!("pro.section_size"))
                                .row(|r| r.field(Shape::width).field(Shape::height))
                                .submit(t!("pro.apply"), save_shape)
                                .on_success(|_, ui| ui.toast(t!("pro.applied"))),
                        ),
                    ),
            )
            .add(StatusBar::new(t!("pro.status")).left(t!("pro.status_ready")).live(t!("pro.status_hint")).right("100%").right("UTF-8")),
    )
}

pub fn feature() -> Feature {
    Feature::new("pro")
        .route_prefix("/pro")
        .guard(SignedIn)
        .screen(palette)
        .screen(designer)
        .command(new_product())
        .command(restock())
        .command(go_products())
        .command(hello())
        .menu(|m| m.group(t!("menu.pro"), Icon::Search).item(t!("pro.palette"), palette).item(t!("pro.designer"), designer))
}
