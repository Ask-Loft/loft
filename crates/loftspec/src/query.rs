//! Naming an element by a query, as `loft find` and `loft review --node` take one (the `loft
//! find` and `loft review` Features): its key, its slug, or its label in any case.

use crate::document::Document;
use crate::identity::{Key, Label};
use crate::ids::NodeId;

/// Whether `query` names the element with `key`, `label` and `slug`: the key in any case, the
/// slug exactly, the label in any case.
#[must_use]
pub fn identifies(query: &str, key: Key, label: Option<Label>, slug: Option<&str>) -> bool {
    key.to_string().eq_ignore_ascii_case(query)
        || slug == Some(query)
        || label.is_some_and(|label| label.to_string().eq_ignore_ascii_case(query))
}

/// Every keyed node of `document` that `query` names, in document order.
#[must_use]
pub fn named_nodes(document: &Document, query: &str) -> Vec<NodeId> {
    let mut found = Vec::new();
    document.walk(&mut |node| {
        let Some(id) = node.id() else { return };
        let slug = node.slug().map(crate::slug::Slug::as_str);
        if identifies(query, id.key(), document.bindings.label_of(id), slug) {
            found.push(id);
        }
    });
    found
}
