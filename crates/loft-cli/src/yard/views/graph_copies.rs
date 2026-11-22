//! What a `spec-change` view's diagram draws of the other documents a connected document relies
//! on (the See a spec change Feature): each copy of another document's Interface that the branch
//! added or removed, each copy whose pin it moved with the nodes of the promise that the move
//! added, changed or removed, and each Interface that a Component gained in its `provides` or
//! `requires`, so that the edge ends at its box.

use std::collections::{BTreeMap, BTreeSet};

use loftspec::copy::Copy;
use loftspec::diff::interface_changes;
use loftspec::document::{Document, Edge};
use loftspec::identity::Key;

/// Each copy of `document`, by its handle and its Interface's key.
fn by_pin(document: &Document) -> BTreeMap<(String, Key), &Copy> {
    (document.copies.iter())
        .map(|copy| ((copy.handle.to_string(), copy.key()), copy))
        .collect()
}

/// A copy that both documents hold, as the base held it and as the tree holds it.
pub(super) struct Moved<'a> {
    pub before: &'a Copy,
    pub after: &'a Copy,
}

/// The copies the branch added, removed and whose pin or bytes it changed.
pub(super) struct Copies<'a> {
    pub added: Vec<&'a Copy>,
    pub removed: Vec<&'a Copy>,
    pub moved: Vec<Moved<'a>>,
}

impl<'a> Copies<'a> {
    pub fn of(base: &'a Document, document: &'a Document) -> Self {
        let (then, now) = (by_pin(base), by_pin(document));
        let added = (now.iter())
            .filter(|(pin, _)| !then.contains_key(*pin))
            .map(|(_, copy)| *copy)
            .collect();
        let removed = (then.iter())
            .filter(|(pin, _)| !now.contains_key(*pin))
            .map(|(_, copy)| *copy)
            .collect();
        let moved = (now.iter())
            .filter_map(|(pin, after)| {
                let before = *then.get(pin)?;
                (before.bytes != after.bytes).then_some(Moved { before, after })
            })
            .collect();
        Self {
            added,
            removed,
            moved,
        }
    }

    /// The change of each node of a copy that the diagram marks, by its key: an added or removed
    /// copy's Interface, and a moved copy's Interface with each node of its promise that the move
    /// added, changed or removed.
    pub fn marks(&self) -> BTreeMap<Key, &'static str> {
        let mut marks = BTreeMap::new();
        marks.extend(self.added.iter().map(|copy| (copy.key(), "added")));
        marks.extend(self.removed.iter().map(|copy| (copy.key(), "removed")));
        for Moved { before, after } in &self.moved {
            let changes = interface_changes(&before.interface, &after.interface);
            let lists = [
                (&changes.added, "added"),
                (&changes.changed, "changed"),
                (&changes.removed, "removed"),
            ];
            for (ids, change) in lists {
                marks.extend(ids.iter().map(|id| (id.key(), change)));
            }
            marks.entry(after.key()).or_insert("changed");
        }
        marks
    }
}

/// The Interfaces that a Component of `document` gained in its `provides` or `requires` against
/// `base`, of the document or of a copy.
pub(super) fn gained_interfaces(base: &Document, document: &Document) -> BTreeSet<Key> {
    (document.edges.iter())
        .filter(|edge| !base.edges.contains(edge))
        .filter_map(|edge| match *edge {
            Edge::Provides { interface, .. } | Edge::Requires { interface, .. } => {
                Some(interface.key())
            }
            _ => None,
        })
        .collect()
}
