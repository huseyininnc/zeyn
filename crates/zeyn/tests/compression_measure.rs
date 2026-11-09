//! Cost of compressing dynamic responses: time and bytes allocated per request for a full page and
//! a partial screen with a 200 row grid, with and without gzip. Its own test binary, so no other
//! test allocates while it measures. Ignored by default; run in release with
//! `cargo test --release -p zeyn --test compression_measure -- --ignored --nocapture`.

use axum::body::Body;
use axum::http::{HeaderMap, Request, header};
use http_body_util::BodyExt;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;
use zeyn::prelude::*;
use zeyn::{I18n, Identity};

struct Counting;

static BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }
}

#[global_allocator]
static A: Counting = Counting;

#[entity(table = "orders")]
pub struct Order {
    #[key]
    pub id: OrderId,
    pub customer: String,
    pub city: String,
    pub amount: Money,
}

#[action(permission = "orders.read")]
pub async fn search_orders(q: DataQuery) -> Result<Page<Order>> {
    let rows = (1..=1000)
        .map(|i: i64| Order {
            id: OrderId(i),
            customer: format!("Customer {i}"),
            city: ["İstanbul", "Ankara", "İzmir"][(i % 3) as usize].to_string(),
            amount: Money::tl(i * 137),
        })
        .collect();
    Ok(q.apply(rows))
}

#[screen(route = "/orders", permission = "orders.read")]
pub fn orders() -> Screen {
    Screen::new("Orders").body(
        DataGrid::of(search_orders)
            .paging(Paging::pages(200))
            .column(Order::customer)
            .column(Order::city)
            .column(Order::amount),
    )
}

fn app() -> axum::Router {
    App::new("Measure")
        .i18n(I18n::new("en"))
        .auth(|_: &HeaderMap| Some(Identity::user("u").with_permissions(["orders.read"])))
        .feature(Feature::new("orders").screen(orders))
        .router()
        .unwrap()
}

/// Runs `n` requests and returns (microseconds per request, bytes allocated per request, size).
async fn measure(
    router: &axum::Router,
    gzip: bool,
    partial: bool,
    n: usize,
) -> (u128, usize, usize) {
    let request = || {
        let mut r = Request::get("/orders");
        if gzip {
            r = r.header(header::ACCEPT_ENCODING, "gzip");
        }
        if partial {
            r = r.header("x-zeyn-partial", "1");
        }
        r.body(Body::empty()).unwrap()
    };
    let mut size = 0;
    for _ in 0..50 {
        let res = router
            .clone()
            .oneshot(request())
            .await
            .unwrap();
        size = res
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .len();
    }
    let before = BYTES.load(Ordering::Relaxed);
    let started = std::time::Instant::now();
    for _ in 0..n {
        let res = router
            .clone()
            .oneshot(request())
            .await
            .unwrap();
        let _ = res.into_body().collect().await.unwrap();
    }
    let us = started.elapsed().as_micros() / n as u128;
    (us, (BYTES.load(Ordering::Relaxed) - before) / n, size)
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "measurement, run in release with --ignored --nocapture"]
async fn compressing_pages() {
    let router = app();
    for (partial, gzip) in [(false, false), (false, true), (true, false), (true, true)] {
        let (us, bytes, size) = measure(&router, gzip, partial, 2000).await;
        println!(
            "{}{}: {size} bytes out, {us} us/request, {bytes} bytes allocated/request",
            if partial {
                "partial screen"
            } else {
                "full page"
            },
            if gzip { ", gzip" } else { "" },
        );
    }
}
