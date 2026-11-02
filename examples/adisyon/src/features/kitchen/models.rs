use crate::features::menu::models::MenuItemId;
use crate::features::orders::models::DiningTableId;
use zeyn::prelude::*;

/// Portions of one menu item on a table's open order.
#[entity(table = "order_lines")]
pub struct OrderLine {
    #[key]
    pub id: OrderLineId,
    pub table: DiningTableId,
    pub table_name: String,
    pub item_id: MenuItemId,
    pub item: String,
    pub quantity: i64,
    pub unit_price: Money,
    pub amount: Money,
    pub status: LineStatus,
    pub ordered: DateTime,
}

/// Where a line is between the waiter and the guest.
#[choice]
pub enum LineStatus {
    New,
    Sent,
    Ready,
}
