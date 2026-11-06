//! Reports: closed bills and revenue per menu category.

use crate::features::orders::models::{PaymentMethod, Sale};
use crate::restaurant::Restaurant;
use zeyn::prelude::*;

/// Closed bills, newest first.
#[action(permission = "pos.reports")]
pub async fn find_sales(q: DataQuery, restaurant: Restaurant) -> Result<Page<Sale>> {
    let mut sales = restaurant.sales()?;
    sales.reverse();
    Ok(q.apply(sales))
}

/// Revenue by category and the list of closed bills.
#[screen(route = "/reports", permission = "pos.reports")]
pub fn reports(_: (), restaurant: Restaurant) -> Result<Screen> {
    let revenue = restaurant.revenue_by_category()?;
    let chart = Chart::bar(t!("reports.by_category"))
        .labels(
            revenue
                .iter()
                .map(|(c, _)| Text::key(format!("Category.{c:?}"))),
        )
        .series(t!("reports.revenue"), revenue.iter().map(|(_, v)| *v))
        .decimals(2);
    Ok(Screen::new(t!("reports.title")).body(
        Stack::new().add(chart).add(
            DataGrid::of(find_sales)
                .mode(DataMode::Server)
                .column(Sale::at)
                .column(Sale::table)
                .column(Sale::method)
                .badge(|m| {
                    if *m == PaymentMethod::Card {
                        Tone::Info
                    } else {
                        Tone::Success
                    }
                })
                .column(Sale::items)
                .sum()
                .column(Sale::total)
                .sum(),
        ),
    ))
}

pub fn feature() -> Feature {
    Feature::new("reports")
        .screen(reports)
        .api(find_sales)
        .menu(|m| {
            m.group(t!("reports.menu"), Icon::ChartBar)
                .item(t!("reports.title"), reports)
        })
}
