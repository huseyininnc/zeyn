//! `Surface`: an area where people place items freely, such as tables on a floor plan or widgets
//! on a board, with [`Draggable`] items that move along one or both axes, inside the surface's
//! bounds and on its snap grid.

use super::{DataSource, dom_id, render_child};
use crate::action::{Action, ActionMeta, Error, Result};
use crate::ctx::{BoxFuture, Ctx, Outcome};
use crate::i18n::Text;
use crate::render::{Render, escape};
use crate::screen::{Component, Node};
use serde_json::{Value as Json, json};

/// Longest item id the browser may send, in bytes.
const MAX_ID: usize = 200;

/// Largest width or height of a surface, in CSS pixels.
pub const MAX_SURFACE: u32 = 20_000;

/// Which way an item moves: both ways, only sideways or only up and down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Axis {
    #[default]
    Both,
    X,
    Y,
}

impl Axis {
    fn attr(self) -> &'static str {
        match self {
            Axis::Both => "both",
            Axis::X => "x",
            Axis::Y => "y",
        }
    }
}

/// A move of one item, as the move action of a [`Surface`] gets it. The server has already
/// checked that `id` is an item of the surface, kept the position inside the bounds, on the snap
/// grid and on the item's axis; the action only stores it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SurfaceMove {
    /// The id the screen gave the item with [`Draggable::new`].
    pub id: String,
    /// Left edge, in CSS pixels from the left of the surface.
    pub x: u32,
    /// Top edge, in CSS pixels from the top of the surface.
    pub y: u32,
}

/// An item of a [`Surface`] at a position the app keeps, usually read from the database.
///
/// ```ignore
/// Draggable::new("t1", Paragraph::new("Table 1"))
///     .label("Table 1")
///     .at(40, 60)
///     .size(120, 80)
/// ```
pub struct Draggable {
    spot: Spot,
    node: Node,
}

/// Where an item of a surface is and how it moves.
struct Spot {
    id: String,
    label: Option<Text>,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    axis: Axis,
}

impl Draggable {
    /// An item with the id the move action gets back, holding `node`. It starts at the top left,
    /// 160 by 80 pixels.
    #[track_caller]
    pub fn new(id: impl Into<String>, node: impl Into<Node>) -> Self {
        Draggable {
            spot: Spot {
                id: id.into(),
                label: None,
                x: 0,
                y: 0,
                w: 160,
                h: 80,
                axis: Axis::Both,
            },
            node: Node::placed(node),
        }
    }

    /// The name screen readers and announcements use; the id when not set.
    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.spot.label = Some(label.into());
        self
    }

    /// Position of the top left corner, in CSS pixels from the top left of the surface.
    pub fn at(mut self, x: u32, y: u32) -> Self {
        self.spot.x = x;
        self.spot.y = y;
        self
    }

    /// Width and height in CSS pixels, at least 8.
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.spot.w = width.clamp(8, MAX_SURFACE);
        self.spot.h = height.clamp(8, MAX_SURFACE);
        self
    }

    /// Lets the item move only along `axis`.
    pub fn axis(mut self, axis: Axis) -> Self {
        self.spot.axis = axis;
        self
    }
}

/// Runs the move action `A`.
type Run = fn(Ctx, SurfaceMove) -> BoxFuture<Result<()>>;

fn run<A>(ctx: Ctx, m: SurfaceMove) -> BoxFuture<Result<()>>
where
    A: Action<Input = SurfaceMove>,
    A::Output: Outcome,
{
    Box::pin(async move {
        let output = A::call(ctx, m).await?;
        output.failure().map_or(Ok(()), Err)
    })
}

/// An area of a fixed size where people move [`Draggable`] items freely.
///
/// A mouse or pen drags an item after it moves 6 pixels, a touch after a short hold. A focused
/// item moves with the arrow keys, one snap step at a time. With a single pointer, a click picks
/// an item and a click on the surface puts it there. Every move stays inside the surface, on the
/// snap grid and on the item's axis, in the browser and again on the server, which then runs the
/// move action with a [`SurfaceMove`].
///
/// ```ignore
/// #[action(permission = "floor.edit")]
/// pub async fn move_table(m: SurfaceMove, store: Tables) -> Result<()> {
///     store.place(&m.id, m.x, m.y).await
/// }
///
/// Surface::new("Floor plan", 800, 480)
///     .snap(20)
///     .on_move(move_table)
///     .item(Draggable::new("t1", Paragraph::new("Table 1")).at(40, 40))
/// ```
pub struct Surface {
    label: Text,
    width: u32,
    height: u32,
    snap: u32,
    items: Vec<Spot>,
    nodes: Vec<Node>,
    action: Option<(ActionMeta, Run)>,
}

impl Surface {
    /// A surface of `width` by `height` CSS pixels, named `label` for screen readers.
    pub fn new(label: impl Into<Text>, width: u32, height: u32) -> Self {
        Surface {
            label: label.into(),
            width: width.clamp(8, MAX_SURFACE),
            height: height.clamp(8, MAX_SURFACE),
            snap: 0,
            items: Vec::new(),
            nodes: Vec::new(),
            action: None,
        }
    }

    /// Puts every position on a grid of `step` pixels. Arrow keys move one step (10 pixels
    /// without a grid).
    pub fn snap(mut self, step: u32) -> Self {
        self.snap = step.min(1_000);
        self
    }

    /// Adds an item.
    pub fn item(mut self, item: Draggable) -> Self {
        self.items.push(item.spot);
        self.nodes.push(item.node);
        self
    }

    /// Runs `action` after each move, which stores the position. Without it moves stay in the
    /// page until it is loaded again.
    pub fn on_move<A>(mut self, _action: A) -> Self
    where
        A: Action<Input = SurfaceMove>,
        A::Output: Outcome,
    {
        self.action = Some((A::META, run::<A>));
        self
    }

    /// `value` kept on the grid and inside `0..=max`.
    fn fit(&self, value: i64, max: u32) -> u32 {
        let max = i64::from(max);
        let mut v = value.clamp(0, max);
        if self.snap > 1 {
            let step = i64::from(self.snap);
            v = ((v + step / 2) / step) * step;
            if v > max {
                v -= step;
            }
        }
        v.clamp(0, max) as u32
    }

    /// The place item `item` ends up at when the browser asks for `(x, y)`.
    fn place(&self, item: &Spot, x: i64, y: i64) -> (u32, u32) {
        let x = match item.axis {
            Axis::Y => i64::from(item.x),
            _ => x,
        };
        let y = match item.axis {
            Axis::X => i64::from(item.y),
            _ => y,
        };
        (
            self.fit(x, self.width.saturating_sub(item.w)),
            self.fit(y, self.height.saturating_sub(item.h)),
        )
    }
}

impl From<Surface> for Node {
    fn from(s: Surface) -> Self {
        Node::new(s)
    }
}

/// What the browser sends for a move.
#[derive(serde::Deserialize)]
struct MoveCall {
    id: String,
    x: i64,
    y: i64,
}

impl DataSource for Surface {
    fn action(&self) -> ActionMeta {
        self.action
            .map(|(meta, _)| meta)
            .expect("source() offers a surface only with a move action")
    }

    fn fetch(&self, ctx: Ctx, input: Json) -> BoxFuture<Result<Json>> {
        let call: Option<MoveCall> = serde_json::from_value(input).ok();
        let found = call.and_then(|c| {
            let item = self
                .items
                .iter()
                .find(|i| c.id.len() <= MAX_ID && i.id == c.id)?;
            Some((item.id.clone(), self.place(item, c.x, c.y)))
        });
        let run = self.action.map(|(_, run)| run);
        Box::pin(async move {
            let ((id, (x, y)), run) = found.zip(run).ok_or(Error::NotFound)?;
            run(ctx, SurfaceMove { id, x, y }).await?;
            Ok(json!({ "x": x, "y": y }))
        })
    }
}

impl Component for Surface {
    fn render(&self, out: &mut String, r: &Render) {
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.surface.{key}")));
        let id = dom_id(r, "surface");
        out.push_str(&format!(
            r#"<div class="z-surface-scroll"><div class="z-surface" role="group" aria-label="{}" aria-describedby="{id}-hint" data-zeyn-surface="{id}" data-snap="{}"{} data-text-moved="{}" data-text-picked="{}" data-text-refused="{}" style="--z-surface-w:{}px;--z-surface-h:{}px">"#,
            r.text(&self.label),
            self.snap,
            if self.action.is_some() {
                " data-save"
            } else {
                ""
            },
            t("moved"),
            t("picked"),
            t("refused"),
            self.width,
            self.height,
        ));
        out.push_str(&format!(
            r#"<p class="z-sr-only" id="{id}-hint">{}</p>"#,
            t("hint")
        ));
        for (i, item) in self.items.iter().enumerate() {
            let label = item
                .label
                .as_ref()
                .map_or_else(|| escape(&item.id), |l| r.text(l));
            let (x, y) = self.place(item, i64::from(item.x), i64::from(item.y));
            out.push_str(&format!(
                r#"<div class="z-surface-item" tabindex="0" aria-roledescription="{}" aria-label="{label}" aria-describedby="{id}-hint" data-id="{}" data-axis="{}" style="left:{x}px;top:{y}px;width:{}px;height:{}px">"#,
                t("item"),
                escape(&item.id),
                item.axis.attr(),
                item.w,
                item.h,
            ));
            render_child(out, r, i, &self.nodes[i]);
            out.push_str("</div>");
        }
        out.push_str("</div></div>");
    }

    fn schema(&self) -> Json {
        json!({
            "type": "Surface",
            "width": self.width,
            "height": self.height,
            "snap": self.snap,
            "items": self
                .items
                .iter()
                .zip(&self.nodes)
                .map(|(i, n)| json!({ "id": i.id, "x": i.x, "y": i.y, "node": n.schema() }))
                .collect::<Vec<_>>(),
        })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }

    fn source(&self, key: &str) -> Option<&dyn DataSource> {
        (key == "move" && self.action.is_some()).then_some(self as &dyn DataSource)
    }
}
