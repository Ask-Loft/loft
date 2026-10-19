//! The two sides a review reads: the tree, and the base it is compared with. Each side is read
//! once: its index, its nodes by key with their own content views and their order, each node's
//! change, the links of its decisions and the Components each Feature realizes; the holders a
//! unit gives are made once each and then reused.

use std::cell::RefCell;
use std::collections::BTreeMap;

use serde_json::Value;

use crate::diff::{Keyed, changes};
use crate::document::{Document, Edge};
use crate::ids::NodeId;
use crate::index::Index;
use crate::kind::NodeKind;
use crate::node::Node;

use super::links::{self, DecisionLinks, Phrases};
use super::{Assertion, Change, Fact, Holder};

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
    keyed: Keyed,
    nodes: BTreeMap<NodeId, Node<'a>>,
    /// Each keyed node's place in [`Side::order`].
    ordinals: BTreeMap<NodeId, usize>,
    changes: BTreeMap<NodeId, Change>,
    pub links: DecisionLinks,
    /// The Components each Feature realizes, in the order of the edges.
    realized: BTreeMap<NodeId, Vec<NodeId>>,
    holders: RefCell<BTreeMap<NodeId, Holder>>,
}

impl<'a> Side<'a> {
    fn of(document: &'a Document, which: Which) -> Self {
        let mut nodes = BTreeMap::new();
        document.walk(&mut |node| {
            if let Some(id) = node.id() {
                nodes.insert(id, node);
            }
        });
        let keyed = Keyed::of(document);
        let ordinals = keyed
            .order
            .iter()
            .enumerate()
            .map(|(at, id)| (*id, at))
            .collect();
        let mut realized: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
        for edge in &document.edges {
            if let Edge::Realizes { feature, component } = edge {
                realized
                    .entry(NodeId::from(*feature))
                    .or_default()
                    .push(NodeId::from(*component));
            }
        }
        Self {
            document,
            which,
            index: Index::build(document).0,
            keyed,
            nodes,
            ordinals,
            changes: BTreeMap::new(),
            links: links::decision_links(document),
            realized,
            holders: RefCell::new(BTreeMap::new()),
        }
    }

    /// The tree, and the base where one is given, with each node's change.
    pub fn pair(document: &'a Document, base: Option<&'a Document>) -> Pair<'a> {
        let mut tree = Self::of(document, Which::Tree);
        let base = base.map(|base| {
            let mut side = Self::of(base, Which::Base);
            let changes = changes(&side.keyed, &tree.keyed, Some((&side.index, &tree.index)));
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
        Pair {
            phrases: links::phrases(&document.terms),
            tree,
            base,
        }
    }

    /// The keyed nodes, parents before children.
    pub fn order(&self) -> &[NodeId] {
        &self.keyed.order
    }

    /// The place of the node `id`, which this side holds, among its keyed nodes.
    pub fn ordinal(&self, id: NodeId) -> usize {
        self.ordinals[&id]
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

    /// The Components the Feature `feature` realizes that this side holds, in the order of the
    /// edges.
    pub fn realized(&self, feature: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let components = self.realized.get(&feature).map_or(&[][..], Vec::as_slice);
        components.iter().copied().filter(|id| self.holds(*id))
    }

    /// The node `id` as the packet gives it.
    pub fn fact(&self, id: NodeId) -> Fact {
        let node = self.node(id);
        Fact {
            id,
            label: self.document.bindings.label_of(id),
            name: node.name().into_owned(),
            view: self.keyed.views.get(&id).cloned().unwrap_or(Value::Null),
            change: self.change(id),
            from_base: self.which == Which::Base,
        }
    }

    /// The node `id` with the Constraints and Quality Attributes it holds, made once.
    pub fn holder(&self, id: NodeId) -> Holder {
        if let Some(holder) = self.holders.borrow().get(&id) {
            return holder.clone();
        }
        let children = self.node(id).children();
        let of = |kind: NodeKind| -> Vec<Fact> {
            children
                .iter()
                .filter(|child| child.kind() == kind)
                .filter_map(|child| child.id())
                .map(|child| self.fact(child))
                .collect()
        };
        let holder = Holder {
            node: self.fact(id),
            constraints: of(NodeKind::Constraint),
            quality_attributes: of(NodeKind::QualityAttribute),
        };
        self.holders.borrow_mut().insert(id, holder.clone());
        holder
    }
}

/// The tree and the base of a review, with the phrases of the tree's terms.
pub(super) struct Pair<'a> {
    pub tree: Side<'a>,
    pub base: Option<Side<'a>>,
    pub phrases: Phrases<'a>,
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
        Assertion { id, change, before }
    }
}
