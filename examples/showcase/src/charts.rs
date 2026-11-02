//! Charts and key figures: every chart kind, KPI cards, and a grid of sales reps with sparkline
//! and bullet cells over 10,000 rows.

use std::sync::LazyLock;
use zeyn::prelude::*;
use zeyn::ui::{BulletCell, SparklineCell};

/// A sales rep with twelve weeks of sales.
#[entity(table = "reps")]
pub struct Rep {
    #[key]
    pub id: RepId,
    #[rule(required, max_len = 60)]
    pub name: String,
    pub sales: i64,
    pub weeks: Vec<f64>,
}

static REPS: LazyLock<Vec<Rep>> = LazyLock::new(|| {
    let names = ["Ada", "Can", "Deniz", "Ece", "Mert", "Nil", "Ozan", "Selin", "Tuna", "Zeynep"];
    (1..=10_000)
        .map(|i| Rep {
            id: RepId(i),
            name: format!("{} {i}", names[(i % 10) as usize]),
            sales: (i * 7_919) % 120_000,
            weeks: (0..12).map(|k| ((i * 31 + k * 17) % 90 + 10) as f64).collect(),
        })
        .collect()
});

/// Sales reps for the grid.
#[action(permission = "stock.product.read")]
pub async fn search_reps(q: DataQuery) -> Result<Page<Rep>> {
    Ok(q.apply(REPS.clone()))
}

#[screen(route = "/charts", permission = "stock.product.read")]
pub fn charts() -> Screen {
    let months = [t!("ui.m1"), t!("ui.m2"), t!("ui.m3"), t!("ui.m4"), t!("ui.m5"), t!("ui.m6")];
    let card = |c: Chart| Card::new().add(c);
    Screen::new(t!("charts.title")).body(
        Stack::new()
            .add(
                Row::new()
                    .col(
                        Kpi::new(t!("charts.revenue"), 184_250.0)
                            .delta(12.4)
                            .target(200_000.0)
                            .trend([140.0, 152.0, 149.0, 160.0, 171.0, 184.0]),
                    )
                    .col(
                        Kpi::new(t!("charts.wait"), 6.5)
                            .decimals(1)
                            .unit(t!("charts.minutes"))
                            .delta(-9.0)
                            .lower_is_better()
                            .trend([9.0, 8.2, 7.4, 7.1, 6.5]),
                    )
                    .col(
                        Kpi::new(t!("charts.returns"), 41.0)
                            .delta(3.0)
                            .lower_is_better()
                            .trend([30.0, 35.0, 33.0, 41.0]),
                    ),
            )
            .add(
                Row::new()
                    .col(card(
                        Chart::stacked_bar(t!("charts.channels"))
                            .labels(months.clone())
                            .series(t!("charts.shop"), [80.0, 72.0, 95.0, 101.0, 98.0, 120.0])
                            .series(t!("charts.web"), [40.0, 26.0, 48.0, 59.0, 53.0, 69.0]),
                    ))
                    .col(card(
                        Chart::combo(t!("charts.margin"))
                            .labels(months.clone())
                            .series(t!("charts.sales"), [120.0, 98.0, 143.0, 160.0, 151.0, 189.0])
                            .line_series(t!("charts.margin_pct"), [18.0, 14.0, 21.0, 22.0, 19.0, 24.0])
                            .right_axis(),
                    )),
            )
            .add(
                Row::new()
                    .col(card(
                        Chart::area(t!("charts.visits"))
                            .labels(months)
                            .series(t!("charts.visits"), [420.0, 510.0, 470.0, 630.0, 700.0, 660.0]),
                    ))
                    .col(card(
                        Chart::scatter(t!("charts.price_units"))
                            .decimals(1)
                            .points(t!("charts.tea"), [(1.5, 40.0), (2.0, 34.0), (2.5, 22.0), (3.0, 18.0)])
                            .points(t!("charts.coffee"), [(3.5, 30.0), (4.0, 26.0), (5.5, 12.0)]),
                    )),
            )
            .add(
                Row::new()
                    .col(card(Chart::gauge(t!("charts.capacity")).value(72.0).range(0.0, 100.0).target(85.0)))
                    .col(card(
                        Chart::funnel(t!("charts.funnel"))
                            .labels([
                                t!("charts.f_visits"),
                                t!("charts.f_carts"),
                                t!("charts.f_orders"),
                                t!("charts.f_paid"),
                            ])
                            .series(t!("charts.people"), [1000.0, 420.0, 180.0, 150.0]),
                    )),
            )
            .add(
                Card::new().title(t!("charts.reps")).add(
                    DataGrid::of(search_reps)
                        .paging(Paging::virtual_scroll())
                        .column(Rep::name)
                        .flex(1)
                        .filter()
                        .column(Rep::sales)
                        .width(220)
                        .cell(BulletCell::new(80_000.0, 120_000.0))
                        .column(Rep::weeks)
                        .width(120)
                        .cell(SparklineCell::new()),
                ),
            ),
    )
}

pub fn feature() -> Feature {
    Feature::new("charts")
        .screen(charts)
        .menu(|m| m.group(t!("menu.charts"), Icon::Chart).item(t!("charts.title"), charts))
}
