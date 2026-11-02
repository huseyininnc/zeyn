//! Stock: products with a paged list, filters, totals and an editor window with row versions.

use std::sync::{Arc, RwLock};
use zeyn::prelude::*;

/// A product kept in stock.
#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20, unique)]
    pub code: String,
    #[rule(required, max_len = 200)]
    pub name: String,
    pub unit: Unit,
    #[rule(min = 0)]
    pub price: Money,
    #[readonly]
    pub stock: i64,
    pub active: bool,
    #[version]
    pub version: i64,
}

/// Unit of measure.
#[choice]
pub enum Unit {
    Piece,
    Kg,
    Liter,
}

/// In memory product store. A real app keeps products in its database.
#[derive(Clone, Default)]
pub struct ProductStore(Arc<RwLock<Vec<Product>>>);

impl Service for ProductStore {}

fn poisoned<T>(_: T) -> Error {
    Error::Internal("product store lock is poisoned".into())
}

impl ProductStore {
    pub fn seeded() -> Self {
        let names = [
            "Black tea", "Green tea", "Arabica beans", "Olive oil", "Pine honey", "Hazelnuts", "Dried apricots",
            "Feta cheese", "Bulgur", "Red lentils", "Pomegranate syrup", "Tahini",
        ];
        let rows = (1..=240)
            .map(|i| Product {
                id: ProductId(i),
                code: format!("NW-{i:04}"),
                name: format!("{} {}", names[(i % 12) as usize], 100 + i * 5),
                unit: match i % 3 {
                    0 => Unit::Kg,
                    1 => Unit::Piece,
                    _ => Unit::Liter,
                },
                price: Money::tl(450 + (i * 7_919) % 250_000),
                stock: (i * 37) % 120,
                active: i % 11 != 0,
                version: 1,
            })
            .collect();
        ProductStore(Arc::new(RwLock::new(rows)))
    }

    pub fn all(&self) -> Result<Vec<Product>> {
        Ok(self.0.read().map_err(poisoned)?.clone())
    }

    pub fn get(&self, id: ProductId) -> Result<Product> {
        self.0.read().map_err(poisoned)?.iter().find(|p| p.id == id).cloned().ok_or(Error::NotFound)
    }

    pub fn save(&self, mut product: Product) -> Result<Product> {
        let mut rows = self.0.write().map_err(poisoned)?;
        if rows.iter().any(|p| p.code.eq_ignore_ascii_case(&product.code) && p.id != product.id) {
            let mut errors = ValidationErrors::default();
            errors.add(Product::code, t!("stock.code_taken", code = product.code));
            return Err(errors.into());
        }
        if product.id.0 == 0 {
            product.id = ProductId(rows.iter().map(|p| p.id.0).max().unwrap_or(0) + 1);
            product.version = 1;
            rows.push(product.clone());
            return Ok(product);
        }
        let current = rows.iter_mut().find(|p| p.id == product.id).ok_or(Error::NotFound)?;
        if current.version != product.version {
            return Err(Error::Conflict);
        }
        product.version += 1;
        *current = product.clone();
        Ok(product)
    }
}

/// Products for lists and the API: filter, sort and page.
#[action(permission = "stock.product.read")]
pub async fn search_products(q: GridQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

/// Creates or updates a product.
#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}

/// Which product the editor opens; `None` creates one.
#[params]
pub struct ProductParams {
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
                .opens(product_editor, ProductParams { id: None })
                .export(Export::Csv)
        })
        .body(
            Grid::of(search_products)
                .mode(GridMode::Server)
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
                .width(120)
                .sum()
                .column(Product::active)
                .width(90)
                .row_style(|p| match p.stock {
                    0 => Some(RowStyle::Danger),
                    1..=9 => Some(RowStyle::Warning),
                    _ => None,
                })
                .on_open(|p| Window::open(product_editor, ProductParams { id: Some(p.id) })),
        )
}

/// Product editor window.
#[screen(permission = "stock.product.write")]
pub fn product_editor(params: ProductParams, store: ProductStore) -> Result<Screen> {
    let product = match params.id {
        Some(id) => store.get(id)?,
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

/// The stock feature, its menu and its public API.
pub fn feature() -> Feature {
    Feature::new("stock")
        .route_prefix("/stock")
        .guard(SignedIn)
        .screen(product_list)
        .screen(product_editor)
        .api(search_products)
        .api(save_product)
        .menu(|m| m.group(t!("menu.stock"), Icon::Box).item(t!("stock.products"), product_list))
}
