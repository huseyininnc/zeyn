//! WASM islands: Rust components compiled to WebAssembly on server rendered screens.
//!
//! Build them with `zeyn build-islands` in `examples/showcase` (or `zeyn dev`). Without the build
//! the screens still work and show each island's fallback.

use showcase_shared::{Count, CounterProps, CounterTexts, SaveCount};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use zeyn::prelude::*;
use zeyn::ui::Island;

/// The counter value, kept in memory.
#[derive(Clone, Default)]
pub struct CounterStore(Arc<AtomicI64>);

impl Service for CounterStore {}

/// Saves the counter value, kept within plus or minus one million.
#[action(permission = "showcase.islands.write", ai = false)]
pub async fn save_count(count: Count, store: CounterStore) -> Result<Count> {
    let value = count.value.clamp(-1_000_000, 1_000_000);
    store.0.store(value, Ordering::Relaxed);
    Ok(Count { value })
}

#[screen(route = "/counter", permission = "showcase.islands.read")]
pub fn island_counter(_: (), store: CounterStore, ctx: Ctx) -> Screen {
    let props = CounterProps {
        start: store.0.load(Ordering::Relaxed),
        texts: CounterTexts {
            less: ctx.t(&t!("islands.less")),
            more: ctx.t(&t!("islands.more")),
            save: ctx.t(&t!("islands.save")),
            saved: ctx.t(&t!("islands.saved", value = "{value}")),
            value: ctx.t(&t!("islands.counter")),
        },
    };
    Screen::new(t!("islands.counter")).body(
        Stack::new().add(Alert::info(t!("islands.counter_intro"))).add(
            Island::new("counter")
                .props(&props)
                .label(t!("islands.counter"))
                .fallback(Alert::info(t!("islands.fallback")))
                .action::<SaveCount, _>(save_count),
        ),
    )
}

pub fn feature() -> Feature {
    Feature::new("islands")
        .route_prefix("/islands")
        .screen(island_counter)
        .menu(|m| m.group(t!("menu.islands"), Icon::Chart).item(t!("islands.counter"), island_counter))
}
