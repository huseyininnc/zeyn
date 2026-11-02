//! Sales: today's orders in the user's time zone and an archive of one million orders.
//!
//! Orders are generated from their number, so the archive uses no memory.

use crate::stock::{ProductParams, ProductStore, Product, ProductId, product_editor};
use zeyn::prelude::*;
use zeyn::{Cursor, PageRequest};

/// A sales order.
#[entity(table = "orders")]
pub struct Order {
    #[key]
    pub id: OrderId,
    pub customer: String,
    pub city: String,
    pub placed_at: DateTime,
    pub amount: Money,
    pub status: OrderStatus,
}

/// A line of a sales order.
#[entity(table = "order_lines")]
pub struct OrderLine {
    #[key]
    pub id: OrderLineId,
    pub order_id: OrderId,
    pub product_id: ProductId,
    pub product: String,
    pub quantity: i64,
    pub price: Money,
    pub amount: Money,
}

/// Which order the lines belong to.
#[params]
pub struct OrderParams {
    pub id: OrderId,
}

/// The lines of order `n`: one to four products from the stock list, always the same for an order.
pub fn order_lines_of(n: u64, products: &[Product]) -> Vec<OrderLine> {
    let h = n.wrapping_mul(0xA24B_AED4_963E_E407) >> 11;
    if products.is_empty() {
        return Vec::new();
    }
    (0..1 + h % 4)
        .map(|k| {
            let p = &products[((h >> (k * 9)) % products.len() as u64) as usize];
            let quantity = 1 + ((h >> (k * 5)) % 9) as i64;
            OrderLine {
                id: OrderLineId((n * 10 + k) as i64),
                order_id: OrderId(n as i64),
                product_id: p.id,
                product: p.name.clone(),
                quantity,
                price: p.price,
                amount: Money::new(p.price.minor * quantity, p.price.currency),
            }
        })
        .collect()
}

/// Lines of the order named by the grid's scope filter on `order_id`, as a database would read
/// `WHERE order_id = ?`. Without that filter there are no lines.
#[action(permission = "sales.order.read")]
pub async fn search_lines(q: GridQuery, store: ProductStore) -> Result<Page<OrderLine>> {
    let order = q.filters.iter().find(|f| f.field == "order_id").and_then(|f| f.from.as_deref()?.parse::<u64>().ok());
    let Some(n) = order else { return Ok(Page::default()) };
    Ok(q.apply(order_lines_of(n, &store.all()?)))
}

/// Order status.
#[choice]
pub enum OrderStatus {
    Open,
    Shipped,
    Cancelled,
}

pub const ARCHIVE: u64 = 1_000_000;
const CUSTOMERS: [&str; 10] = [
    "Ada Yılmaz", "Can Demir", "Elif Şahin", "Mert Kaya", "Zeynep Arslan", "Ömer Çelik", "İpek Aydın", "Deniz Koç",
    "Selin Öztürk", "Kaan Yıldız",
];
const CITIES: [&str; 7] = ["İstanbul", "Ankara", "İzmir", "Bursa", "Antalya", "Konya", "Eskişehir"];

/// Order `n`, placed `minutes_ago` before `now`. Always the same for the same input.
pub fn order(n: u64, now: DateTime, minutes_ago: i64) -> Order {
    let h = n.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 17;
    Order {
        id: OrderId(n as i64),
        customer: CUSTOMERS[(h % 10) as usize].to_string(),
        city: CITIES[(h / 10 % 7) as usize].to_string(),
        placed_at: now.add_ms(-minutes_ago * 60_000),
        amount: Money::tl((h % 2_500_000) as i64 + 1_500),
        status: match h % 10 {
            0 => OrderStatus::Cancelled,
            1..=4 => OrderStatus::Open,
            _ => OrderStatus::Shipped,
        },
    }
}

/// Orders placed since midnight in the user's time zone, one every 9 minutes.
pub fn todays_orders(ctx: &Ctx) -> Vec<Order> {
    let now = ctx.now();
    let midnight = DateTime::from_local(LocalDateTime::new(ctx.today(), Time::default()), ctx.time_zone()).unwrap_or(now);
    let minutes = (now.unix_ms() - midnight.unix_ms()) / 60_000;
    (0..=minutes / 9).map(|i| order(ARCHIVE + 1 + i as u64, now, minutes - i * 9)).collect()
}

#[action(permission = "sales.order.read")]
pub async fn search_today(q: GridQuery, ctx: Ctx) -> Result<Page<Order>> {
    Ok(q.apply(todays_orders(&ctx)))
}

#[action(permission = "sales.order.read")]
pub async fn search_archive(q: GridQuery, ctx: Ctx) -> Result<Page<Order>> {
    let desc = q.sort.first().is_none_or(|s| !(s.field == "id" && !s.desc));
    let offset = match &q.page {
        PageRequest::Offset { offset, .. } => *offset,
        PageRequest::After { cursor, .. } => cursor.as_ref().and_then(|c| c.0.parse().ok()).unwrap_or(0),
    }
    .min(ARCHIVE);
    let end = (offset + u64::from(q.limit())).min(ARCHIVE);
    let now = ctx.now();
    let items = (offset..end)
        .map(|i| {
            let n = if desc { ARCHIVE - i } else { i + 1 };
            order(n, now, ((ARCHIVE - n) as i64 + 1) * 23)
        })
        .collect();
    let page = Page::new(items).total(ARCHIVE);
    Ok(if end < ARCHIVE { page.next(Cursor(end.to_string())) } else { page })
}

fn status_tone(s: &OrderStatus) -> Tone {
    match s {
        OrderStatus::Open => Tone::Warning,
        OrderStatus::Shipped => Tone::Success,
        OrderStatus::Cancelled => Tone::Danger,
    }
}

fn muted_if_cancelled(o: &Order) -> Option<RowStyle> {
    matches!(o.status, OrderStatus::Cancelled).then_some(RowStyle::Muted)
}

#[screen(route = "/today", permission = "sales.order.read")]
pub fn today() -> Screen {
    Screen::new(t!("sales.today")).body(
        Grid::of(search_today)
            .mode(GridMode::Server)
            .paging(Paging::pages(20))
            .column(Order::id)
            .width(110)
            .count()
            .column(Order::placed_at)
            .width(240)
            .filter()
            .min()
            .relative_time()
            .column(Order::customer)
            .flex(2)
            .filter()
            .column(Order::city)
            .flex(1)
            .filter()
            .column(Order::amount)
            .width(240)
            .filter()
            .sum()
            .avg()
            .column(Order::status)
            .width(130)
            .filter()
            .badge(status_tone)
            .row_style(muted_if_cancelled)
            .detail(|o| (order_lines, OrderParams { id: o.id })),
    )
}

/// Today's orders grouped by city and then by status, with counts and totals per group.
#[screen(route = "/by-city", permission = "sales.order.read")]
pub fn orders_by_city() -> Screen {
    Screen::new(t!("sales.by_city")).body(
        Grid::of(search_today)
            .mode(GridMode::Server)
            .paging(Paging::pages(20))
            .group_by(Order::city)
            .group_by(Order::status)
            .column(Order::id)
            .width(110)
            .column(Order::customer)
            .flex(2)
            .filter()
            .column(Order::placed_at)
            .width(200)
            .relative_time()
            .column(Order::amount)
            .width(260)
            .filter()
            .sum()
            .avg()
            .column(Order::status)
            .width(130)
            .badge(status_tone)
            .detail(|o| (order_lines, OrderParams { id: o.id })),
    )
}

/// The lines of one order, shown when a row of today's orders opens. Each line opens the product
/// editor in turn.
#[screen(permission = "sales.order.read")]
pub fn order_lines(params: OrderParams) -> Screen {
    Screen::window(t!("sales.order_lines", id = params.id.0.to_string())).body(
        Grid::of(search_lines)
            .scope(OrderLine::order_id, params.id)
            .paging(Paging::none())
            .column(OrderLine::product_id)
            .width(90)
            .column(OrderLine::product)
            .flex(1)
            .column(OrderLine::quantity)
            .width(110)
            .sum()
            .column(OrderLine::price)
            .width(150)
            .column(OrderLine::amount)
            .width(170)
            .sum()
            .detail(|l| (product_editor, ProductParams { id: Some(l.product_id) })),
    )
}

#[screen(route = "/archive", permission = "sales.order.read")]
pub fn archive() -> Screen {
    Screen::new(t!("sales.archive")).toolbar(|t| t.export(Export::Csv)).body(
        Grid::of(search_archive)
            .mode(GridMode::Server)
            .paging(Paging::virtual_scroll())
            .exportable()
            .column(Order::id)
            .width(110)
            .column(Order::placed_at)
            .width(190)
            .column(Order::customer)
            .flex(2)
            .column(Order::city)
            .flex(1)
            .column(Order::amount)
            .width(150)
            .column(Order::status)
            .width(130)
            .row_style(muted_if_cancelled),
    )
}

pub fn feature() -> Feature {
    Feature::new("sales")
        .route_prefix("/sales")
        .guard(SignedIn)
        .screen(today)
        .screen(order_lines)
        .screen(orders_by_city)
        .screen(archive)
        .menu(|m| {
            m.group(t!("menu.sales"), Icon::Cart)
                .item(t!("sales.today"), today)
                .item(t!("sales.by_city"), orders_by_city)
                .item(t!("sales.archive"), archive)
        })
}
