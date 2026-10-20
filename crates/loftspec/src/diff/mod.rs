//! The nodes one document added, changed and removed against another, its base (the `loft
//! status` Feature). A node is matched by its key, so a node that moves stays the same node; it
//! changed when its own content view differs, the hashes of its children left out, so a
//! change to a node is not a change to each of its ancestors. A node moved when its own content
//! view is the base's and its parent is not (the `loft review` Feature); a move alone is no
//! change, so [`NodeChanges::is_empty`] ignores it. The comparer, [`compare`], tells the same
//! changes by each node's record in place of its content view.

mod compare;

use std::collections::BTreeMap;

use serde_json::Value;

use crate::document::Document;
use crate::hash::own_views;
use crate::ids::NodeId;
use crate::index::Index;

pub use compare::{
    Comparison, DecisionChange, DecisionDifference, Difference, NodeDifference, Order, Part,
    Refusal, Side, Version, Which, compare,
};

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
    let parents = (then.parents(), now.parents());
    changes(&Keyed::of(base), &Keyed::of(document), Some(parents))
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
        Self {
            views: own_views(document),
            order: Self::order_of(document),
        }
    }

    /// A document's keyed nodes in order, parents before children.
    pub fn order_of(document: &Document) -> Vec<NodeId> {
        let mut order = Vec::new();
        document.walk(&mut |node| order.extend(node.id()));
        order
    }
}

/// Each keyed node's parent, the nearest keyed node above it; none for a Product or a root
/// Component.
pub(crate) type Parents = BTreeMap<NodeId, NodeId>;

/// How a version holds a node it holds against another version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeChange {
    Added,
    Changed,
    /// Its own content is the other version's and its parent is not.
    Moved,
}

/// How `after` holds the node `id` it holds against `before`, telling a move only where both
/// sides' parents are given; none where it holds the node as `before` does.
pub(crate) fn change_of(
    before: &Keyed,
    after: &Keyed,
    parents: Option<(&Parents, &Parents)>,
    id: NodeId,
) -> Option<NodeChange> {
    let moved = parents.is_some_and(|(then, now)| then.get(&id) != now.get(&id));
    match before.views.get(&id) {
        None => Some(NodeChange::Added),
        Some(view) if after.views.get(&id) != Some(view) => Some(NodeChange::Changed),
        Some(_) if moved => Some(NodeChange::Moved),
        Some(_) => None,
    }
}

/// The nodes `after` added, changed and removed against `before`, and the moved where both
/// sides' parents are given.
pub(crate) fn changes(
    before: &Keyed,
    after: &Keyed,
    parents: Option<(&Parents, &Parents)>,
) -> NodeChanges {
    let mut changes = NodeChanges::default();
    for id in &after.order {
        let list = match change_of(before, after, parents, *id) {
            Some(NodeChange::Added) => &mut changes.added,
            Some(NodeChange::Changed) => &mut changes.changed,
            Some(NodeChange::Moved) => &mut changes.moved,
            None => continue,
        };
        list.push(*id);
    }
    changes.removed = before
        .order
        .iter()
        .filter(|id| !after.views.contains_key(id))
        .copied()
        .collect();
    changes
}
