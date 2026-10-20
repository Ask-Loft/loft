//! A node held outside a document, with every node beneath it and the edges they write: what a
//! detach returns and an insert takes.

use std::collections::BTreeSet;

use crate::document::Edge;
use crate::ids::{ElementId, NodeId};
use crate::kind::NodeKind;
use crate::node::Node;
use crate::tree::{
    Component, Constraint, Criterion, Feature, FeatureGroup, Product, QualityAttribute,
    Requirement, RootComponent, Section,
};

/// A node of any kind, owned. A Component detached from the root keeps its slug as a
/// [`RootComponent`]; one detached from under a Component has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnedNode {
    Product(Product),
    FeatureGroup(FeatureGroup),
    Feature(Feature),
    Requirement(Requirement),
    Criterion(Criterion),
    RootComponent(RootComponent),
    Component(Component),
    Constraint(Constraint),
    QualityAttribute(QualityAttribute),
    Section(Section),
}

impl OwnedNode {
    /// The node, borrowed.
    #[must_use]
    pub fn node(&self) -> Node<'_> {
        match self {
            Self::Product(node) => Node::Product(node),
            Self::FeatureGroup(node) => Node::FeatureGroup(node),
            Self::Feature(node) => Node::Feature(node),
            Self::Requirement(node) => Node::Requirement(node),
            Self::Criterion(node) => Node::Criterion(node),
            Self::RootComponent(node) => Node::Component(&node.component),
            Self::Component(node) => Node::Component(node),
            Self::Constraint(node) => Node::Constraint(node),
            Self::QualityAttribute(node) => Node::QualityAttribute(node),
            Self::Section(node) => Node::Section(node),
        }
    }
}

macro_rules! owned_from {
    ($($kind:ident),*) => {$(
        impl From<$kind> for OwnedNode {
            fn from(node: $kind) -> Self {
                Self::$kind(node)
            }
        }
    )*};
}

owned_from!(
    Product,
    FeatureGroup,
    Feature,
    Requirement,
    Criterion,
    RootComponent,
    Component,
    Constraint,
    QualityAttribute,
    Section
);

/// A node with every node beneath it, and the edges whose source is one of them, held outside
/// any document. Detaching a node returns one, and inserting one puts all of it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subtree(Box<Parts>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Parts {
    node: OwnedNode,
    edges: Vec<Edge>,
}

impl Subtree {
    /// A subtree of `node` and the nodes beneath it, writing no edge.
    #[must_use]
    pub fn new(node: impl Into<OwnedNode>) -> Self {
        Self::with_edges(node.into(), Vec::new())
    }

    pub(super) fn with_edges(node: OwnedNode, edges: Vec<Edge>) -> Self {
        Self(Box::new(Parts { node, edges }))
    }

    /// The subtree's top node, borrowed.
    #[must_use]
    pub fn node(&self) -> Node<'_> {
        self.0.node.node()
    }

    /// The subtree's top node, owned by the subtree.
    #[must_use]
    pub fn owned(&self) -> &OwnedNode {
        &self.0.node
    }

    /// The edges whose source is a node of the subtree.
    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.0.edges
    }

    #[must_use]
    pub fn kind(&self) -> NodeKind {
        self.node().kind()
    }

    /// The top node's ID; none while it has no key.
    #[must_use]
    pub fn id(&self) -> Option<NodeId> {
        self.node().id()
    }

    /// The top node and the edges, taken apart.
    #[must_use]
    pub fn into_parts(self) -> (OwnedNode, Vec<Edge>) {
        let Parts { node, edges } = *self.0;
        (node, edges)
    }

    /// The ID of every keyed node of the subtree, parents before children.
    #[must_use]
    pub fn ids(&self) -> Vec<NodeId> {
        ids_beneath(self.node())
    }
}

impl<T: Into<OwnedNode>> From<T> for Subtree {
    fn from(node: T) -> Self {
        Self::new(node)
    }
}

/// The ID of `node` and of every keyed node beneath it, parents before children.
pub(super) fn ids_beneath(node: Node<'_>) -> Vec<NodeId> {
    let mut ids = Vec::new();
    node.walk(&mut |each| ids.extend(each.id()));
    ids
}

/// Whether `edge`'s source is one of `ids`.
pub(super) fn sourced_in(edge: &Edge, ids: &BTreeSet<NodeId>) -> bool {
    match edge.source() {
        ElementId::Node(source) => ids.contains(&source),
        ElementId::Decision(_) => false,
    }
}
