//! The product catalog: a grid with category badges, stock bars and an editor window.

use crate::can;
use crate::models::{Category, Product, ProductId};
use crate::store::Store;
use zeyn::prelude::*;

/// Products for the catalog grid.
#[action(permission = "admin.products.read")]
pub async fn search_products(q: DataQuery, store: Store) -> Result<Page<Product>> {
    store.find(&q).await
}

/// Creates a product (id 0) or saves one.
#[action(permission = "admin.products.write", audit)]
pub async fn save_product(input: Valid<Product>, store: Store, ctx: Ctx) -> Result<Product> {
    let product = input.into_inner();
    if product.id.0 != 0 {
        let before: Product = store.get(product.id.0).await?;
        ctx.record_changes(&before, &product);
    }
    store.save(product).await
}

/// The product catalog.
#[screen(route = "/products", permission = "admin.products.read")]
pub fn product_list(_: (), ctx: Ctx) -> Screen {
    let write = can(&ctx, "admin.products.write");
    let mut screen = Screen::new(t!("products.title"));
    if write {
        screen = screen.toolbar(|t| {
            t.button(t!("products.new"))
                .icon(Icon::Plus)
                .shortcut("Alt+N")
                .opens(product_form, ProductFormParams { id: None })
                .export(Export::Csv)
        });
    }
    let mut grid = DataGrid::of(search_products)
        .mode(DataMode::Server)
        .paging(Paging::pages(25))
        .column(Product::sku)
        .width(120)
        .filter()
        .column(Product::name)
        .flex(1)
        .filter()
        .column(Product::category)
        .width(140)
        .filter()
        .badge(|c| match c {
            Category::Hardware => Tone::Info,
            Category::Software => Tone::Brand,
            Category::Service => Tone::Success,
            Category::Supplies => Tone::Neutral,
        })
        .column(Product::price)
        .width(140)
        .filter()
        .column(Product::stock)
        .width(160)
        .filter()
        .sum()
        .progress(90)
        .column(Product::active)
        .width(100)
        .filter()
        .check_icon()
        .search()
        .filter_chips()
        .exportable()
        .row_style(|p| (p.stock < 10).then_some(RowStyle::Warning));
    if write {
        grid = grid.on_open(|p| Window::open(product_form, ProductFormParams { id: Some(p.id) }));
    }
    screen.body(grid)
}

/// Which product the editor changes; `None` creates one.
#[params]
pub struct ProductFormParams {
    pub id: Option<ProductId>,
}

/// The product editor window.
#[screen(permission = "admin.products.write")]
pub async fn product_form(p: ProductFormParams, store: Store) -> Result<Screen> {
    let product = match p.id {
        Some(id) => store.get(id.0).await?,
        None => Product {
            active: true,
            ..Product::default()
        },
    };
    Ok(Screen::window(t!("products.product"))
        .width(640)
        .body(
            Form::of(product)
                .row(|r| {
                    r.field(Product::sku)
                        .field(Product::category)
                })
                .field(Product::name)
                .row(|r| {
                    r.field(Product::price)
                        .field(Product::stock)
                })
                .field(Product::active)
                .submit(t!("products.save"), save_product)
                .on_success(|saved, ui| {
                    if let Ok(p) = saved {
                        ui.toast(t!("products.saved", sku = p.sku));
                    }
                    ui.close_and_refresh();
                }),
        ))
}

/// The catalog feature and its menu.
pub fn feature() -> Feature {
    Feature::new("products")
        .guard(SignedIn)
        .screen(product_list)
        .screen(product_form)
        .api(search_products)
        .api(save_product)
        .menu(|m| {
            m.group(t!("menu.catalog"), Icon::Box)
                .item(t!("products.title"), product_list)
                .icon(Icon::Tag)
        })
}
