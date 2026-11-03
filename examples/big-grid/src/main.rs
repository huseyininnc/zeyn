//! One million orders in one scrollable grid.
//!
//! Rows are generated from their number, so the demo uses no memory for data. Only sorting by
//! order number is supported; a real app sorts and filters in the database.
//!
//! Run: `cargo run -p zeyn-example-big-grid`, then open http://127.0.0.1:3000

use zeyn::prelude::*;
use zeyn::{Cursor, I18n, PageRequest};

/// A sales order.
#[entity(table = "orders")]
pub struct Order {
    #[key]
    pub id: OrderId,
    pub customer: String,
    pub city: String,
    pub amount: Money,
    pub status: Status,
}

/// Order status.
#[choice]
pub enum Status {
    Open,
    Shipped,
    Cancelled,
}

const TOTAL: u64 = 1_000_000;
const CUSTOMERS: [&str; 8] = [
    "Ada Yılmaz",
    "Can Demir",
    "Elif Şahin",
    "Mert Kaya",
    "Zeynep Arslan",
    "Ömer Çelik",
    "İpek Aydın",
    "Deniz Koç",
];
const CITIES: [&str; 6] = ["İstanbul", "Ankara", "İzmir", "Bursa", "Antalya", "Konya"];

/// Order number `n`, always the same for the same number.
fn order(n: u64) -> Order {
    let h = n.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 17;
    Order {
        id: OrderId(n as i64),
        customer: CUSTOMERS[(h % 8) as usize].to_string(),
        city: CITIES[(h / 8 % 6) as usize].to_string(),
        amount: Money::tl((h % 5_000_000) as i64),
        status: match h % 10 {
            0 => Status::Cancelled,
            1..=4 => Status::Open,
            _ => Status::Shipped,
        },
    }
}

#[action(permission = "sales.order.read")]
pub async fn search_orders(q: DataQuery) -> Result<Page<Order>> {
    let desc = q
        .sort
        .first()
        .is_some_and(|s| s.field == "id" && s.desc);
    let offset = match &q.page {
        PageRequest::Offset { offset, .. } => *offset,
        PageRequest::After { cursor, .. } => cursor
            .as_ref()
            .and_then(|c| c.0.parse().ok())
            .unwrap_or(0),
    }
    .min(TOTAL);
    let end = (offset + u64::from(q.limit())).min(TOTAL);
    let items = (offset..end)
        .map(|i| order(if desc { TOTAL - i } else { i + 1 }))
        .collect();
    let page = Page::new(items).total(TOTAL);
    Ok(if end < TOTAL {
        page.next(Cursor(end.to_string()))
    } else {
        page
    })
}

#[screen(route = "/orders", permission = "sales.order.read")]
pub fn order_list() -> Screen {
    Screen::new(t!("orders.title")).body(
        DataGrid::of(search_orders)
            .mode(DataMode::Server)
            .paging(Paging::virtual_scroll())
            .column(Order::id)
            .width(110)
            .column(Order::customer)
            .flex(2)
            .column(Order::city)
            .flex(1)
            .column(Order::amount)
            .width(150)
            .column(Order::status)
            .width(130)
            .row_style(|o| matches!(o.status, Status::Cancelled).then_some(RowStyle::Muted)),
    )
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let i18n = I18n::new("en")
        .catalog("en", include_str!("../locales/en.json"))
        .and_then(|i| i.catalog("tr", include_str!("../locales/tr.json")))
        .expect("catalogs are valid JSON");
    let addr = std::env::var("ZEYN_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".into());
    App::new(t!("orders.title"))
        .i18n(i18n)
        .auth(DevAuth::superuser("demo"))
        .allow_dev_auth()
        .home(order_list)
        .feature(
            Feature::new("sales")
                .screen(order_list)
                .menu(|m| {
                    m.group(t!("orders.menu"), Icon::Cart)
                        .item(t!("orders.list"), order_list)
                }),
        )
        .serve(&addr)
        .await
}
