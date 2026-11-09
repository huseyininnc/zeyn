//! Cost of `DataQuery::apply` on a list of one million rows: time and allocations to sort by a
//! text column and by a number, for the first page and a deep page. Its own test binary, so no
//! other test allocates while it measures. Ignored by default; run in release with
//! `cargo test --release -p zeyn --test grid_measure -- --ignored --nocapture`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use zeyn::prelude::*;
use zeyn::{PageRequest, Sort};

struct Counting;

static COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.fetch_add(1, Ordering::Relaxed);
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
    pub amount: Money,
}

const NAMES: [&str; 8] = [
    "Ada Yılmaz",
    "Can Demir",
    "Elif Şahin",
    "Mert Kaya",
    "Zeynep Arslan",
    "Ömer Çelik",
    "İpek Aydın",
    "Deniz Koç",
];

fn rows(n: u64) -> Vec<Order> {
    (1..=n)
        .map(|i| {
            let h = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 17;
            Order {
                id: OrderId(i as i64),
                customer: format!("{} {}", NAMES[(h % 8) as usize], h % 1000),
                amount: Money::tl((h % 5_000_000) as i64),
            }
        })
        .collect()
}

fn query(field: &str, offset: u64) -> DataQuery {
    DataQuery {
        page: PageRequest::Offset { offset, limit: 200 },
        sort: vec![Sort {
            field: field.into(),
            desc: false,
        }],
        ..DataQuery::default()
    }
}

#[test]
#[ignore = "measurement, run in release with --ignored --nocapture"]
fn sorting_a_million_rows() {
    let all = rows(1_000_000);
    for (field, offset) in [
        ("customer", 0),
        ("customer", 500_000),
        ("amount", 0),
        ("amount", 500_000),
    ] {
        let input = all.clone();
        let q = query(field, offset);
        let before = COUNT.load(Ordering::Relaxed);
        let started = std::time::Instant::now();
        let page = q.apply(input);
        let ms = started.elapsed().as_millis();
        let allocs = COUNT.load(Ordering::Relaxed) - before;
        assert_eq!(page.items.len(), 200);
        println!("sort by {field}, offset {offset}: {ms} ms, {allocs} allocations");
    }
}
