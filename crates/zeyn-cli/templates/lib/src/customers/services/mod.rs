//! Services of the feature: stores and clients that read and write data. Apps register each
//! one with `.service(...)`.

mod customer_store;
pub use customer_store::CustomerStore;
