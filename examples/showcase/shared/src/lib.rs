//! Types shared by the showcase server and its WASM islands.
//!
//! The server renders props with them and binds each [`Remote`] to an action of the same input
//! and output types; the islands read the props and call the remotes. A change here that one
//! side does not follow is a compile error, not a broken page.

use serde::{Deserialize, Serialize};
use zeyn_island::Remote;

/// Props of the `counter` island.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CounterProps {
    /// Value saved on the server.
    pub start: i64,
    /// Localized texts, so the island ships no translations.
    pub texts: CounterTexts,
}

/// Texts of the `counter` island.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CounterTexts {
    pub less: String,
    pub more: String,
    pub save: String,
    pub saved: String,
    pub value: String,
}

/// A counter value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Count {
    pub value: i64,
}

/// Most shapes one diagram may hold.
pub const MAX_SHAPES: usize = 200;
/// Longest shape label, in characters.
pub const MAX_LABEL: usize = 40;
/// Coordinates and sizes stay within this many CSS pixels.
pub const MAX_COORD: i32 = 4000;

/// Kind of a diagram shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    #[default]
    Box,
    Ellipse,
}

/// A labelled shape on the diagram, in CSS pixels of the unzoomed canvas.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Shape {
    pub id: u32,
    pub kind: ShapeKind,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub label: String,
}

/// A whole diagram.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Diagram {
    pub shapes: Vec<Shape>,
}

impl Diagram {
    /// The diagram within the limits: at most [`MAX_SHAPES`] shapes with unique ids, labels cut
    /// at [`MAX_LABEL`] characters, positions and sizes clamped. The server applies it to every
    /// save; the island never needs to be trusted.
    pub fn clamped(mut self) -> Diagram {
        let mut seen = std::collections::HashSet::new();
        self.shapes.retain(|s| seen.insert(s.id));
        self.shapes.truncate(MAX_SHAPES);
        for s in &mut self.shapes {
            s.x = s.x.clamp(0, MAX_COORD);
            s.y = s.y.clamp(0, MAX_COORD);
            s.w = s.w.clamp(16, 800);
            s.h = s.h.clamp(16, 800);
            s.label = s.label.chars().filter(|c| !c.is_control()).take(MAX_LABEL).collect();
        }
        self
    }

    /// One line per shape, for the text summary: `Box "Start" at 40, 40, 120 by 60`.
    pub fn summary(&self, kinds: &DiagramTexts) -> Vec<String> {
        self.shapes
            .iter()
            .map(|s| {
                let kind = if s.kind == ShapeKind::Box { &kinds.kind_box } else { &kinds.kind_ellipse };
                kinds.line.replace("{kind}", kind).replace("{label}", &s.label).replace("{x}", &s.x.to_string()).replace("{y}", &s.y.to_string()).replace("{w}", &s.w.to_string()).replace("{h}", &s.h.to_string())
            })
            .collect()
    }
}

/// Texts of the diagram editor, localized by the server.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DiagramTexts {
    pub add_box: String,
    pub add_ellipse: String,
    pub remove: String,
    pub next: String,
    pub save: String,
    pub saved: String,
    pub tools: String,
    pub canvas: String,
    pub summary: String,
    pub empty: String,
    pub selected: String,
    pub kind_box: String,
    pub kind_ellipse: String,
    /// `{kind} "{label}" at {x}, {y}, {w} by {h}`.
    pub line: String,
}

/// Props of the `diagram` island.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DiagramProps {
    pub diagram: Diagram,
    pub texts: DiagramTexts,
    /// Canvas height in CSS pixels.
    pub height: u32,
}

/// What a diagram save answers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    pub shapes: u32,
}

/// Saves a diagram; the server clamps it first.
pub struct SaveDiagram;

impl Remote for SaveDiagram {
    const NAME: &'static str = "save_diagram";
    type Input = Diagram;
    type Output = Saved;
}

/// Saves the counter; the server answers with the value it stored.
pub struct SaveCount;

impl Remote for SaveCount {
    const NAME: &'static str = "save_count";
    type Input = Count;
    type Output = Count;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamped_keeps_saves_within_limits() {
        let long = "x".repeat(100);
        let shape = |id, x| Shape { id, kind: ShapeKind::Box, x, y: -5, w: 1, h: 9000, label: format!("{long}\u{7}") };
        let mut shapes: Vec<Shape> = (0..300).map(|i| shape(i, 99_999)).collect();
        shapes.push(shape(0, 1));
        let d = Diagram { shapes }.clamped();
        assert_eq!(d.shapes.len(), MAX_SHAPES);
        let s = &d.shapes[0];
        assert_eq!((s.x, s.y, s.w, s.h), (MAX_COORD, 0, 16, 800));
        assert_eq!(s.label.chars().count(), MAX_LABEL);
        assert!(!s.label.contains('\u{7}'));
    }
}
