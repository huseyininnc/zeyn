//! Every rule the compiler enforces, as a `compile_fail` doctest. The module exists only when
//! rustdoc runs doctests (`cfg(doctest)`), so these examples are tested but never shown on
//! docs.rs.
//!
//!
//! An action without access:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[action]
//! pub async fn delete_product(_id: i64) -> Result<()> { Ok(()) }
//! ```
//!
//! A shared cache on a screen that is not public, and one longer than a day:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[screen(permission = "stock.product.read", cache = 300)]
//! pub fn products() -> Screen { Screen::new("Products") }
//! ```
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[screen(public, cache = 100000)]
//! pub fn about() -> Screen { Screen::new("About") }
//! ```
//!
//! A text rule on a field that is not text:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[model]
//! pub struct Filter {
//!     #[rule(max_len = 5)]
//!     pub active: bool,
//! }
//! ```
//!
//! A form whose submit action does not take `Valid<M>`:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! fn form() -> Node {
//!     Form::of(Product::default()).field(Product::code).submit("Save", search_products).into()
//! }
//! ```
//!
//! A route segment that names no field of the screen's params:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[params]
//! pub struct OrderParams {
//!     pub id: i64,
//! }
//!
//! #[screen(route = "/orders/{order_id}", public)]
//! pub fn order(_p: OrderParams) -> Screen { Screen::new("Order") }
//! ```
//!
//! A route with segments on a screen that takes no params:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[screen(route = "/orders/{id}", public)]
//! pub fn order() -> Screen { Screen::new("Order") }
//! ```
//!
//! An error screen that takes params (`App::not_found` and `App::server_error` take screens
//! without params, since an error page learns nothing about the request):
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[params]
//! pub struct OrderParams {
//!     pub id: i64,
//! }
//!
//! #[screen(route = "/orders/{id}", public)]
//! pub fn order(_p: OrderParams) -> Screen { Screen::new("Order") }
//!
//! let app = App::new("Shop").not_found(order);
//! ```
//!
//! A total on a text column:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).sum().into() }
//! ```
//!
//! An average on a text column, and a minimum on a yes/no column:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).avg().into() }
//! ```
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub active: bool,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::active).min().into() }
//! ```
//!
//! A badge on a column that is not a `#[choice]` enum:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//! use zeyn::Tone;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).badge(|_| Tone::Info).into() }
//! ```
//!
//! A row action whose input is not built from the row the way the action takes it:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! #[action(permission = "stock.product.write")]
//! pub async fn archive_product(_id: ProductId) -> Result<()> { Ok(()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).row_action("Archive", archive_product, |p| p.code.clone()).into() }
//! ```
//!
//! A bulk action whose input does not hold keys of the given key field:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! #[action(permission = "stock.product.write")]
//! pub async fn archive_products(_codes: Vec<String>) -> Result<()> { Ok(()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).bulk_action("Archive", archive_products, Product::id).into() }
//! ```
//!
//! An inline edit save action that does not take `Valid<M>`:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[entity(table = "products")]
//! pub struct Product {
//!     #[key]
//!     pub id: ProductId,
//!     pub code: String,
//! }
//!
//! #[action(permission = "stock.product.read")]
//! pub async fn search_products(_q: DataQuery) -> Result<Page<Product>> { Ok(Page::default()) }
//!
//! #[action(permission = "stock.product.write")]
//! pub async fn save_code(_p: Product) -> Result<()> { Ok(()) }
//!
//! fn grid() -> Node { DataGrid::of(search_products).column(Product::code).editable().edit_with(save_code).into() }
//! ```
//!
//! A menu item for a screen without a route:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[screen(permission = "stock.product.write")]
//! pub fn product_form() -> Screen { Screen::window("Product") }
//!
//! fn feature() -> Feature {
//!     Feature::new("stock").screen(product_form).menu(|m| m.group("Stock", Icon::Box).item("Product", product_form))
//! }
//!
//! fn main() {
//!     let _ = feature();
//! }
//! ```
//!
//! This check runs when the code is generated, so `cargo check` alone may not report it.
//! [`Feature::check`] reports the same problem at startup.
//!
//! A key on a model that is not an entity:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[model]
//! pub struct Search {
//!     #[key]
//!     pub id: i64,
//! }
//! ```
//!
//! A `scale` rule on a field that is not a `Decimal`:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[model]
//! pub struct Price {
//!     #[rule(scale = 2)]
//!     pub amount: i64,
//! }
//! ```
//!
//! A `Vec` of something that is neither a line model nor a `#[choice]` enum:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[model]
//! pub struct Tags {
//!     pub names: Vec<String>,
//! }
//! ```
//!
//! A `mask` rule on a field that is not text:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[model]
//! pub struct Contact {
//!     #[rule(mask = "(999) 999 99 99")]
//!     pub phone: i64,
//! }
//! ```
//!
//! A mask on a field that is not text:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[model]
//! pub struct Card {
//!     #[mask(last4)]
//!     pub number: i64,
//! }
//! ```
//!
//! An unknown action option (the AI options are `ai = "..."`, `ai = false` and `confirm`):
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[action(permission = "stock.product.write", ai_hidden)]
//! pub async fn archive_product(_code: String) -> Result<()> { Ok(()) }
//! ```
//!
//! `ai = true`, which is already the default:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[action(permission = "stock.product.write", ai = true)]
//! pub async fn archive_product(_code: String) -> Result<()> { Ok(()) }
//! ```
//!
//! Empty guidance for models:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[action(permission = "stock.product.write", ai = " ")]
//! pub async fn archive_product(_code: String) -> Result<()> { Ok(()) }
//! ```
//!
//! `confirm` on an action that models never see:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[action(permission = "stock.product.write", ai = false, confirm)]
//! pub async fn archive_product(_code: String) -> Result<()> { Ok(()) }
//! ```
//!
//! `confirm` on a screen instead of the action that writes:
//!
//! ```compile_fail
//! use zeyn::prelude::*;
//!
//! #[screen(route = "/products", permission = "stock.product.read", confirm)]
//! pub fn product_list() -> Screen { Screen::new("Products") }
//! ```

/// Rules of the data layer, tested when a database feature is on.
#[cfg(any(feature = "postgres", feature = "oracle"))]
mod sql {
    //! SQL built at run time for `Db::query`: the statement must be a literal, values go in `args`.
    //!
    //! ```compile_fail
    //! use zeyn::prelude::*;
    //!
    //! #[model]
    //! pub struct Hit {
    //!     pub id: i64,
    //! }
    //!
    //! async fn search(db: zeyn::sql::Db, q: String) -> Result<Vec<Hit>> {
    //!     let sql = format!("SELECT id FROM items WHERE title = '{q}'");
    //!     db.query(&sql, vec![]).await
    //! }
    //! ```
    //!
    //! The same with an owned `String` on a transaction:
    //!
    //! ```compile_fail
    //! use zeyn::prelude::*;
    //!
    //! #[model]
    //! pub struct Hit {
    //!     pub id: i64,
    //! }
    //!
    //! async fn search(mut tx: zeyn::sql::Tx, sql: String) -> Result<Option<Hit>> {
    //!     tx.query_one(sql, vec![]).await
    //! }
    //! ```
    //!
    //! The literal form compiles:
    //!
    //! ```
    //! use zeyn::prelude::*;
    //!
    //! #[model]
    //! pub struct Hit {
    //!     pub id: i64,
    //! }
    //!
    //! async fn search(db: zeyn::sql::Db, q: String) -> Result<Vec<Hit>> {
    //!     db.query("SELECT id FROM items WHERE title = $1", vec![q.into()]).await
    //! }
    //! ```
}
