//! The diagram as a Y-CRDT document (feature `crdt`), shared by the server's document store and
//! the island. Root map `shapes`: shape id to the shape as JSON. Two people moving different
//! shapes never conflict; two moving the same shape end with the last write.

use crate::{Diagram, Shape};
use std::collections::HashMap;
use yrs::updates::decoder::Decode;
use yrs::{Any, Map, Out, ReadTxn, StateVector, Transact, Update};

pub use yrs::Doc;

const SHAPES: &str = "shapes";

/// The diagram held by `doc`, sorted by id and within the limits.
pub fn read(doc: &Doc) -> Diagram {
    let map = doc.get_or_insert_map(SHAPES);
    let txn = doc.transact();
    let mut shapes: Vec<Shape> = map
        .iter(&txn)
        .filter_map(|(_, v)| match v {
            Out::Any(Any::String(s)) => serde_json::from_str(&s).ok(),
            _ => None,
        })
        .collect();
    shapes.sort_by_key(|s| s.id);
    Diagram { shapes }.clamped()
}

/// Brings `doc` to `diagram`, touching only shapes that changed. Returns the update to send to
/// the others, or `None` when nothing changed.
pub fn write(doc: &Doc, diagram: &Diagram) -> Option<Vec<u8>> {
    let map = doc.get_or_insert_map(SHAPES);
    let before = doc.transact().state_vector();
    {
        let mut txn = doc.transact_mut();
        let current: HashMap<String, String> = map
            .iter(&txn)
            .filter_map(|(k, v)| match v {
                Out::Any(Any::String(s)) => Some((k.to_string(), s.to_string())),
                _ => None,
            })
            .collect();
        let mut keep = Vec::with_capacity(diagram.shapes.len());
        for s in &diagram.shapes {
            let key = s.id.to_string();
            let json = serde_json::to_string(s).ok()?;
            if current.get(&key) != Some(&json) {
                map.insert(&mut txn, key.clone(), Any::from(json));
            }
            keep.push(key);
        }
        for key in current
            .keys()
            .filter(|k| !keep.contains(k))
        {
            map.remove(&mut txn, key);
        }
    }
    let update = doc
        .transact()
        .encode_state_as_update_v1(&before);
    (doc.transact().state_vector() != before).then_some(update)
}

/// Applies an update from the server. `false` when it cannot be read.
pub fn apply(doc: &Doc, update: &[u8]) -> bool {
    match Update::decode_v1(update) {
        Ok(u) => doc
            .transact_mut()
            .apply_update(u)
            .is_ok(),
        Err(_) => false,
    }
}

/// The whole document as one update.
pub fn state(doc: &Doc) -> Vec<u8> {
    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

/// A document holding `diagram`.
pub fn from_diagram(diagram: &Diagram) -> Doc {
    let doc = Doc::new();
    write(&doc, diagram);
    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShapeKind;

    fn shape(id: u32, x: i32) -> Shape {
        Shape {
            id,
            kind: ShapeKind::Box,
            x,
            y: 10,
            w: 100,
            h: 50,
            label: format!("S{id}"),
        }
    }

    #[test]
    fn two_editors_converge_through_updates() {
        let a = from_diagram(&Diagram {
            shapes: vec![shape(1, 10), shape(2, 20)],
        });
        let b = Doc::new();
        assert!(apply(&b, &state(&a)));
        assert_eq!(read(&b), read(&a));

        let ua = write(
            &a,
            &Diagram {
                shapes: vec![shape(1, 99), shape(2, 20)],
            },
        )
        .unwrap();
        let ub = write(
            &b,
            &Diagram {
                shapes: vec![shape(1, 10), shape(3, 30)],
            },
        )
        .unwrap();
        assert!(apply(&a, &ub) && apply(&b, &ua));
        let both = read(&a);
        assert_eq!(both, read(&b));
        assert_eq!(
            both.shapes
                .iter()
                .map(|s| (s.id, s.x))
                .collect::<Vec<_>>(),
            [(1, 99), (3, 30)]
        );
        assert!(write(&a, &both).is_none());
    }
}
