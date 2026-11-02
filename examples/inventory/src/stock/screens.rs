use super::actions::{save_product, search_products};
use super::models::{Product, ProductId};
use super::store::ProductStore;
use zeyn::prelude::*;

/// Which product the form edits; `None` creates a new one.
#[params]
pub struct ProductFormParams {
    pub id: Option<ProductId>,
}

/// Product list.
#[screen(route = "/products", permission = "stock.product.read")]
pub fn product_list() -> Screen {
    Screen::new(t!("stock.products"))
        .toolbar(|t| {
            t.button(t!("stock.new_product"))
                .icon(Icon::Plus)
                .shortcut("Ctrl+N")
                .opens(product_form, ProductFormParams { id: None })
                .export(Export::Csv)
        })
        .body(
            DataGrid::of(search_products)
                .mode(DataMode::Server)
                .paging(Paging::pages(25))
                .column(Product::code)
                .width(120)
                .filter()
                .column(Product::name)
                .flex(1)
                .filter()
                .column(Product::unit)
                .width(120)
                .column(Product::price)
                .width(140)
                .column(Product::stock)
                .width(110)
                .sum()
                .column(Product::active)
                .width(90)
                .row_style(|p| (p.stock == 0).then_some(RowStyle::Warning))
                .on_open(|p| Window::open(product_form, ProductFormParams { id: Some(p.id) })),
        )
}

/// Product editor window.
#[screen(permission = "stock.product.write")]
pub async fn product_form(params: ProductFormParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id).await?,
        None => Product { active: true, ..Product::default() },
    };
    Ok(Screen::window(t!("stock.product")).width(640).body(
        Form::of(product)
            .row(|r| r.field(Product::code).field(Product::unit))
            .field(Product::name)
            .row(|r| r.field(Product::price).field(Product::active))
            .submit(t!("stock.save"), save_product)
            .on_success(|saved, ui| {
                if let Ok(p) = saved {
                    ui.toast(t!("stock.saved", code = p.code));
                }
                ui.close_and_refresh();
            }),
    ))
}
