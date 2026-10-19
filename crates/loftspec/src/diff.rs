//! The nodes one document added, changed and removed against another, its base (the `loft
//! status` Feature). A node is matched by its key, so a node that moves stays the same node; it
//! changed when its own content view differs, the hashes of its children left out, so a
//! change to a node is not a change to each of its ancestors. A node moved when its own content
//! view is the base's and its parent is not (the `loft review` Feature); a move alone is no
//! change, so [`NodeChanges::is_empty`] ignores it.

use std::collections::BTreeMap;

use serde_json::Value;

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

/// The nodes `document` added, changed, moved and removed against `base`.
#[must_use]
pub fn node_changes(base: &Document, document: &Document) -> NodeChanges {
    let (then, _, _) = Index::build(base);
    let (now, _, _) = Index::build(document);
    changes(&Keyed::of(base), &Keyed::of(document), Some((&then, &now)))
}

/// The nodes `document` added, changed and removed against `base`, as [`node_changes`] gives
/// them, without the moved, which takes both documents indexed to tell.
#[must_use]
pub fn content_changes(base: &Document, document: &Document) -> NodeChanges {
    changes(&Keyed::of(base), &Keyed::of(document), None)
}

/// A document's keyed nodes: each one's own content view, and their order, parents before
/// children.
pub(crate) struct Keyed {
    pub views: BTreeMap<NodeId, Value>,
    pub order: Vec<NodeId>,
}

impl Keyed {
    pub fn of(document: &Document) -> Self {
        let mut order = Vec::new();
        document.walk(&mut |node| order.extend(node.id()));
        Self {
            views: own_views(document),
            order,
        }
    }
}

/// The nodes `after` added, changed and removed against `before`, and the moved where both
/// sides' indexes, `parents`, are given.
pub(crate) fn changes(
    before: &Keyed,
    after: &Keyed,
    parents: Option<(&Index, &Index)>,
) -> NodeChanges {
    let moved = |id: NodeId| parents.is_some_and(|(then, now)| then.parent(id) != now.parent(id));
    let mut changes = NodeChanges::default();
    for id in &after.order {
        match before.views.get(id) {
            None => changes.added.push(*id),
            Some(view) if after.views.get(id) != Some(view) => changes.changed.push(*id),
            Some(_) if moved(*id) => changes.moved.push(*id),
            Some(_) => {}
        }
    }
    changes.removed = before
        .order
        .iter()
        .filter(|id| !after.views.contains_key(id))
        .copied()
        .collect();
    changes
}
