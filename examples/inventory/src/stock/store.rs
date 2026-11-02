use super::models::{Product, ProductId, Unit};
use std::sync::{Arc, RwLock};
use zeyn::prelude::*;

/// In memory product store with row versions. A real app keeps products in the database.
#[derive(Clone, Default)]
pub struct ProductStore(Arc<RwLock<Vec<Product>>>);

impl Service for ProductStore {}

fn poisoned<T>(_: T) -> Error {
    Error::Internal("product store lock is poisoned".into())
}

impl ProductStore {
    /// Store filled with sample products.
    pub fn seeded() -> Self {
        let names = ["Black tea", "Green tea", "Coffee beans", "Olive oil", "Honey", "Hazelnuts", "Dried apricots", "Feta cheese"];
        let products = (1..=120)
            .map(|i| Product {
                id: ProductId(i),
                code: format!("P-{i:04}"),
                name: format!("{} {}", names[(i % 8) as usize], i),
                unit: match i % 3 {
                    0 => Unit::Kg,
                    1 => Unit::Piece,
                    _ => Unit::Liter,
                },
                price: Money::tl(i * 1_275),
                stock: (i * 7) % 40,
                active: i % 9 != 0,
                version: 1,
            })
            .collect();
        ProductStore(Arc::new(RwLock::new(products)))
    }

    pub fn all(&self) -> Result<Vec<Product>> {
        Ok(self.0.read().map_err(poisoned)?.clone())
    }

    pub fn get(&self, id: ProductId) -> Result<Product> {
        self.0.read().map_err(poisoned)?.iter().find(|p| p.id == id).cloned().ok_or(Error::NotFound)
    }

    /// Inserts a new product (id 0) or updates one. Fails with `Conflict` when the row version
    /// sent by the user is not the stored one, and with a validation error when the code is taken.
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
