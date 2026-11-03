//! A whole service in tests: order, stock, kitchen, payment, and the screens through the router.

#[path = "../src/app.rs"]
mod app;
#[path = "../src/features/mod.rs"]
mod features;
#[path = "../src/restaurant.rs"]
mod restaurant;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use features::kitchen::models::LineStatus;
use features::menu::models::MenuItemId;
use features::orders::models::{DiningTableId, PaymentMethod, TableStatus};
use http_body_util::BodyExt;
use restaurant::Restaurant;
use tower::ServiceExt;
use zeyn::{Config, DateTime, Error, Money, StaticSource};

const TABLE: DiningTableId = DiningTableId(1);
const LENTIL_SOUP: MenuItemId = MenuItemId(1);
const RICE_PUDDING: MenuItemId = MenuItemId(8);
const TEA: MenuItemId = MenuItemId(10);

fn now() -> DateTime {
    DateTime::parse("2026-11-02T12:00:00Z").unwrap()
}

fn stock(r: &Restaurant, item: MenuItemId) -> i64 {
    r.menu()
        .unwrap()
        .into_iter()
        .find(|m| m.id == item)
        .unwrap()
        .stock
}

async fn get(path: &str, lang: &str) -> (StatusCode, String) {
    let config = Config::load(vec![Box::new(StaticSource(Default::default()))])
        .await
        .unwrap();
    let router = app::app(config)
        .unwrap()
        .router()
        .unwrap();
    let req = Request::get(path)
        .header("accept-language", lang)
        .body(Body::empty())
        .unwrap();
    let res = router.oneshot(req).await.unwrap();
    let status = res.status();
    (
        status,
        String::from_utf8(
            res.into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap(),
    )
}

#[tokio::test]
async fn the_app_has_no_structural_problems() {
    let config = Config::load(vec![]).await.unwrap();
    assert_eq!(app::app(config).unwrap().check(), Ok(()));
}

#[test]
fn a_table_orders_sends_to_the_kitchen_and_pays() {
    let r = Restaurant::seeded();
    let before = stock(&r, LENTIL_SOUP);
    r.add(TABLE, LENTIL_SOUP, now())
        .unwrap();
    r.add(TABLE, LENTIL_SOUP, now())
        .unwrap();
    r.add(TABLE, TEA, now()).unwrap();
    assert_eq!(r.table(TABLE).unwrap().status, TableStatus::Ordering);
    assert_eq!(
        r.table(TABLE).unwrap().open_total,
        Money::tl(2 * 12_000 + 3_000)
    );
    assert_eq!(
        stock(&r, LENTIL_SOUP),
        before,
        "ordering alone does not touch stock"
    );

    assert!(
        matches!(
            r.pay(TABLE, PaymentMethod::Card, Money::tl(0), now(), "en"),
            Err(Error::Validation(_))
        ),
        "unsent lines block payment"
    );
    assert_eq!(r.send(TABLE, now()).unwrap(), 3);
    assert_eq!(
        stock(&r, LENTIL_SOUP),
        before - 2,
        "sending takes portions out of stock"
    );
    assert_eq!(r.table(TABLE).unwrap().status, TableStatus::Kitchen);

    let soup = r
        .lines()
        .unwrap()
        .into_iter()
        .find(|l| l.item_id == LENTIL_SOUP)
        .unwrap();
    assert!(
        matches!(r.remove(soup.id), Err(Error::Validation(_))),
        "lines in the kitchen cannot be removed"
    );
    r.ready(soup.id).unwrap();
    assert_eq!(
        r.lines()
            .unwrap()
            .into_iter()
            .find(|l| l.id == soup.id)
            .unwrap()
            .status,
        LineStatus::Ready
    );

    assert!(
        matches!(
            r.pay(TABLE, PaymentMethod::Cash, Money::tl(100), now(), "en"),
            Err(Error::Validation(_))
        ),
        "cash must cover the total"
    );
    assert_eq!(
        r.pay(TABLE, PaymentMethod::Cash, Money::tl(30_000), now(), "en")
            .unwrap(),
        Money::tl(3_000)
    );
    assert_eq!(r.table(TABLE).unwrap().status, TableStatus::Free);
    assert_eq!(r.sales().unwrap().len(), 1);
    let soups = r.revenue_by_category().unwrap()[0].1;
    assert_eq!(soups, 240.0);
}

#[test]
fn stock_limits_what_can_be_ordered_and_sent() {
    let r = Restaurant::seeded();
    r.add(TABLE, RICE_PUDDING, now())
        .unwrap();
    r.add(TABLE, RICE_PUDDING, now())
        .unwrap();
    assert!(
        matches!(r.add(TABLE, RICE_PUDDING, now()), Err(Error::Validation(_))),
        "only two portions are left"
    );

    let other = DiningTableId(2);
    assert!(
        matches!(r.add(other, RICE_PUDDING, now()), Err(Error::Validation(_))),
        "portions waiting on another table count too"
    );
    r.send(TABLE, now()).unwrap();
    assert_eq!(stock(&r, RICE_PUDDING), 0);
    assert!(
        r.add(other, RICE_PUDDING, now())
            .is_err()
    );
}

#[tokio::test]
async fn the_screens_open_in_both_languages() {
    for (lang, floor, kitchen) in [
        ("en", "Floor", "Kitchen display"),
        ("tr", "Salon", "Mutfak ekranı"),
    ] {
        let (status, html) = get("/", lang).await;
        assert_eq!(status, StatusCode::OK, "{html}");
        assert!(html.contains(floor), "{lang}");
        let (status, html) = get("/kitchen", lang).await;
        assert_eq!(status, StatusCode::OK);
        assert!(html.contains(kitchen));
    }
    let (status, html) = get("/order?params=%7B%22table%22%3A1%7D", "tr").await;
    assert_eq!(status, StatusCode::OK, "{html}");
    assert!(html.contains("Mercimek") || html.contains("Lentil soup"));
    assert!(html.contains("Mutfağa gönder"));
    let (status, _) = get("/reports", "en").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = get("/menu", "en").await;
    assert_eq!(status, StatusCode::OK);
}
