//! What a `spec-change` view's diagram draws of the other documents a connected document relies
//! on (the See a spec change Feature): each copy of another document's Interface that the branch
//! added or removed, each copy whose pin it moved with the nodes of the promise that the move
//! added, changed or removed, and each Interface that a Component gained in its `provides` or
//! `requires`, so that the edge ends at its box.

use std::collections::{BTreeMap, BTreeSet};

use loftspec::copy::Copy;
use loftspec::diff::interface_changes;
use loftspec::document::{Document, Edge, OtherDocument};
use loftspec::identity::Key;
use serde_json::{Value, json};

use super::words;

/// Each copy of `document`, by its Interface's key, which the diagram and the diff mark it by,
/// whatever handle the document lists the other document under.
fn by_interface(document: &Document) -> BTreeMap<Key, &Copy> {
    (document.copies.iter())
        .map(|copy| (copy.key(), copy))
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
        let (then, now) = (by_interface(base), by_interface(document));
        let added = (now.iter())
            .filter(|(key, _)| !then.contains_key(*key))
            .map(|(_, copy)| *copy)
            .collect();
        let removed = (then.iter())
            .filter(|(key, _)| !now.contains_key(*key))
            .map(|(_, copy)| *copy)
            .collect();
        let moved = (now.iter())
            .filter_map(|(key, after)| {
                let before = *then.get(key)?;
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

    /// The publish sequence of each added and removed copy's pin, and of each moved copy's pin
    /// before and after where the move changed it, by its Interface's key, for its entry in the
    /// diff.
    pub fn pins(&self) -> BTreeMap<Key, Value> {
        let pin = |before: Option<&Copy>, after: Option<&Copy>| {
            json!({
                "before": before.map(|copy| copy.publish_sequence),
                "after": after.map(|copy| copy.publish_sequence),
            })
        };
        let added = (self.added.iter()).map(|copy| (copy.key(), pin(None, Some(copy))));
        let removed = (self.removed.iter()).map(|copy| (copy.key(), pin(Some(copy), None)));
        let moved = (self.moved.iter())
            .filter(|moved| moved.before.publish_sequence != moved.after.publish_sequence)
            .map(|moved| {
                (
                    moved.after.key(),
                    pin(Some(moved.before), Some(moved.after)),
                )
            });
        added.chain(removed).chain(moved).collect()
    }
}

/// The diff's entry of each entry of `documents` that the branch added, removed or whose address
/// it changed: its handle, its change and its address before and after.
pub(super) fn other_documents(base: &Document, document: &Document) -> Vec<Value> {
    let by_handle = |document: &Document| -> BTreeMap<String, String> {
        (document.documents.iter())
            .map(|OtherDocument { handle, remote, .. }| (handle.to_string(), remote.clone()))
            .collect()
    };
    let (then, now) = (by_handle(base), by_handle(document));
    let handles: BTreeSet<&String> = then.keys().chain(now.keys()).collect();
    handles
        .into_iter()
        .filter_map(|handle| {
            let (before, after) = (then.get(handle), now.get(handle));
            let change = match (before, after) {
                (None, Some(_)) => "added",
                (Some(_), None) => "removed",
                (Some(before), Some(after)) if before != after => "changed",
                _ => return None,
            };
            Some(json!({
                "key": format!("document:{handle}"),
                "kind": "document",
                "label": null,
                "name": handle,
                "change": change,
                "words": words::pair(before.map(String::as_str), after.map(String::as_str)),
            }))
        })
        .collect()
}

/// The Interfaces that a Component of `document` gained in its `provides` or `requires` against
/// `base`, of the document or of a copy.
pub(super) fn gained_interfaces(base: &Document, document: &Document) -> BTreeSet<Key> {
    let had: Vec<&Edge> = promises(base).collect();
    promises(document)
        .filter(|edge| !had.contains(edge))
        .filter_map(|edge| match *edge {
            Edge::Provides { interface, .. } | Edge::Requires { interface, .. } => {
                Some(interface.key())
            }
            _ => None,
        })
        .collect()
}

/// The `provides` and `requires` edges of `document`, few beside its other edges.
fn promises(document: &Document) -> impl Iterator<Item = &Edge> {
    (document.edges.iter())
        .filter(|edge| matches!(edge, Edge::Provides { .. } | Edge::Requires { .. }))
}
