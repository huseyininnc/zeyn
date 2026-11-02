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
