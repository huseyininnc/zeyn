//! WASM islands: Rust components compiled to WebAssembly on server rendered screens.
//!
//! Build them with `zeyn build-islands` in `examples/showcase` (or `zeyn dev`). Without the build
//! the screens still work and show each island's fallback.

use showcase_shared::{Count, CounterProps, CounterTexts, Diagram, DiagramProps, DiagramTexts, SaveCount, SaveDiagram, Saved, Shape, ShapeKind, crdt};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, RwLock};
use zeyn::prelude::*;
use zeyn::realtime::{DocStore, Hub, Realtime, RoomKey};
use zeyn::render::escape;
use zeyn::ui::{Canvas, Island, Live};
use zeyn::{BoxFuture, Component, Render};

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

impl DiagramStore {
    fn set(&self, diagram: Diagram) -> Result<()> {
        *self.0.write().map_err(|_| Error::Internal("diagram lock".into()))? = diagram;
        Ok(())
    }
}

/// Saves the diagram after clamping it to the limits of `Diagram::clamped`, and tells the
/// screens that list its shapes to refresh.
#[action(permission = "showcase.islands.write", ai = false)]
pub async fn save_diagram(diagram: Diagram, store: DiagramStore, hub: Hub, ctx: Ctx) -> Result<Saved> {
    let diagram = diagram.clamped();
    let shapes = diagram.shapes.len() as u32;
    store.set(diagram)?;
    hub.notify(&ctx, "diagram");
    Ok(Saved { shapes })
}

/// The live diagram document: the one document `main`, persisted into the [`DiagramStore`]. Each
/// save also notifies the `diagram` topic, so the shape grid refreshes in place.
struct DiagramDocs {
    store: DiagramStore,
    hub: Hub,
    state: Arc<RwLock<Option<Vec<u8>>>>,
}

impl DocStore for DiagramDocs {
    fn load(&self, key: RoomKey) -> BoxFuture<Result<Option<Vec<u8>>>> {
        let state = (key.doc == "main").then(|| {
            self.state.read().ok().and_then(|s| s.clone()).unwrap_or_else(|| crdt::state(&crdt::from_diagram(&self.store.get())))
        });
        Box::pin(async move { state.map(Some).ok_or(Error::NotFound) })
    }

    fn save(&self, key: RoomKey, state: Vec<u8>) -> BoxFuture<Result<()>> {
        let doc = crdt::Doc::new();
        let saved = crdt::apply(&doc, &state).then(|| self.store.set(crdt::read(&doc)));
        if let Ok(mut s) = self.state.write() {
            *s = Some(state);
        }
        self.hub.notify_tenant(Some(key.tenant.as_str()).filter(|t| !t.is_empty()), "diagram");
        Box::pin(async move { saved.unwrap_or(Err(Error::Internal("unreadable diagram state".into()))) })
    }
}

/// The realtime channel of the showcase: the live diagram and its change topic.
pub fn realtime(store: DiagramStore) -> Realtime {
    let realtime = Realtime::new().topic("diagram", "showcase.islands.read");
    let docs = DiagramDocs { store, hub: realtime.hub(), state: Arc::default() };
    realtime.documents("diagram", "showcase.islands.read", "showcase.islands.write", docs)
}

/// A shape of the diagram as a grid row.
#[model]
pub struct DiagramRow {
    pub label: String,
    pub kind: String,
    pub x: i64,
    pub y: i64,
}

/// The shapes as the server last stored them.
#[action(permission = "showcase.islands.read")]
pub async fn diagram_rows(q: DataQuery, store: DiagramStore) -> Result<Page<DiagramRow>> {
    let rows = store
        .get()
        .shapes
        .into_iter()
        .map(|s| DiagramRow { label: s.label, kind: format!("{:?}", s.kind), x: i64::from(s.x), y: i64::from(s.y) })
        .collect();
    Ok(q.apply(rows))
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
        live_on: t("live_on"),
        live_off: t("live_off"),
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
    let props = DiagramProps { diagram, texts, live: Some("main".into()), height: 420 };
    Screen::new(t!("islands.diagram")).body(
        Stack::new()
            .add(
                Canvas::new("diagram", t!("islands.diagram_help"))
                    .props(&props)
                    .label(t!("islands.diagram"))
                    .fallback(Node::new(summary))
                    .action::<SaveDiagram, _>(save_diagram),
            )
            .add(
                Panel::new(t!("islands.stored")).add(
                    Live::new("diagram").add(
                        DataGrid::of(diagram_rows).paging(Paging::pages(10)).column(DiagramRow::label).flex(1).column(DiagramRow::kind).column(DiagramRow::x).column(DiagramRow::y),
                    ),
                ),
            ),
    )
}

pub fn feature() -> Feature {
    Feature::new("islands")
        .route_prefix("/islands")
        .screen(island_counter)
        .screen(island_diagram)
        .menu(|m| m.group(t!("menu.islands"), Icon::Chart).item(t!("islands.counter"), island_counter).item(t!("islands.diagram"), island_diagram))
}
