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

    /// Adds ten units to a product's stock.
    pub fn restock(&self, id: ProductId) -> Result<()> {
        let mut rows = self.0.write().map_err(poisoned)?;
        let product = rows.iter_mut().find(|p| p.id == id).ok_or(Error::NotFound)?;
        product.stock += 10;
        product.version += 1;
        Ok(())
    }

    /// Marks products as inactive. Returns how many changed.
    pub fn deactivate(&self, ids: &[ProductId]) -> Result<usize> {
        let mut rows = self.0.write().map_err(poisoned)?;
        let mut n = 0;
        for p in rows.iter_mut().filter(|p| ids.contains(&p.id) && p.active) {
            p.active = false;
            p.version += 1;
            n += 1;
        }
        Ok(n)
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
pub async fn search_products(q: DataQuery, store: ProductStore) -> Result<Page<Product>> {
    Ok(q.apply(store.all()?))
}

/// Creates or updates a product.
#[action(permission = "stock.product.write", audit)]
pub async fn save_product(product: Valid<Product>, store: ProductStore) -> Result<Product> {
    store.save(product.into_inner())
}

/// Adds ten units to a product's stock. A row action on the product list.
#[action(permission = "stock.product.write", audit)]
pub async fn restock_product(id: ProductId, store: ProductStore) -> Result<()> {
    store.restock(id)
}

/// Marks the selected products as inactive. A bulk action on the product list.
#[action(permission = "stock.product.write", audit)]
pub async fn deactivate_products(ids: Vec<ProductId>, store: ProductStore) -> Result<usize> {
    store.deactivate(&ids)
}

/// Archives a product by its code: it stays in reports but can no longer be sold.
#[action(permission = "stock.product.write", audit, confirm, ai = "Products are named by codes such as NW-0003.")]
pub async fn archive_product(code: String, store: ProductStore) -> Result<()> {
    let product = store.all()?.into_iter().find(|p| p.code.eq_ignore_ascii_case(code.trim())).ok_or(Error::NotFound)?;
    store.deactivate(&[product.id]).map(|_| ())
}

/// Adds ten units to every product with fewer than ten. From the product list's context menu.
#[action(permission = "stock.product.write", audit)]
pub async fn restock_low(_: (), store: ProductStore) -> Result<()> {
    for p in store.all()?.into_iter().filter(|p| p.stock < 10) {
        store.restock(p.id)?;
    }
    Ok(())
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
        .context_menu(
            ContextMenu::new()
                .item(t!("stock.new_product"), Window::open(product_editor, ProductParams { id: None }))
                .icon(Icon::Plus)
                .shortcut("Ctrl+N")
                .action(t!("stock.restock_low"), restock_low, ()),
        )
        .toolbar(|t| {
            t.button(t!("stock.new_product"))
                .icon(Icon::Plus)
                .shortcut("Ctrl+N")
                .opens(product_editor, ProductParams { id: None })
                .export(Export::Csv)
        })
        .body(
            DataGrid::of(search_products)
                .mode(DataMode::Server)
                .paging(Paging::pages(25))
                .column(Product::code)
                .width(120)
                .pin_start()
                .filter()
                .link(|p| Window::open(product_editor, ProductParams { id: Some(p.id) }))
                .column(Product::id)
                .width(90)
                .hidden()
                .column(Product::name)
                .flex(1)
                .filter()
                .editable()
                .column(Product::unit)
                .width(130)
                .filter()
                .editable()
                .badge(|u| match u {
                    Unit::Piece => Tone::Neutral,
                    Unit::Kg => Tone::Info,
                    Unit::Liter => Tone::Brand,
                })
                .column(Product::price)
                .width(220)
                .filter()
                .editable()
                .avg()
                .max()
                .column(Product::stock)
                .width(170)
                .filter()
                .sum()
                .progress(120)
                .column(Product::active)
                .width(110)
                .filter()
                .check_icon()
                .editable()
                .edit_with(save_product)
                .exportable()
                .row_action(t!("stock.restock"), restock_product, |p| p.id)
                .bulk_action(t!("stock.deactivate"), deactivate_products, Product::id)
                .row_menu(|p| {
                    ContextMenu::new()
                        .item(t!("stock.edit"), Window::open(product_editor, ProductParams { id: Some(p.id) }))
                        .icon(Icon::Edit)
                        .shortcut("Enter")
                        .action(t!("stock.restock"), restock_product, p.id)
                        .separator()
                        .action(t!("stock.deactivate"), deactivate_products, vec![p.id])
                        .disabled(!p.active)
                })
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
        .api(restock_product)
        .api(archive_product)
        .menu(|m| m.group(t!("menu.stock"), Icon::Box).item(t!("stock.products"), product_list))
}
