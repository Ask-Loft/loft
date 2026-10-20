//! The canonical JSON export (F-25): RFC 8785 canonical JSON of the content view, with each
//! node's and decision's key and label beside its view, every edge, both registries, the term
//! list when it has an entry, and the state root it was rendered from. Its layout is published with the golden vectors (AC-655):
//!
//! ```text
//! { "format", "stateRoot",
//!   "nodes":      [{ "key", "label", "kind", "view", "ext"? }], in document order; `ext` only
//!                                                            on a node that carries it
//!   "decisions":  [{ "key", "label", "view" }],              by key
//!   "edges":      [{ "type", "source", "target", "role" }],  by canonical bytes
//!   "tags":       [{ "key", "name", "definition" }],         by name
//!   "categories": [{ "key", "name", "definition" }],         by name
//!   "terms"?:     [{ "key", "name", "definition", "aliases" }] } by name; only when the
//!                                                            document has a term
//! ```
//!
//! A label or role that is absent is `null`; a `subject` edge to a node the document no longer
//! holds has the target `{"formerLabel": <label>}`.

use serde_json::{Value, json};

use super::Labels;
use crate::canonical::canonical_bytes;
use crate::document::{Document, EdgeTarget, SubjectRole};
use crate::hash::Views;

pub(super) fn render(document: &Document, state: &str, views: &Views, labels: &Labels) -> Vec<u8> {
    let nodes = nodes(document, views, labels);
    let decisions: Vec<Value> = views
        .decisions
        .iter()
        .map(|(id, view)| json!({ "key": id.key(), "label": labels.label(*id), "view": view }))
        .collect();
    let mut edges: Vec<(Vec<u8>, Value)> = document
        .edges
        .iter()
        .map(|edge| {
            let target = match edge.target() {
                EdgeTarget::Element(element) => json!(element.key()),
                EdgeTarget::Former(label) => json!({ "formerLabel": label }),
            };
            let edge = json!({
                "type": edge.kind().as_str(),
                "source": edge.source().key(),
                "target": target,
                "role": edge.role().map(SubjectRole::as_str),
            });
            (canonical_bytes(&edge), edge)
        })
        .collect();
    edges.sort_by(|left, right| left.0.cmp(&right.0));
    let registry = |entries: Vec<(String, String, String)>| {
        let mut entries = entries;
        entries.sort();
        entries
            .into_iter()
            .map(|(name, key, definition)| json!({ "key": key, "name": name, "definition": definition }))
            .collect::<Vec<_>>()
    };
    let tags = registry(
        document
            .tags
            .iter()
            .map(|e| {
                (
                    e.name.to_string(),
                    e.id.key().to_string(),
                    e.definition.clone(),
                )
            })
            .collect(),
    );
    let categories = registry(
        document
            .categories
            .iter()
            .map(|e| {
                (
                    e.name.to_string(),
                    e.id.key().to_string(),
                    e.definition.clone(),
                )
            })
            .collect(),
    );
    let mut export = json!({
        "format": crate::FORMAT,
        "stateRoot": state,
        "nodes": nodes,
        "decisions": decisions,
        "edges": edges.into_iter().map(|(_, edge)| edge).collect::<Vec<_>>(),
        "tags": tags,
        "categories": categories,
    });
    if !document.terms.is_empty() {
        export["terms"] = terms(document);
    }
    canonical_bytes(&export)
}

/// The term list, sorted by name and then key, each entry's aliases sorted.
fn terms(document: &Document) -> Value {
    let mut terms: Vec<_> = document.terms.iter().collect();
    terms.sort_by_key(|term| (term.name.as_str(), term.id));
    terms
        .into_iter()
        .map(|term| {
            let mut aliases: Vec<&str> = term.aliases.iter().map(String::as_str).collect();
            aliases.sort_unstable();
            json!({
                "key": term.id.key(),
                "name": term.name,
                "definition": term.definition,
                "aliases": aliases,
            })
        })
        .collect()
}

/// Each keyed node in document order: its key, label, kind and view, and its `ext` when it
/// carries one.
fn nodes(document: &Document, views: &Views, labels: &Labels) -> Vec<Value> {
    let mut nodes = Vec::new();
    document.walk(&mut |node| {
        if let Some(id) = node.id() {
            let mut entry = json!({
                "key": id.key(),
                "label": labels.label(id),
                "kind": id.kind().as_str(),
                "view": views.nodes[&id],
            });
            if !node.ext().is_empty() {
                entry["ext"] = crate::ext::to_json(node.ext());
            }
            nodes.push(entry);
        }
    });
    nodes
}
