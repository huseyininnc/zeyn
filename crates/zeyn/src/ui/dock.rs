//! Docking panels: a center area with tabs and panel areas at the left, right and bottom, as in
//! an IDE. Users resize the areas, drag tabs between them, close panels and bring them back from
//! the panel options or a menu bar; the layout is saved per user and screen.

use super::{dom_id, render_child};
use crate::grid::{ColumnView, DataGridView, Pin};
use crate::i18n::Text;
use crate::render::{Render, attr, escape};
use crate::screen::{Component, Node};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

super::layout::into_node!(Dock);

/// Where a dock panel sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockArea {
    Left,
    Right,
    Bottom,
    Center,
}

impl DockArea {
    pub const ALL: [DockArea; 4] = [DockArea::Left, DockArea::Center, DockArea::Bottom, DockArea::Right];

    pub fn code(self) -> &'static str {
        match self {
            DockArea::Left => "left",
            DockArea::Right => "right",
            DockArea::Bottom => "bottom",
            DockArea::Center => "center",
        }
    }

    fn parse(s: &str) -> Option<DockArea> {
        DockArea::ALL.into_iter().find(|a| a.code() == s)
    }
}

/// Sizes of the side areas in percent of the dock: left and right of its width, bottom of its
/// height.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockSizes {
    pub left: u8,
    pub right: u8,
    pub bottom: u8,
}

impl Default for DockSizes {
    fn default() -> Self {
        DockSizes { left: 22, right: 24, bottom: 30 }
    }
}

/// Smallest and largest share of a side area, in percent.
pub const DOCK_SIZE: (u8, u8) = (10, 60);

/// One panel in a saved layout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockPanelState {
    pub id: String,
    pub area: DockArea,
    pub open: bool,
    /// The panel shown in its area.
    #[serde(default)]
    pub active: bool,
}

/// How a user arranged a dock: sizes and, in order, where each panel is and whether it is open.
/// It comes from the client, so the server cleans it with [`DockLayout::clean`] before use.
///
/// It is kept in the app's [`crate::DataGridViewStore`] (one store for every per user layout)
/// as a [`DataGridView`]; see [`DockLayout::to_view`].
///
/// ```
/// use zeyn::ui::{DockArea, DockLayout, DockPanelState, DockSizes};
/// let layout = DockLayout {
///     sizes: DockSizes { left: 5, right: 30, bottom: 90 },
///     panels: vec![
///         DockPanelState { id: "files".into(), area: DockArea::Right, open: true, active: true },
///         DockPanelState { id: "nope".into(), area: DockArea::Left, open: true, active: false },
///         DockPanelState { id: "files".into(), area: DockArea::Left, open: false, active: false },
///     ],
/// };
/// let clean = layout.clean(&["files", "log"]);
/// assert_eq!(clean.sizes, DockSizes { left: 10, right: 30, bottom: 60 });
/// assert_eq!(clean.panels.len(), 1);
/// assert_eq!(DockLayout::from_view(&clean.to_view()), Some(clean));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DockLayout {
    #[serde(default)]
    pub sizes: DockSizes,
    #[serde(default)]
    pub panels: Vec<DockPanelState>,
}

const SIZE_PREFIX: &str = "size:";
const PANEL_PREFIX: &str = "panel:";

impl DockLayout {
    /// Keeps only panels in `known`, each once, and sizes within [`DOCK_SIZE`].
    pub fn clean(mut self, known: &[&str]) -> DockLayout {
        let clamp = |v: u8| v.clamp(DOCK_SIZE.0, DOCK_SIZE.1);
        self.sizes = DockSizes { left: clamp(self.sizes.left), right: clamp(self.sizes.right), bottom: clamp(self.sizes.bottom) };
        let mut seen: Vec<String> = Vec::new();
        self.panels.retain(|p| {
            let keep = known.contains(&p.id.as_str()) && !seen.contains(&p.id);
            if keep {
                seen.push(p.id.clone());
            }
            keep
        });
        self
    }

    /// The layout in the shape of a saved grid view: `size:left` columns carry a size in
    /// `width`, `panel:{area}:{id}` columns carry `width` 1 when open and 0 when closed and pin
    /// `start` on the active panel.
    pub fn to_view(&self) -> DataGridView {
        let size = |name: &str, v: u8| ColumnView { field: format!("{SIZE_PREFIX}{name}"), width: Some(v.into()), pin: None };
        let mut columns = vec![size("left", self.sizes.left), size("right", self.sizes.right), size("bottom", self.sizes.bottom)];
        columns.extend(self.panels.iter().map(|p| ColumnView {
            field: format!("{PANEL_PREFIX}{}:{}", p.area.code(), p.id),
            width: Some(p.open.into()),
            pin: p.active.then_some(Pin::Start),
        }));
        DataGridView { columns, ..DataGridView::default() }
    }

    /// Reads a layout written by [`DockLayout::to_view`]. `None` when it holds anything else.
    pub fn from_view(view: &DataGridView) -> Option<DockLayout> {
        let mut layout = DockLayout::default();
        for c in &view.columns {
            if let Some(name) = c.field.strip_prefix(SIZE_PREFIX) {
                let v = u8::try_from(c.width?).ok()?;
                match name {
                    "left" => layout.sizes.left = v,
                    "right" => layout.sizes.right = v,
                    "bottom" => layout.sizes.bottom = v,
                    _ => return None,
                }
            } else {
                let (area, id) = c.field.strip_prefix(PANEL_PREFIX)?.split_once(':')?;
                layout.panels.push(DockPanelState {
                    id: id.to_string(),
                    area: DockArea::parse(area)?,
                    open: c.width.unwrap_or(1) > 0,
                    active: c.pin.is_some(),
                });
            }
        }
        Some(layout)
    }
}

struct DockPanel {
    id: &'static str,
    title: Text,
    area: DockArea,
}

/// Panels docked left, right, bottom and as center tabs. Every panel holds a node, so grids and
/// forms keep working wherever the user moves them.
///
/// Users resize the side areas with the pointer or, on the focused divider, with the arrow keys
/// (Home and End jump to the limits); move between the tabs of an area with Left and Right;
/// close the focused panel with Delete; and open the panel options (the area's button,
/// Shift+F10 or the context menu key) to move a panel to another area, close it or show a
/// closed one. Tabs can also be dragged between areas. Add [`super::MenuList::panels`] to a menu
/// bar to show and hide panels from there. The layout is saved per user and screen in the
/// app's view store (`App::datagrid_views`), or in the browser without one.
///
/// ```ignore
/// Dock::new("Workspace")
///     .left("files", "Files", tree)
///     .center("orders", "Orders", grid)
///     .center("chart", "Chart", chart)
///     .bottom("log", "Log", log_grid)
///     .right("props", "Properties", inspector)
/// ```
pub struct Dock {
    label: Text,
    panels: Vec<DockPanel>,
    nodes: Vec<Node>,
    sizes: DockSizes,
}

impl Dock {
    /// A dock with an accessible name, such as "Workspace".
    pub fn new(label: impl Into<Text>) -> Self {
        Self { label: label.into(), panels: Vec::new(), nodes: Vec::new(), sizes: DockSizes::default() }
    }

    /// Adds a panel. `id` names it in saved layouts: keep it stable, lower case, unique in the
    /// dock.
    pub fn panel(mut self, area: DockArea, id: &'static str, title: impl Into<Text>, node: impl Into<Node>) -> Self {
        self.panels.push(DockPanel { id, title: title.into(), area });
        self.nodes.push(node.into());
        self
    }

    pub fn left(self, id: &'static str, title: impl Into<Text>, node: impl Into<Node>) -> Self {
        self.panel(DockArea::Left, id, title, node)
    }

    pub fn right(self, id: &'static str, title: impl Into<Text>, node: impl Into<Node>) -> Self {
        self.panel(DockArea::Right, id, title, node)
    }

    pub fn bottom(self, id: &'static str, title: impl Into<Text>, node: impl Into<Node>) -> Self {
        self.panel(DockArea::Bottom, id, title, node)
    }

    pub fn center(self, id: &'static str, title: impl Into<Text>, node: impl Into<Node>) -> Self {
        self.panel(DockArea::Center, id, title, node)
    }

    /// First sizes of the side areas in percent, each kept within [`DOCK_SIZE`].
    pub fn sizes(mut self, left: u8, right: u8, bottom: u8) -> Self {
        self.sizes = DockLayout { sizes: DockSizes { left, right, bottom }, panels: Vec::new() }.clean(&[]).sizes;
        self
    }

    /// Ids of the panels, in order.
    pub fn panel_ids(&self) -> Vec<&'static str> {
        self.panels.iter().map(|p| p.id).collect()
    }
}

impl Component for Dock {
    fn render(&self, out: &mut String, r: &Render) {
        let id = dom_id(r, "dock");
        let t = |key: &str| r.text(&Text::key(format!("zeyn.ui.dock.{key}")));
        let s = self.sizes;
        out.push_str(&format!(
            r#"<div class="z-dock" role="group" aria-label="{}" data-zeyn-dock="{id}" data-testid="{}.dock" style="--z-dock-left:{}%;--z-dock-right:{}%;--z-dock-bottom:{}%""#,
            r.text(&self.label),
            r.scope,
            s.left,
            s.right,
            s.bottom
        ));
        for key in ["left", "right", "bottom", "center", "options", "move", "close", "show", "moved", "closed", "shown", "empty"] {
            out.push_str(&format!(r#" data-text-{key}="{}""#, t(key)));
        }
        let titles: Vec<Json> = self.panels.iter().map(|p| json!({ "id": p.id, "title": r.text(&p.title) })).collect();
        out.push_str(&format!(r#" data-panels="{}">"#, attr(&titles)));
        for area in DockArea::ALL {
            let code = area.code();
            if area == DockArea::Bottom {
                out.push_str(&self.bar(&id, "bottom", s.bottom, r));
            }
            if area == DockArea::Right {
                out.push_str(&self.bar(&id, "right", s.right, r));
            }
            let mine: Vec<usize> = (0..self.panels.len()).filter(|i| self.panels[*i].area == area).collect();
            let hidden = if mine.is_empty() && area != DockArea::Center { " hidden" } else { "" };
            let area_name = t(code);
            out.push_str(&format!(
                r#"<section class="z-dock-area" data-area="{code}" aria-label="{area_name}"{hidden}><div class="z-dock-head"><div class="z-dock-tabs" role="tablist" aria-label="{area_name}">"#
            ));
            for (n, i) in mine.iter().enumerate() {
                let p = &self.panels[*i];
                let pid = escape(p.id);
                out.push_str(&format!(
                    r#"<button type="button" role="tab" class="z-dock-tab" id="{id}-tab-{pid}" aria-controls="{id}-panel-{pid}" aria-selected="{}" tabindex="{}" draggable="true" data-panel="{pid}" data-testid="{}.dock.{pid}">{}</button>"#,
                    n == 0,
                    if n == 0 { 0 } else { -1 },
                    r.scope,
                    r.text(&p.title)
                ));
            }
            out.push_str(&format!(
                r#"</div><button type="button" class="z-dock-options" aria-haspopup="menu" aria-expanded="false" aria-label="{} ({area_name})" data-testid="{}.dock.options.{code}"><span aria-hidden="true">&#8943;</span></button></div><div class="z-dock-body">"#,
                t("options"),
                r.scope
            ));
            for (n, i) in mine.iter().enumerate() {
                let pid = escape(self.panels[*i].id);
                let hidden = if n == 0 { "" } else { " hidden" };
                out.push_str(&format!(
                    r#"<div class="z-dock-panel" role="tabpanel" id="{id}-panel-{pid}" aria-labelledby="{id}-tab-{pid}" tabindex="0" data-panel="{pid}"{hidden}>"#
                ));
                render_child(out, r, *i, &self.nodes[*i]);
                out.push_str("</div>");
            }
            out.push_str("</div></section>");
            if area == DockArea::Left {
                out.push_str(&self.bar(&id, "left", s.left, r));
            }
        }
        out.push_str(r#"<p class="z-sr-only" aria-live="polite" data-zeyn-dock-live></p></div>"#);
    }

    fn schema(&self) -> Json {
        let panels: Vec<Json> =
            self.panels.iter().zip(&self.nodes).map(|(p, n)| json!({ "id": p.id, "title": p.title, "area": p.area, "node": n.schema() })).collect();
        json!({ "type": "dock", "label": self.label, "sizes": self.sizes, "panels": panels })
    }

    fn children(&self) -> &[Node] {
        &self.nodes
    }
}

impl Dock {
    fn bar(&self, id: &str, edge: &str, value: u8, r: &Render) -> String {
        let orientation = if edge == "bottom" { "horizontal" } else { "vertical" };
        let label = r.text(&Text::key("zeyn.ui.dock.resize").arg("area", r.text(&Text::key(format!("zeyn.ui.dock.{edge}")))));
        format!(
            r#"<div class="z-dock-bar" data-edge="{edge}" role="separator" tabindex="0" aria-orientation="{orientation}" aria-valuenow="{value}" aria-valuemin="{}" aria-valuemax="{}" aria-label="{label}" data-testid="{}.dock.resize.{edge}" data-id="{id}"></div>"#,
            DOCK_SIZE.0,
            DOCK_SIZE.1,
            r.scope
        )
    }
}

/// Panel ids of a dock from its schema; `None` when the schema is not a dock's.
pub(crate) fn panel_ids(schema: &Json) -> Option<Vec<String>> {
    if schema.get("type")?.as_str()? != "dock" {
        return None;
    }
    Some(schema.get("panels")?.as_array()?.iter().filter_map(|p| p.get("id")?.as_str().map(String::from)).collect())
}
