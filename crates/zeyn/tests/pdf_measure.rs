//! Cost of PDF documents: heap held while rendering and time, for a one page document and a
//! list report of 1,000 rows. Its own test binary, so no other test allocates while it measures.
//! Run with `--nocapture` to see the numbers.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use zeyn::{Pdf, PdfTemplate};

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

const NOTE: PdfTemplate = PdfTemplate::new("note", "#set text(font: \"Libertinus Serif\")\n= #data.title\n#data.body");

/// Renders once and returns (peak heap above the start, heap kept afterwards, milliseconds, size).
fn measure(pdf: &Pdf, template: PdfTemplate, data: &serde_json::Value) -> (usize, isize, u128, usize) {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let started = std::time::Instant::now();
    let bytes = pdf.render_now(template, serde_json::to_vec(data).unwrap()).unwrap();
    let ms = started.elapsed().as_millis();
    let size = bytes.len();
    drop(bytes);
    let peak = PEAK.load(Ordering::Relaxed) - base;
    let kept = LIVE.load(Ordering::Relaxed) as isize - base as isize;
    (peak, kept, ms, size)
}

#[test]
fn rendering_cost_is_bounded_and_does_not_leak() {
    let base = LIVE.load(Ordering::Relaxed);
    let pdf = Pdf::new();
    eprintln!("fonts loaded: {} KiB of heap", (LIVE.load(Ordering::Relaxed) - base) / 1024);
    let mut kept = Vec::new();
    for round in 0..4 {
        let note = serde_json::json!({ "title": format!("Note {round}"), "body": "Short text with Turkish letters: çğıöşü İ." });
        let rows: Vec<Vec<String>> = (0..1000)
            .map(|i| {
                vec![
                    format!("ORD-{round}{i:05}"),
                    format!("Customer {i}"),
                    "İzmir".into(),
                    format!("{},50 ₺", i * 7 + round),
                ]
            })
            .collect();
        let list = serde_json::json!({
            "title": "Orders", "lang": "tr", "note": null, "rows": rows,
            "columns": [{ "label": "No", "numeric": false }, { "label": "Customer", "numeric": false }, { "label": "City", "numeric": false }, { "label": "Total", "numeric": true }],
        });
        let (peak, k, ms, size) = measure(&pdf, NOTE, &note);
        eprintln!("note   round {round}: peak {} KiB, kept {} KiB, {ms} ms, {size} bytes", peak / 1024, k / 1024);
        let (peak, k, ms, size) = measure(&pdf, PdfTemplate::LIST, &list);
        eprintln!("list   round {round}: peak {} KiB, kept {} KiB, {ms} ms, {size} bytes", peak / 1024, k / 1024);
        kept.push(LIVE.load(Ordering::Relaxed));
    }
    let growth = kept[3] as isize - kept[1] as isize;
    eprintln!("heap growth from round 1 to 3: {} KiB", growth / 1024);
    assert!(growth < 8 * 1024 * 1024, "rendering keeps memory: {growth} bytes");
}
