//! Naming an element by a query, as `loft find` and `loft review --node` take one (the `loft
//! find` and `loft review` Features): its key, its slug, or its label in any case.

use serde_json::{Map, Value, json};

use crate::codec::Site;
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

/// The fields that name an element, as `loft find` and the review packet give it: its key, label
/// and kind, its site where it has one, and its name.
#[must_use]
pub fn element_fields(
    key: Key,
    label: Option<Label>,
    kind: &str,
    site: Option<&Site>,
    name: &str,
) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("key".to_owned(), json!(key.to_string()));
    fields.insert(
        "label".to_owned(),
        json!(label.map(|label| label.to_string())),
    );
    fields.insert("kind".to_owned(), json!(kind));
    if let Some(site) = site {
        fields.extend(site.fields());
    }
    fields.insert("name".to_owned(), json!(name));
    fields
}
