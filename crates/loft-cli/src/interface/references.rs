//! The references of a document into its copies: each edge whose target is a node of a copy,
//! named by the element that makes it and the field its record writes it in.

use std::collections::BTreeSet;

use loftspec::document::{Document, EdgeKind, EdgeTarget};
use loftspec::identity::Key;
use loftspec::ids::ElementId;

/// A reference of the document to a node of a copy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Reference {
    /// The element that makes it, by its label or, without one, its key.
    pub element: String,
    /// The field its record writes it in.
    pub field: &'static str,
    /// The key of the node it names.
    pub key: Key,
}

/// Each reference of `document` to a node keyed in `keys`, in the order of its element, its
/// field and the key it names.
pub(super) fn references(document: &Document, keys: &BTreeSet<Key>) -> Vec<Reference> {
    let mut found: Vec<Reference> = (document.edges.iter())
        .filter_map(|edge| {
            let EdgeTarget::Element(target) = edge.target() else {
                return None;
            };
            let key = target.key();
            keys.contains(&key).then(|| Reference {
                element: name(document, edge.source()),
                field: field(edge.kind()),
                key,
            })
        })
        .collect();
    found.sort();
    found.dedup();
    found
}

/// The element's label, or its key where it has none.
fn name(document: &Document, element: ElementId) -> String {
    document
        .bindings
        .label_of(element)
        .map_or_else(|| element.key().to_string(), |label| label.to_string())
}

/// The field of an element's record that writes an edge of `kind`.
fn field(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Subject => "subjects",
        other => other.as_str(),
    }
}
