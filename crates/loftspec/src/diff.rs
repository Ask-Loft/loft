//! The nodes one document added, changed and removed against another, its base (the `loft
//! status` Feature). A node is matched by its key, so a node that moves stays the same node; it
//! changed when its own content view differs, the hashes of its children left out, so a
//! change to a node is not a change to each of its ancestors. A node whose content is the
//! base's and whose parent is not moved, which is no change (the `loft review` Feature).

use crate::document::Document;
use crate::hash::own_views;
use crate::ids::NodeId;
use crate::index::Index;

/// The keyed nodes a document added, changed, moved and removed against its base: the added,
/// changed and moved in the document's order, the removed in the base's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeChanges {
    pub added: Vec<NodeId>,
    pub changed: Vec<NodeId>,
    pub removed: Vec<NodeId>,
    /// The nodes whose own content is the base's and whose parent is not.
    pub moved: Vec<NodeId>,
}

impl NodeChanges {
    /// Whether the document holds every node of its base unchanged and no other; a move is no
    /// change.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.changed.is_empty() && self.removed.is_empty()
    }
}

/// The nodes `document` added, changed and removed against `base`.
#[must_use]
pub fn node_changes(base: &Document, document: &Document) -> NodeChanges {
    let before = own_views(base);
    let after = own_views(document);
    let (then, _, _) = Index::build(base);
    let (now, _, _) = Index::build(document);
    let mut changes = NodeChanges::default();
    for id in in_order(document) {
        match before.get(&id) {
            None => changes.added.push(id),
            Some(view) if after.get(&id) != Some(view) => changes.changed.push(id),
            Some(_) if then.parent(id) != now.parent(id) => changes.moved.push(id),
            Some(_) => {}
        }
    }
    changes.removed = in_order(base)
        .into_iter()
        .filter(|id| !after.contains_key(id))
        .collect();
    changes
}

/// The keyed nodes of a document, parents before children.
fn in_order(document: &Document) -> Vec<NodeId> {
    let mut ids = Vec::new();
    document.walk(&mut |node| ids.extend(node.id()));
    ids
}
