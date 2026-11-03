//! Memory of reading an import file stays within its limits: an oversized or oddly shaped file is
//! refused or cut while it is read, before it is held in memory. Its own test binary, so no other
//! test allocates while it measures.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use zeyn::prelude::*;

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

#[model]
pub struct Contact {
    #[rule(required, max_len = 60)]
    pub name: String,
}

const MIB: usize = 1024 * 1024;

/// Peak bytes allocated while `work` runs, above what was live before it.
fn peak(work: impl FnOnce()) -> usize {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    work();
    PEAK.load(Ordering::Relaxed) - base
}

#[test]
fn import_files_are_cut_to_their_limits_while_they_are_read() {
    let ctx = Ctx::for_tests(Identity::user("u"), zeyn::Services::default());

    let tall = format!("name\n{}", "a\n".repeat(4 * MIB - 8));
    let used = peak(|| assert!(zeyn::check_csv::<Contact>(&ctx, &tall, 5_000).is_err()));
    assert!(
        used < 4 * MIB,
        "a CSV with too many rows used {} MiB",
        used / MIB
    );

    let wide = format!("name\n{}\n", "a,".repeat(4 * MIB - 8));
    let used = peak(|| {
        let checked = zeyn::check_csv::<Contact>(&ctx, &wide, 5_000).unwrap();
        assert_eq!(checked.invalid(), 1, "extra cells are still reported");
    });
    assert!(used < 4 * MIB, "a wide CSV row used {} MiB", used / MIB);

    #[cfg(feature = "xlsx")]
    {
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        sheet.write(0, 0, "name").unwrap();
        for r in 1..=5_000u32 {
            sheet.write(r, 0, "Ada").unwrap();
            sheet.write(r, 511, "x").unwrap();
        }
        let bytes = book.save_to_buffer().unwrap();
        let file = zeyn::ImportFile::Xlsx(bytes);
        let used = peak(|| {
            let checked = zeyn::check_file::<Contact>(&ctx, &file, 50_000).unwrap();
            assert_eq!(checked.invalid(), 5_000);
        });
        assert!(
            used < 24 * MIB,
            "a workbook with far right cells used {} MiB",
            used / MIB
        );
    }
}
