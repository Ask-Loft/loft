//! Node content views and the index of keyed nodes' content hashes (RQ-11).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::HashError;
use crate::canonical::{Digest, Domain, digest};
use crate::identity::Key;
use crate::model::Node;

/// The content view of a node whose children hash to `children`: kind, statement or name, body,
/// sorted tag names, the five typed attributes, then the ordered child content hashes. The view
/// is a JSON array so that RFC 8785, which sorts object members, keeps this order.
pub(crate) fn view_with_children(node: &Node, children: &[Digest]) -> Value {
    let tags: BTreeSet<&str> = node.tags.iter().map(String::as_str).collect();
    let attributes = &node.attributes;
    json!([
        node.kind.as_str(),
        node.title.text(),
        node.body,
        tags,
        attributes.qa_kind,
        attributes.quality_target,
        attributes.constraint_binding,
        attributes.conservation_kind,
        attributes.verification_fate,
        children,
    ])
}

/// The content view of a node.
#[must_use]
pub fn node_view(node: &Node) -> Value {
    let children: Vec<Digest> = node.children.iter().map(content_hash).collect();
    view_with_children(node, &children)
}

/// A node's content hash: the digest of its content view under `loftspec/node/1`.
#[must_use]
pub fn content_hash(node: &Node) -> Digest {
    digest(Domain::Node, &node_view(node))
}

/// Hashes a tree bottom-up once, recording the content hash of every keyed node.
pub(crate) fn index_tree(
    node: &Node,
    index: &mut BTreeMap<Key, Digest>,
) -> Result<Digest, HashError> {
    let children = node
        .children
        .iter()
        .map(|child| index_tree(child, index))
        .collect::<Result<Vec<_>, _>>()?;
    let hash = digest(Domain::Node, &view_with_children(node, &children));
    if let Some(key) = node.key
        && index.insert(key, hash).is_some()
    {
        return Err(HashError::DuplicateKey(key));
    }
    Ok(hash)
}
