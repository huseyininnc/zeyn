use super::models::{Product, ProductId, Unit};
use std::sync::{Arc, RwLock};
use zeyn::prelude::*;
#[cfg(any(feature = "postgres", feature = "oracle"))]
use zeyn::sql::{Db, PoolLimits};

/// Where products are kept: in memory (the default, sample data) or in PostgreSQL or Oracle when
/// `ZEYN_DATABASE_URL` is set and the example is built with the `postgres` or `oracle` feature.
#[derive(Clone)]
pub enum ProductStore {
    Memory(Arc<RwLock<Vec<Product>>>),
    #[cfg(any(feature = "postgres", feature = "oracle"))]
    Sql(Db),
}

impl Service for ProductStore {}

fn poisoned<T>(_: T) -> Error {
    Error::Internal("product store lock is poisoned".into())
}

impl ProductStore {
    /// Store filled with sample products.
    pub fn seeded() -> Self {
        let names = [
            "Black tea",
            "Green tea",
            "Coffee beans",
            "Olive oil",
            "Honey",
            "Hazelnuts",
            "Dried apricots",
            "Feta cheese",
        ];
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
        ProductStore::Memory(Arc::new(RwLock::new(products)))
    }

    #[cfg(any(feature = "postgres", feature = "oracle"))]
    /// Database store. On PostgreSQL the migrations in `migrations/` create and fill the table;
    /// on Oracle the table is created from the entity and filled with the sample products the
    /// first time.
    pub async fn connect(url: &str) -> Result<Self> {
        let db = Db::connect(url, PoolLimits::default()).await?;
        if db.dialect().name() == "postgres" {
            db.migrate_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations")).await?;
        } else if db.find::<Product>(&DataQuery::default()).await.is_err() {
            db.create_table::<Product>().await?;
            let ProductStore::Memory(samples) = ProductStore::seeded() else { unreachable!() };
            let samples = samples.read().map_err(poisoned)?.clone();
            let mut tx = db.begin().await?;
            for p in samples {
                tx.insert(&Product { id: ProductId(0), ..p }).await?;
            }
            tx.execute("UPDATE products SET stock = MOD(id * 7, 40)", vec![]).await?;
            tx.commit().await?;
        }
        Ok(ProductStore::Sql(db))
    }

    /// Filters, sorts and pages products: in SQL, or in memory for the sample store.
    pub async fn search(&self, q: &DataQuery) -> Result<Page<Product>> {
        match self {
            ProductStore::Memory(rows) => Ok(q.apply(rows.read().map_err(poisoned)?.clone())),
            #[cfg(any(feature = "postgres", feature = "oracle"))]
            ProductStore::Sql(db) => db.find(q).await,
        }
    }

    pub async fn get(&self, id: ProductId) -> Result<Product> {
        match self {
            ProductStore::Memory(rows) => {
                rows.read().map_err(poisoned)?.iter().find(|p| p.id == id).cloned().ok_or(Error::NotFound)
            }
            #[cfg(any(feature = "postgres", feature = "oracle"))]
            ProductStore::Sql(db) => db.get(id).await,
        }
    }

    /// Inserts a new product (id 0) or updates one. Fails with `Conflict` when the row version
    /// sent by the user is not the stored one, and with a validation error when the code is taken.
    /// The stock is read only, so it is never taken from the form.
    pub async fn save(&self, product: Product) -> Result<Product> {
        match self {
            ProductStore::Memory(rows) => {
                let mut rows = rows.write().map_err(poisoned)?;
                save_in_memory(&mut rows, product)
            }
            #[cfg(any(feature = "postgres", feature = "oracle"))]
            ProductStore::Sql(db) if product.id.0 == 0 => db.insert(&product).await,
            #[cfg(any(feature = "postgres", feature = "oracle"))]
            ProductStore::Sql(db) => db.update(&product).await,
        }
    }
}

fn save_in_memory(rows: &mut Vec<Product>, mut product: Product) -> Result<Product> {
    if rows.iter().any(|p| p.code.eq_ignore_ascii_case(&product.code) && p.id != product.id) {
        let mut errors = ValidationErrors::default();
        errors.add(Product::code, t!("stock.code_taken", code = product.code));
        return Err(errors.into());
    }
    if product.id.0 == 0 {
        product.id = ProductId(rows.iter().map(|p| p.id.0).max().unwrap_or(0) + 1);
        product.version = 1;
        product.stock = 0;
        rows.push(product.clone());
        return Ok(product);
    }
    let current = rows.iter_mut().find(|p| p.id == product.id).ok_or(Error::NotFound)?;
    if current.version != product.version {
        return Err(Error::Conflict);
    }
    product.version += 1;
    product.stock = current.stock;
    *current = product.clone();
    Ok(product)
}
