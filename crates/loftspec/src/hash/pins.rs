//! The copies of other documents' Interfaces in the hashes: each node of a copy hashed as its
//! own document hashed it, for the edges that name it, and the pins of the copies, which the
//! state view covers where a tree holds a copy.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::tree;
use crate::canonical::Digest;
use crate::document::Document;
use crate::ids::NodeId;

/// The content hash of every node of every copy, as its own document hashes it.
pub(super) fn copy_nodes(document: &Document) -> BTreeMap<NodeId, Digest> {
    let mut nodes = BTreeMap::new();
    let mut visit = |id: Option<NodeId>, _: &tree::NodeView<'_>, hash: Digest| {
        if let Some(id) = id {
            nodes.insert(id, hash);
        }
    };
    for copy in &document.copies {
        tree::hash_with((&copy.interface).into(), &mut visit);
    }
    nodes
}

/// The pins of a tree's copies: each as its other document's key, its Interface's key, its
/// publish sequence, its state root and its Interface hash, sorted by document key and then by
/// Interface key; none for a tree without a copy. A copy whose handle names no other document,
/// which the reader refuses, names no document key.
pub(super) fn view(document: &Document) -> Option<Vec<Value>> {
    if document.copies.is_empty() {
        return None;
    }
    let mut pins: Vec<(String, String, Value)> = (document.copies.iter())
        .map(|copy| {
            let other = (document.documents.iter()).find(|other| other.handle == copy.handle);
            let key = other.map_or_else(String::new, |other| other.key.to_string());
            let interface = copy
                .interface
                .id
                .map_or_else(String::new, |id| id.to_string());
            let pin = json!([
                other.map(|other| other.key),
                copy.interface.id,
                copy.publish_sequence,
                copy.state_root,
                copy.interface_hash,
            ]);
            (key, interface, pin)
        })
        .collect();
    pins.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
    Some(pins.into_iter().map(|(_, _, pin)| pin).collect())
}
