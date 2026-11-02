use super::actions::{find_kitchen_lines, mark_ready};
use super::models::OrderLine;
use zeyn::prelude::*;

/// The kitchen display: what to cook, for which table, and how long it has been waiting.
#[screen(route = "/kitchen", permission = "pos.kitchen")]
pub fn kitchen() -> Screen {
    Screen::new(t!("kitchen.title")).body(
        DataGrid::of(find_kitchen_lines)
            .mode(DataMode::Server)
            .column(OrderLine::table_name)
            .column(OrderLine::item)
            .column(OrderLine::quantity)
            .sum()
            .column(OrderLine::ordered)
            .relative_time()
            .row_action(t!("kitchen.ready"), mark_ready, |l| l.id),
    )
}
