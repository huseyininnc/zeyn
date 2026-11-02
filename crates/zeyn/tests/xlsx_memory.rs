//! Memory of an XLSX export stays flat as rows grow: the worksheet is written in constant memory
//! mode. Its own test binary, so no other test allocates while it measures.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use zeyn::{Currency, Date, ExportColumn, LocalDateTime, Money, Time, Value, XlsxSheet};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            let now = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Discards what is written, counting the bytes.
struct Sink(usize);

impl std::io::Write for Sink {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0 += b.len();
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn page(start: usize) -> Vec<Vec<Value>> {
    (start..start + 500)
        .map(|i| {
            vec![
                Value::Text(format!("Row number {i} with some text")),
                Value::Int(i as i64),
                Value::Money(Money::new(i as i64 * 137, Currency::Try)),
                Value::Bool(i % 2 == 0),
                Value::Date(Date::new(2026, 1 + (i % 12) as u8, 1 + (i % 28) as u8).unwrap()),
                Value::LocalDateTime(LocalDateTime::new(Date::new(2026, 5, 4).unwrap(), Time::new((i % 24) as u8, 30, 0).unwrap())),
            ]
        })
        .collect()
}

#[test]
fn exporting_100k_rows_keeps_memory_flat() {
    let columns: Vec<ExportColumn> = ["Text", "Int", "Money", "Bool", "Date", "Moment"].iter().map(|l| ExportColumn { label: l.to_string(), width: None, numeric: false }).collect();
    let started = std::time::Instant::now();
    let base = LIVE.load(Ordering::Relaxed);
    let mut sheet = XlsxSheet::new(&columns, "tr").unwrap();
    let mut peaks = Vec::new();
    for p in 0..200 {
        PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
        let rows = page(p * 500);
        sheet.write_rows(&rows).unwrap();
        drop(rows);
        peaks.push(PEAK.load(Ordering::Relaxed) - base);
    }
    assert_eq!(sheet.rows(), 100_000);
    PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
    let mut out = Sink(0);
    sheet.finish(&mut out).unwrap();
    let finish_peak = PEAK.load(Ordering::Relaxed) - base;
    let early = *peaks[..20].iter().max().unwrap();
    let late = *peaks[180..].iter().max().unwrap();
    eprintln!(
        "100k rows: {} bytes zipped in {:?}; peak heap {} KiB after 10k rows, {} KiB after 100k rows, {} KiB while zipping",
        out.0,
        started.elapsed(),
        early / 1024,
        late / 1024,
        finish_peak / 1024
    );
    assert!(late < early + 256 * 1024, "memory grew with rows: {early} -> {late}");
    assert!(finish_peak < 8 * 1024 * 1024, "zipping held {finish_peak} bytes");
    assert!(out.0 > 1_000_000);
}
