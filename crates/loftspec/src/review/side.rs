//! The two sides a review reads: the tree, and the base it is compared with, each with its
//! index, its nodes by key, its order and each node's change.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::diff::node_changes;
use crate::document::Document;
use crate::hash::own_views;
use crate::ids::NodeId;
use crate::index::Index;
use crate::node::Node;

use super::{Assertion, Change, Fact};

/// Which side holds a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Which {
    Tree,
    Base,
}

/// One side of a review.
pub(super) struct Side<'a> {
    pub document: &'a Document,
    which: Which,
    index: Index,
    views: BTreeMap<NodeId, Value>,
    nodes: BTreeMap<NodeId, Node<'a>>,
    /// The keyed nodes, parents before children.
    pub order: Vec<NodeId>,
    changes: BTreeMap<NodeId, Change>,
}

impl<'a> Side<'a> {
    fn of(document: &'a Document, which: Which) -> Self {
        let mut nodes = BTreeMap::new();
        let mut order = Vec::new();
        document.walk(&mut |node| {
            if let Some(id) = node.id() {
                nodes.insert(id, node);
                order.push(id);
            }
        });
        Self {
            document,
            which,
            index: Index::build(document).0,
            views: own_views(document),
            nodes,
            order,
            changes: BTreeMap::new(),
        }
    }

    /// The tree, and the base where one is given, with each node's change.
    pub fn pair(document: &'a Document, base: Option<&'a Document>) -> Pair<'a> {
        let mut tree = Self::of(document, Which::Tree);
        let base = base.map(|base| {
            let mut side = Self::of(base, Which::Base);
            let changes = node_changes(base, document);
            for (ids, change) in [
                (&changes.added, Change::Added),
                (&changes.changed, Change::Changed),
                (&changes.moved, Change::Moved),
            ] {
                tree.changes.extend(ids.iter().map(|id| (*id, change)));
            }
            side.changes
                .extend(changes.removed.iter().map(|id| (*id, Change::Removed)));
            side
        });
        Pair { tree, base }
    }

    pub fn holds(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }

    /// The node `id`, which this side holds.
    pub fn node(&self, id: NodeId) -> Node<'a> {
        self.nodes[&id]
    }

    pub fn change(&self, id: NodeId) -> Option<Change> {
        self.changes.get(&id).copied()
    }

    /// The nodes above `id`, nearest first.
    pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
        self.index.ancestors(id).collect()
    }

    /// Whether `id` is `node` or lies below it.
    pub fn within(&self, id: NodeId, node: NodeId) -> bool {
        id == node || self.index.ancestors(id).any(|above| above == node)
    }

    /// The node `id` as the packet gives it.
    pub fn fact(&self, id: NodeId) -> Fact {
        let node = self.node(id);
        Fact {
            id,
            kind: node.kind(),
            label: self.document.bindings.label_of(id),
            name: node.name().into_owned(),
            view: self.views.get(&id).cloned().unwrap_or(Value::Null),
            change: self.change(id),
            from_base: self.which == Which::Base,
        }
    }
}

/// The tree and the base of a review.
pub(super) struct Pair<'a> {
    pub tree: Side<'a>,
    pub base: Option<Side<'a>>,
}

impl<'a> Pair<'a> {
    pub fn side(&self, which: Which) -> &Side<'a> {
        match (which, &self.base) {
            (Which::Base, Some(base)) => base,
            _ => &self.tree,
        }
    }

    /// The node `id`, from the tree where it holds it and from the base otherwise.
    pub fn node_anywhere(&self, id: NodeId) -> Node<'a> {
        match &self.base {
            Some(base) if !self.tree.holds(id) => base.node(id),
            _ => self.tree.node(id),
        }
    }

    /// The assertion `id` of the side `which`, with the base's node where it differs from it.
    pub fn assertion(&self, id: NodeId, which: Which) -> Assertion {
        let change = self.side(which).change(id);
        let before = match change {
            Some(Change::Changed | Change::Moved | Change::Removed) => {
                self.base.as_ref().map(|base| base.fact(id))
            }
            Some(Change::Added) | None => None,
        };
        Assertion {
            id,
            kind: id.kind(),
            change,
            before,
        }
    }
}
