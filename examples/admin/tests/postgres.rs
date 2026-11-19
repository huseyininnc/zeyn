//! The admin app on PostgreSQL: migrations, the seeded rows, the dashboard's grouped queries and
//! saves with row version checks. Skipped unless `ZEYN_TEST_PG` names a database:
//!
//! ```text
//! docker run -d --name zeyn-data-pg -e POSTGRES_USER=zeyn -e POSTGRES_PASSWORD=zeyn \
//!     -e POSTGRES_DB=zeyn -p 55437:5432 postgres:17-alpine
//! ZEYN_TEST_PG=postgres://zeyn:zeyn@127.0.0.1:55437/zeyn \
//!     cargo test -p zeyn-example-admin --features postgres --test postgres
//! ```
#![cfg(feature = "postgres")]

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{Config, StaticSource};
use zeyn_example_admin::models::{Customer, Order, OrderStatus};
use zeyn_example_admin::roles::Role;
use zeyn_example_admin::store::{CUSTOMERS, ORDERS, Store};

async fn store() -> Option<(Config, Store)> {
    let url = std::env::var("ZEYN_TEST_PG").ok()?;
    let config = Config::load(vec![Box::new(StaticSource(
        [("database.url".to_string(), url)]
            .into_iter()
            .collect(),
    ))])
    .await
    .unwrap();
    let store = Store::connect(&config).await.unwrap();
    Some((config, store))
}

#[tokio::test]
async fn migrations_seed_a_few_thousand_rows() {
    let Some((_, store)) = store().await else {
        return;
    };
    let total = |q: DataQuery| DataQuery {
        with_total: true,
        ..q
    };
    let orders = store
        .find::<Order>(&total(DataQuery::default()))
        .await
        .unwrap();
    assert_eq!(orders.total, Some(ORDERS as u64));
    let customers = store
        .find::<Customer>(&total(DataQuery::default()))
        .await
        .unwrap();
    assert!(customers.total >= Some(CUSTOMERS as u64));
}

#[tokio::test]
async fn every_screen_opens_on_postgres() {
    let Some((config, store)) = store().await else {
        return;
    };
    let auth = |_: &axum::http::HeaderMap| Some(Role::Admin.identity("tester"));
    let router = zeyn_example_admin::app(config, store, auth)
        .unwrap()
        .router()
        .unwrap();
    for path in [
        "/",
        "/orders",
        "/orders/7",
        "/customers",
        "/customers/3",
        "/products",
        "/admin/settings",
        "/admin/users",
        "/admin/roles",
    ] {
        let res = router
            .clone()
            .oneshot(
                Request::get(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{path}");
    }
}

#[tokio::test]
async fn a_save_with_an_old_row_version_is_a_conflict() {
    let Some((_, store)) = store().await else {
        return;
    };
    let order: Order = store.get(11).await.unwrap();
    let next = match order.status {
        OrderStatus::Shipped => OrderStatus::Delivered,
        _ => OrderStatus::Shipped,
    };
    let saved = store
        .save(Order {
            status: next,
            ..order.clone()
        })
        .await
        .unwrap();
    assert_eq!(saved.version, order.version + 1);
    assert!(matches!(store.save(order).await, Err(Error::Conflict)));
}
