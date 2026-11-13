//! Tree examples: branches loaded from the server as they open.

use crate::data::product_tree;
use crate::{Entry, ex};
use std::sync::{LazyLock, Mutex};
use zeyn::prelude::*;

pub fn overview(_ctx: &Ctx) -> Node {
    Tree::of(product_tree)
        .label("Products by kind")
        .into()
}

/// Leaves drag onto a drop target, such as an island; the Insert button or Control+Enter
/// inserts the focused leaf.
pub fn draggable(_ctx: &Ctx) -> Node {
    Tree::of(product_tree)
        .label("Products to place")
        .draggable()
        .into()
}

/// A folder of the sample tree: id, label, parent and its place among its siblings.
struct Folder {
    id: &'static str,
    label: &'static str,
    parent: Option<&'static str>,
    order: u32,
}

/// The sample folders, shared by every visitor; moves only change parents and order, so the
/// store never grows.
static FOLDERS: LazyLock<Mutex<Vec<Folder>>> = LazyLock::new(|| {
    let folder = |id, label, parent, order| Folder {
        id,
        label,
        parent,
        order,
    };
    Mutex::new(vec![
        folder("docs", "Documents", None, 1),
        folder("plans", "Plans", Some("docs"), 1),
        folder("reports", "Reports", Some("docs"), 2),
        folder("q3", "Q3 report", Some("reports"), 1),
        folder("media", "Media", None, 2),
        folder("logos", "Logos", Some("media"), 1),
        folder("photos", "Photos", Some("media"), 2),
        folder("archive", "Archive", None, 3),
    ])
});

/// The folders under `q.parent`, in their order. Every folder can hold others.
#[action(public)]
pub async fn folder_tree(q: TreeQuery) -> Result<Vec<TreeItem>> {
    let all = FOLDERS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut level: Vec<&Folder> = all
        .iter()
        .filter(|f| f.parent == q.parent.as_deref())
        .collect();
    level.sort_by_key(|f| f.order);
    Ok(level
        .iter()
        .map(|f| TreeItem::branch(f.id, f.label))
        .collect())
}

/// Moves a sample folder. `TreeMove::parent` refuses a move into the folder itself or below it.
#[action(public)]
pub async fn move_folder(m: TreeMove) -> Result<()> {
    let mut all = FOLDERS
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let known = |id: &str| all.iter().any(|f| f.id == id);
    if !known(&m.id) || !known(&m.target) {
        return Err(Error::NotFound);
    }
    let parent = m.parent(|id| {
        all.iter()
            .find(|f| f.id == id)
            .and_then(|f| f.parent)
            .map(String::from)
    })?;
    let parent: Option<&'static str> = parent.and_then(|p| {
        all.iter()
            .find(|f| f.id == p)
            .map(|f| f.id)
    });
    let mut level: Vec<&'static str> = {
        let mut v: Vec<&Folder> = all
            .iter()
            .filter(|f| f.parent == parent && f.id != m.id)
            .collect();
        v.sort_by_key(|f| f.order);
        v.iter().map(|f| f.id).collect()
    };
    let at = match m.place {
        TreePlace::Inside => level.len(),
        TreePlace::Before => level
            .iter()
            .position(|id| *id == m.target)
            .unwrap_or(0),
        TreePlace::After => level
            .iter()
            .position(|id| *id == m.target)
            .map_or(level.len(), |i| i + 1),
    };
    let id = all
        .iter()
        .find(|f| f.id == m.id)
        .map(|f| f.id)
        .ok_or(Error::NotFound)?;
    level.insert(at, id);
    for f in all.iter_mut() {
        if f.id == id {
            f.parent = parent;
        }
        if let Some(i) = level.iter().position(|x| *x == f.id) {
            f.order = i as u32 + 1;
        }
    }
    Ok(())
}

/// Folders people move: drag one onto another row, use the Move button on the focused folder,
/// or Alt with an arrow key. Moving a folder into itself or below itself is refused on the server.
pub fn movable(_ctx: &Ctx) -> Node {
    Tree::of(folder_tree)
        .label("Folders")
        .movable(move_folder)
        .into()
}

/// What the catalog shows of `Tree`.
pub const ENTRY: Entry = Entry {
    name: "Tree",
    docs: "components/data",
    api: &["Tree", "TreeItem", "TreeMove"],
    overview: &[ex!(overview)],
    examples: &[ex!(draggable), ex!(movable)],
};
