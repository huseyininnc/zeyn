use zeyn::prelude::*;

/// A table on the floor with the state of its open order.
#[entity(table = "dining_tables")]
pub struct DiningTable {
    #[key]
    pub id: DiningTableId,
    pub name: String,
    pub seats: i64,
    #[readonly]
    pub status: TableStatus,
    #[readonly]
    pub open_total: Money,
}

/// What is happening at a table.
#[choice]
pub enum TableStatus {
    Free,
    Ordering,
    Kitchen,
    Served,
}

/// Which table's order a screen shows.
#[params]
pub struct TableParams {
    pub table: DiningTableId,
}

/// One portion of a menu item for a table. Built on the server; never read from the client.
#[model]
pub struct AddItem {
    pub table: DiningTableId,
    pub item: crate::features::menu::models::MenuItemId,
}

/// Closing a bill. The table comes from the server; the cashier picks the method and amount.
#[model]
pub struct Payment {
    pub table: DiningTableId,
    pub method: PaymentMethod,
    #[rule(min = 0)]
    pub received: Money,
}

/// How the guest pays.
#[choice]
pub enum PaymentMethod {
    Cash,
    Card,
}

/// A closed bill.
#[entity(table = "sales")]
pub struct Sale {
    #[key]
    pub id: SaleId,
    pub table: String,
    pub at: DateTime,
    pub method: PaymentMethod,
    pub items: i64,
    pub total: Money,
}
