//! WASM islands: Rust components compiled to WebAssembly on server rendered screens.
//!
//! Build them with `zeyn build-islands` in `examples/showcase` (or `zeyn dev`). Without the build
//! the screens still work and show each island's fallback.

use showcase_shared::{Count, CounterProps, CounterTexts, Diagram, DiagramProps, DiagramTexts, SaveCount, SaveDiagram, Saved, Shape, ShapeKind};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, RwLock};
use zeyn::prelude::*;
use zeyn::render::escape;
use zeyn::ui::{Canvas, Island};
use zeyn::{Component, Render};

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

/// The shared diagram, kept in memory.
#[derive(Clone)]
pub struct DiagramStore(Arc<RwLock<Diagram>>);

impl Service for DiagramStore {}

impl Default for DiagramStore {
    fn default() -> Self {
        let shape = |id, kind, x, y, label: &str| Shape { id, kind, x, y, w: 140, h: 64, label: label.into() };
        DiagramStore(Arc::new(RwLock::new(Diagram {
            shapes: vec![
                shape(1, ShapeKind::Ellipse, 40, 40, "Order placed"),
                shape(2, ShapeKind::Box, 240, 40, "Check stock"),
                shape(3, ShapeKind::Box, 440, 40, "Ship"),
            ],
        })))
    }
}

impl DiagramStore {
    pub fn get(&self) -> Diagram {
        self.0.read().map(|d| d.clone()).unwrap_or_default()
    }
}

/// Saves the diagram after clamping it to the limits of `Diagram::clamped`.
#[action(permission = "showcase.islands.write", ai = false)]
pub async fn save_diagram(diagram: Diagram, store: DiagramStore) -> Result<Saved> {
    let diagram = diagram.clamped();
    let shapes = diagram.shapes.len() as u32;
    *store.0.write().map_err(|_| Error::Internal("diagram lock".into()))? = diagram;
    Ok(Saved { shapes })
}

fn diagram_texts(ctx: &Ctx) -> DiagramTexts {
    let t = |key: &str| ctx.t(&Text::key(format!("islands.{key}")));
    DiagramTexts {
        add_box: t("add_box"),
        add_ellipse: t("add_ellipse"),
        remove: t("remove"),
        next: t("next"),
        save: t("save"),
        saved: ctx.t(&t!("islands.diagram_saved", count = "{count}")),
        tools: t("tools"),
        canvas: t("diagram"),
        summary: t("summary"),
        empty: t("empty"),
        selected: ctx.t(&t!("islands.selected", shape = "{shape}")),
        kind_box: t("kind_box"),
        kind_ellipse: t("kind_ellipse"),
        line: ctx.t(&t!("islands.line", kind = "{kind}", label = "{label}", x = "{x}", y = "{y}", w = "{w}", h = "{h}")),
    }
}

/// The text version of the diagram, rendered on the server for users without the island.
struct Summary {
    title: Text,
    lines: Vec<String>,
}

impl Component for Summary {
    fn render(&self, out: &mut String, r: &Render) {
        out.push_str(&format!("<h3>{}</h3><ul>", r.text(&self.title)));
        for line in &self.lines {
            out.push_str(&format!("<li>{}</li>", escape(line)));
        }
        out.push_str("</ul>");
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({ "type": "diagram_summary", "lines": self.lines })
    }
}

#[screen(route = "/diagram", permission = "showcase.islands.read")]
pub fn island_diagram(_: (), store: DiagramStore, ctx: Ctx) -> Screen {
    let texts = diagram_texts(&ctx);
    let diagram = store.get();
    let summary = Summary { title: t!("islands.summary"), lines: diagram.summary(&texts) };
    let props = DiagramProps { diagram, texts, height: 420 };
    Screen::new(t!("islands.diagram")).body(
        Canvas::new("diagram", t!("islands.diagram_help"))
            .props(&props)
            .label(t!("islands.diagram"))
            .fallback(Node::new(summary))
            .action::<SaveDiagram, _>(save_diagram),
    )
}

pub fn feature() -> Feature {
    Feature::new("islands")
        .route_prefix("/islands")
        .screen(island_counter)
        .screen(island_diagram)
        .menu(|m| m.group(t!("menu.islands"), Icon::Chart).item(t!("islands.counter"), island_counter).item(t!("islands.diagram"), island_diagram))
}
