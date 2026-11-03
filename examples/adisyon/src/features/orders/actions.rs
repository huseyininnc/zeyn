use super::models::{AddItem, DiningTable, DiningTableId, Payment};
use crate::features::kitchen::models::{OrderLine, OrderLineId};
use crate::restaurant::Restaurant;
use zeyn::prelude::*;

/// Every table with its status and open total.
#[action(permission = "pos.order")]
pub async fn find_tables(q: DataQuery, restaurant: Restaurant) -> Result<Page<DiningTable>> {
    Ok(q.apply(restaurant.tables()?))
}

/// Open order lines of every table; screens scope it to one table.
#[action(permission = "pos.order")]
pub async fn find_lines(q: DataQuery, restaurant: Restaurant) -> Result<Page<OrderLine>> {
    Ok(q.apply(restaurant.lines()?))
}

/// Adds one portion to a table's order.
#[action(permission = "pos.order")]
pub async fn add_item(input: AddItem, restaurant: Restaurant, ctx: Ctx) -> Result<OrderLine> {
    restaurant.add(input.table, input.item, ctx.now())
}

/// Takes back a line the kitchen has not seen yet.
#[action(permission = "pos.order")]
pub async fn remove_line(id: OrderLineId, restaurant: Restaurant) -> Result<()> {
    restaurant.remove(id)
}

/// Sends new lines to the kitchen and takes their portions out of stock.
#[action(permission = "pos.order", audit)]
pub async fn send_to_kitchen(
    table: DiningTableId,
    restaurant: Restaurant,
    ctx: Ctx,
) -> Result<i64> {
    restaurant.send(table, ctx.now())
}

/// Closes the bill and frees the table. Returns the change.
#[action(permission = "pos.cashier", audit)]
pub async fn take_payment(
    payment: Valid<Payment>,
    restaurant: Restaurant,
    ctx: Ctx,
) -> Result<Money> {
    let p = payment.into_inner();
    restaurant.pay(p.table, p.method, p.received, ctx.now(), ctx.locale())
}
