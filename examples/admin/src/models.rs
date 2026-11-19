//! The records of the admin app. Each entity is one table; see `migrations/`.

use crate::roles::Role;
use zeyn::prelude::*;

/// A customer who places orders.
#[entity(table = "customers")]
pub struct Customer {
    #[key]
    pub id: CustomerId,
    #[rule(required, max_len = 120)]
    pub name: String,
    #[rule(required, email, max_len = 200)]
    pub email: String,
    #[rule(max_len = 80)]
    pub city: String,
    pub segment: Segment,
    pub active: bool,
    #[readonly]
    pub since: Date,
    #[version]
    pub version: i64,
}

/// How a customer buys.
#[choice]
pub enum Segment {
    Retail,
    Business,
    Partner,
}

/// A product of the catalog.
#[entity(table = "products")]
pub struct Product {
    #[key]
    pub id: ProductId,
    #[rule(required, max_len = 20)]
    pub sku: String,
    #[rule(required, max_len = 200)]
    pub name: String,
    pub category: Category,
    #[rule(min = 0)]
    pub price: Money,
    #[rule(min = 0)]
    pub stock: i64,
    pub active: bool,
    #[version]
    pub version: i64,
}

/// Product category.
#[choice]
pub enum Category {
    Hardware,
    Software,
    Service,
    Supplies,
}

/// An order of a customer.
#[entity(table = "orders")]
pub struct Order {
    #[key]
    pub id: OrderId,
    #[readonly]
    #[rule(max_len = 20)]
    pub number: String,
    #[readonly]
    pub customer_id: i64,
    #[readonly]
    #[rule(max_len = 120)]
    pub customer_name: String,
    #[readonly]
    pub placed: Date,
    pub status: OrderStatus,
    #[readonly]
    pub items: i64,
    #[readonly]
    pub total: Money,
    #[rule(max_len = 500)]
    pub notes: Option<String>,
    #[version]
    pub version: i64,
}

/// Where an order stands.
#[choice]
pub enum OrderStatus {
    New,
    Processing,
    Shipped,
    Delivered,
    Cancelled,
}

impl OrderStatus {
    /// Statuses of orders that still need work.
    pub const OPEN: [OrderStatus; 2] = [OrderStatus::New, OrderStatus::Processing];

    /// The tone the status is shown in.
    pub fn tone(self) -> Tone {
        match self {
            OrderStatus::New => Tone::Info,
            OrderStatus::Processing => Tone::Warning,
            OrderStatus::Shipped => Tone::Brand,
            OrderStatus::Delivered => Tone::Success,
            OrderStatus::Cancelled => Tone::Neutral,
        }
    }
}

/// One entry of an order's history: created, saved, status changed.
#[entity(table = "order_events")]
pub struct OrderEvent {
    #[key]
    pub id: OrderEventId,
    pub order_id: i64,
    pub at: DateTime,
    #[rule(max_len = 120)]
    pub user_name: String,
    #[rule(max_len = 20)]
    pub kind: String,
    #[rule(max_len = 2000)]
    pub detail: String,
    #[version]
    pub version: i64,
}

/// A person who signs in to the admin app.
#[entity(table = "users")]
pub struct User {
    #[key]
    pub id: UserId,
    #[rule(required, max_len = 120)]
    pub name: String,
    #[rule(required, email, max_len = 200)]
    pub email: String,
    pub role: Role,
    pub active: bool,
    #[version]
    pub version: i64,
}

/// The settings of the company, one row.
#[entity(table = "settings")]
pub struct Settings {
    #[key]
    pub id: SettingsId,
    #[rule(required, max_len = 120)]
    pub company: String,
    #[rule(required, email, max_len = 200)]
    pub support_email: String,
    #[rule(min = 1, max = 365)]
    pub ship_within_days: i64,
    #[rule(min = 0, max = 1000)]
    pub low_stock: i64,
    pub notify_new_orders: bool,
    pub weekly_report: bool,
    #[version]
    pub version: i64,
}
