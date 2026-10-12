//! What every rule reads: each keyed node with its parent, in document order, and the edges and
//! registry entries the structural rules count.

use std::collections::{BTreeMap, BTreeSet};

use crate::document::{Document, Edge};
use crate::ids::{ComponentId, FeatureId, NodeId};
use crate::names::TagName;
use crate::node::Node;

/// A keyed node and the node that holds it.
#[derive(Clone, Copy)]
pub(super) struct Item<'a> {
    pub node: Node<'a>,
    pub id: NodeId,
    pub parent: Option<NodeId>,
}

impl<'a> Item<'a> {
    /// The node's name; a Requirement's is its statement as rendered.
    pub(super) fn name(&self) -> std::borrow::Cow<'a, str> {
        match self.node {
            Node::Product(node) => (&node.name).into(),
            Node::FeatureGroup(node) => (&node.name).into(),
            Node::Feature(node) => (&node.name).into(),
            Node::Requirement(node) => node.statement.render().into(),
            Node::Criterion(node) => (&node.name).into(),
            Node::Component(node) => (&node.name).into(),
            Node::Constraint(node) => (&node.name).into(),
            Node::QualityAttribute(node) => (&node.name).into(),
            Node::Section(node) => (&node.name).into(),
        }
    }

    /// The node's body, if it has one.
    pub(super) fn body(&self) -> Option<&'a str> {
        match self.node {
            Node::Section(section) => Some(&section.body),
            node => node.details()?.body.as_deref(),
        }
    }

    /// The tags the node lists.
    pub(super) fn tags(&self) -> impl Iterator<Item = &'a str> {
        self.node
            .details()
            .into_iter()
            .flat_map(|details| details.tags.iter().map(TagName::as_str))
    }
}

pub(super) struct Context<'a> {
    pub document: &'a Document,
    /// Every keyed node, parents before children, in document order.
    pub items: Vec<Item<'a>>,
    by_id: BTreeMap<NodeId, usize>,
    pub realizing: BTreeSet<FeatureId>,
    pub realized: BTreeSet<ComponentId>,
}

impl<'a> Context<'a> {
    pub(super) fn new(document: &'a Document) -> Self {
        let mut items = Vec::new();
        for root in document.root_nodes() {
            collect(root, None, &mut items);
        }
        let by_id = items
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id, index))
            .collect();
        let mut realizing = BTreeSet::new();
        let mut realized = BTreeSet::new();
        for edge in &document.edges {
            if let Edge::Realizes { feature, component } = *edge {
                realizing.insert(feature);
                realized.insert(component);
            }
        }
        Self {
            document,
            items,
            by_id,
            realizing,
            realized,
        }
    }

    pub(super) fn item(&self, id: NodeId) -> Option<&Item<'a>> {
        self.by_id.get(&id).map(|index| &self.items[*index])
    }

    /// The node and every node above it, nearest first.
    pub(super) fn lineage(&self, id: NodeId) -> impl Iterator<Item = &Item<'a>> {
        std::iter::successors(self.item(id), |item| self.item(item.parent?))
    }
}

/// Collects a node and everything beneath it; an unkeyed node is skipped and its children are
/// held by its nearest keyed ancestor.
fn collect<'a>(node: Node<'a>, parent: Option<NodeId>, items: &mut Vec<Item<'a>>) {
    let id = node.id();
    if let Some(id) = id {
        items.push(Item { node, id, parent });
    }
    for child in node.children() {
        collect(child, id.or(parent), items);
    }
}
