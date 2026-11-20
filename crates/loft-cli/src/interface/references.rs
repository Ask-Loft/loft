//! The references of a document into its copies, each named by the element that makes it and
//! the field its record writes it in.

use std::collections::BTreeSet;

use loftspec::copy;
use loftspec::document::Document;
use loftspec::identity::Key;

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
    let mut found: Vec<Reference> = (copy::references(&document.edges, keys).into_iter())
        .map(|reference| Reference {
            element: document.bindings.label_of(reference.source).map_or_else(
                || reference.source.key().to_string(),
                |label| label.to_string(),
            ),
            field: reference.kind.field(),
            key: reference.key,
        })
        .collect();
    found.sort();
    found.dedup();
    found
}
