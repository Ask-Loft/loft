//! The nodes one document added, changed and removed against another, its base (the `loft
//! status` Feature). A node is matched by its key, so a node that moves stays the same node; it
//! changed when its own content view differs, the hashes of its children left out, so a change
//! to a node is not a change to each of its ancestors, or when the edges its record writes
//! differ: a Feature's `realizes`, a Component's `scopedTo` and any node's `mentions`. A node
//! moved when its own content view is the base's and its parent is not (the `loft review`
//! Feature); a move alone is no change, so [`NodeChanges::is_empty`] ignores it. The comparer,
//! [`compare`], tells the same changes by each node's record in place of its content view, and
//! [`decision_changes`] tells the decisions added, changed and removed, with the subjects each
//! gained and lost.

mod compare;
mod decisions;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::document::{Document, EdgeKind, EdgeTarget};
use crate::hash::own_views;
use crate::identity::Key;
use crate::ids::{DocumentId, ElementId, InterfaceId, NodeId};
use crate::index::Index;
use crate::tree::Interface;

pub use compare::{
    Comparison, DecisionChange, DecisionDifference, Difference, NodeDifference, Order, Part,
    Refusal, Side, Version, Which, compare,
};
pub use decisions::{DecisionChanges, Subject, SubjectChange, decision_changes};

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
    changes(
        &Keyed::compared(base),
        &Keyed::compared(document),
        Some(parents),
    )
}

/// The nodes `document` added, changed and removed against `base`, as [`node_changes`] gives
/// them, without the moved, which takes both documents indexed to tell.
#[must_use]
pub fn content_changes(base: &Document, document: &Document) -> NodeChanges {
    changes(&Keyed::compared(base), &Keyed::compared(document), None)
}

/// The nodes the Interface `newer` added, changed and removed against `older`, as
/// [`content_changes`] tells them: what a move of a copy's pin did to the promise it copies. A
/// copy holds its Interface without edges, so only the nodes' own content is compared.
#[must_use]
pub fn interface_changes(older: &Interface, newer: &Interface) -> NodeChanges {
    let alone = |interface: &Interface| {
        let key = (interface.id)
            .map(InterfaceId::key)
            .expect("a copied Interface carries its key");
        let mut document = Document::empty(DocumentId::new(key));
        document.interfaces.push(interface.clone());
        document
    };
    content_changes(&alone(older), &alone(newer))
}

/// A document's keyed nodes: each one's own content view, and their order, parents before
/// children; and, as they are compared, the edges each one's record writes.
pub(crate) struct Keyed {
    pub views: BTreeMap<NodeId, Value>,
    pub order: Vec<NodeId>,
    /// The kind and target of each edge a node's record writes, where it writes one.
    pub written: BTreeMap<NodeId, BTreeSet<(EdgeKind, Key)>>,
}

impl Keyed {
    /// A document's keyed nodes as they are compared, with the edges each one's record writes:
    /// a Feature's `realizes`, a Component's `scopedTo` and any node's `mentions`. The content
    /// view leaves edges to the edges root, but the record writes these, so a change to them
    /// changes the node.
    pub(crate) fn compared(document: &Document) -> Self {
        Self {
            views: own_views(document),
            order: Self::order_of(document),
            written: (written(document).into_iter())
                .filter_map(|(from, edges)| match from {
                    ElementId::Node(node) => Some((node, edges)),
                    ElementId::Decision(_) => None,
                })
                .collect(),
        }
    }

    /// A document's keyed nodes in order, parents before children.
    pub fn order_of(document: &Document) -> Vec<NodeId> {
        let mut order = Vec::new();
        document.walk(&mut |node| order.extend(node.id()));
        order
    }
}

/// The kind and target of each edge an element's record writes, by the element: a Feature's
/// `realizes`, a Component's `scopedTo`, `provides` and `requires`, and any node's or decision's
/// `mentions`. A decision's
/// subjects are left out, since they are compared apart, each former label read as its node.
pub(crate) fn written(document: &Document) -> BTreeMap<ElementId, BTreeSet<(EdgeKind, Key)>> {
    let mut written: BTreeMap<ElementId, BTreeSet<(EdgeKind, Key)>> = BTreeMap::new();
    for edge in &document.edges {
        let EdgeTarget::Element(to) = edge.target() else {
            continue;
        };
        if edge.kind() != EdgeKind::Subject {
            let edges = written.entry(edge.source()).or_default();
            edges.insert((edge.kind(), to.key()));
        }
    }
    written
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
        Some(view)
            if after.views.get(&id) != Some(view)
                || before.written.get(&id) != after.written.get(&id) =>
        {
            Some(NodeChange::Changed)
        }
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
