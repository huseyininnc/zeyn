use super::models::{LineStatus, OrderLine, OrderLineId};
use crate::restaurant::Restaurant;
use zeyn::prelude::*;

/// Lines the kitchen still has to cook. Other lines never leave the server for kitchen staff.
#[action(permission = "pos.kitchen")]
pub async fn find_kitchen_lines(q: DataQuery, restaurant: Restaurant) -> Result<Page<OrderLine>> {
    let mut lines: Vec<OrderLine> = restaurant.lines()?.into_iter().filter(|l| l.status == LineStatus::Sent).collect();
    lines.sort_by_key(|l| l.ordered);
    Ok(q.apply(lines))
}

/// Marks a line as cooked and ready to serve.
#[action(permission = "pos.kitchen")]
pub async fn mark_ready(id: OrderLineId, restaurant: Restaurant) -> Result<()> {
    restaurant.ready(id)
}
