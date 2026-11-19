//! Dashboard: key figures of the last 30 days against the 30 before, revenue by month, orders by
//! status and the latest orders. It refreshes in place when an order is saved anywhere.

use super::orders::{OrderParams, count_open, order_page, search_orders};
use crate::ORDERS_TOPIC;
use crate::models::{Customer, Order, OrderStatus};
use crate::store::Store;
use zeyn::prelude::*;
use zeyn::ui::Live;
use zeyn::{Aggregate, AggregateRequest, Filter, PageRequest, Value};

/// Months shown by the revenue chart.
const MONTHS: i64 = 6;

/// Revenue and order count of one day.
struct Day {
    date: Date,
    orders: u64,
    revenue: i64,
}

#[screen(route = "/", permission = "admin.dashboard.read")]
pub async fn dashboard(_: (), store: Store, ctx: Ctx) -> Result<Screen> {
    let today = ctx.today();
    let days = revenue_by_day(&store, today).await?;
    let window = |from: i64, to: i64| {
        days.iter()
            .filter(|d| {
                let age = today.days_since_epoch() - d.date.days_since_epoch();
                (from..=to).contains(&age)
            })
            .fold((0u64, 0i64), |(n, r), d| (n + d.orders, r + d.revenue))
    };
    let (orders, revenue) = window(0, 29);
    let (orders_before, revenue_before) = window(30, 59);
    let weeks: Vec<f64> = (0..8)
        .rev()
        .map(|w| major(window(w * 7, w * 7 + 6).1))
        .collect();
    let customers = new_customers(&store, today, 0).await?;
    let customers_before = new_customers(&store, today, 30).await?;
    let open = count_open(&store).await?;
    let average = |n: u64, r: i64| if n == 0 { 0 } else { r / n as i64 };
    let kpis = Row::new()
        .col(
            Kpi::money(t!("dashboard.revenue"), Money::tl(revenue))
                .delta(change(major(revenue), major(revenue_before)))
                .trend(weeks),
        )
        .col(
            Kpi::new(t!("dashboard.orders"), orders as f64)
                .delta(change(orders as f64, orders_before as f64)),
        )
        .col(
            Kpi::money(t!("dashboard.average"), Money::tl(average(orders, revenue))).delta(change(
                major(average(orders, revenue)),
                major(average(orders_before, revenue_before)),
            )),
        )
        .col(
            Kpi::new(t!("dashboard.new_customers"), customers as f64)
                .delta(change(customers as f64, customers_before as f64)),
        );
    let charts = Row::new()
        .col(Card::new().add(monthly_chart(&days, today)))
        .weight(2)
        .col(Card::new().add(status_chart(&store, today).await?));
    Ok(Screen::new(t!("dashboard.title"))
        .subtitle(t!("dashboard.subtitle", open = open))
        .body(Live::new(ORDERS_TOPIC).add(Stack::new().add(kpis).add(charts)))
        .body(Panel::new(t!("dashboard.latest")).add(latest_orders().listens(ORDERS_TOPIC))))
}

/// Revenue and order count per day of the last months, cancelled orders left out: one grouped
/// query, in SQL or in memory.
async fn revenue_by_day(store: &Store, today: Date) -> Result<Vec<Day>> {
    let from = today
        .add_months(1 - MONTHS)
        .and_then(|d| Date::new(d.year(), d.month(), 1))
        .unwrap_or(today);
    let q = DataQuery {
        filters: vec![
            Filter::range("placed", Some(&from.to_string()), None),
            Filter {
                field: "status".into(),
                any_of: [
                    OrderStatus::New,
                    OrderStatus::Processing,
                    OrderStatus::Shipped,
                    OrderStatus::Delivered,
                ]
                .iter()
                .map(|s| format!("{s:?}"))
                .collect(),
                ..Filter::default()
            },
        ],
        aggregates: vec![AggregateRequest {
            field: "total".into(),
            op: Aggregate::Sum,
        }],
        group: Some("placed".into()),
        page: PageRequest::Offset {
            offset: 0,
            limit: 500,
        },
        ..DataQuery::default()
    };
    Ok(store
        .find::<Order>(&q)
        .await?
        .groups
        .into_iter()
        .filter_map(|g| {
            let Value::Date(date) = g.value else {
                return None;
            };
            let revenue = match g.totals.first().map(|t| &t.value) {
                Some(Value::Money(m)) => m.minor,
                Some(Value::Int(n)) => *n,
                _ => 0,
            };
            Some(Day {
                date,
                orders: g.count,
                revenue,
            })
        })
        .collect())
}

/// Customers who joined in the 30 days that end `ago` days before today.
async fn new_customers(store: &Store, today: Date, ago: i64) -> Result<u64> {
    let day = |n: i64| {
        today
            .add_days(-n)
            .unwrap_or(today)
            .to_string()
    };
    let q = DataQuery {
        filters: vec![Filter::range(
            "since",
            Some(&day(ago + 29)),
            Some(&day(ago)),
        )],
        page: PageRequest::Offset {
            offset: 0,
            limit: 1,
        },
        with_total: true,
        ..DataQuery::default()
    };
    Ok(store
        .find::<Customer>(&q)
        .await?
        .total
        .unwrap_or(0))
}

/// Revenue of each of the last months as bars.
fn monthly_chart(days: &[Day], today: Date) -> Chart {
    let months: Vec<Date> = (0..MONTHS)
        .rev()
        .filter_map(|m| today.add_months(-m))
        .collect();
    let labels: Vec<Text> = months
        .iter()
        .map(|d| Text::key(format!("months.m{}", d.month())))
        .collect();
    let values: Vec<f64> = months
        .iter()
        .map(|m| {
            major(
                days.iter()
                    .filter(|d| d.date.year() == m.year() && d.date.month() == m.month())
                    .map(|d| d.revenue)
                    .sum(),
            )
        })
        .collect();
    Chart::bar(t!("dashboard.revenue_by_month"))
        .labels(labels)
        .series(t!("dashboard.revenue"), values)
}

/// Orders of the last 30 days by status.
async fn status_chart(store: &Store, today: Date) -> Result<Chart> {
    let from = today
        .add_days(-29)
        .unwrap_or(today)
        .to_string();
    let q = DataQuery {
        filters: vec![Filter::range("placed", Some(&from), None)],
        group: Some("status".into()),
        page: PageRequest::Offset {
            offset: 0,
            limit: 10,
        },
        ..DataQuery::default()
    };
    let groups = store.find::<Order>(&q).await?.groups;
    let labels: Vec<Text> = groups
        .iter()
        .map(|g| match &g.value {
            Value::Choice(c) => Text::key(format!("OrderStatus.{c}")),
            _ => Text::from(""),
        })
        .collect();
    let counts: Vec<f64> = groups
        .iter()
        .map(|g| g.count as f64)
        .collect();
    Ok(Chart::donut(t!("dashboard.by_status"))
        .labels(labels)
        .series(t!("dashboard.orders"), counts))
}

/// The eight newest orders, each a link to its page.
fn latest_orders() -> DataGrid<Order> {
    DataGrid::of(search_orders)
        .paging(Paging::pages(8))
        .sort_by_desc(Order::placed)
        .column(Order::number)
        .width(120)
        .link(|o| Effect::navigate(order_page, OrderParams { id: o.id.0 }))
        .column(Order::customer_name)
        .flex(1)
        .column(Order::placed)
        .width(140)
        .column(Order::status)
        .width(140)
        .badge(|s| s.tone())
        .column(Order::total)
        .width(150)
        .on_open(|o| Effect::navigate(order_page, OrderParams { id: o.id.0 }))
}

/// An amount in the smallest unit as a number in the main unit.
fn major(minor: i64) -> f64 {
    minor as f64 / 100.0
}

/// The change from `before` to `now` in percent, 0 without a value before.
fn change(now: f64, before: f64) -> f64 {
    if before == 0.0 {
        0.0
    } else {
        ((now - before) / before * 1000.0).round() / 10.0
    }
}

/// The dashboard feature and its menu.
pub fn feature() -> Feature {
    Feature::new("dashboard")
        .guard(SignedIn)
        .screen(dashboard)
        .menu(|m| {
            m.group(t!("menu.overview"), Icon::Dashboard)
                .item(t!("dashboard.title"), dashboard)
                .icon(Icon::Home)
        })
}
