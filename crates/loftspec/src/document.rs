//! A whole document (CMP-2): the Products in the manifest's order, the root Components, the
//! typed edges, the two registries, the decisions, the bindings and the record of a v1 source.

use crate::bindings::{Bindings, Counters};
use crate::canonical::Digest;
use crate::decision::Decision;
use crate::ids::{
    AuthoredStateId, CategoryId, ComponentId, DecisionId, DocumentId, ElementId, FeatureId, NodeId,
    ProductId, TagId,
};
use crate::names::{CategoryName, TagName};
use crate::tree::{Component, Product};

/// The type of an edge (D-11 F-13, F-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeKind {
    Realizes,
    ScopedTo,
    Mentions,
    Subject,
}

impl EdgeKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Realizes => "realizes",
            Self::ScopedTo => "scoped_to",
            Self::Mentions => "mentions",
            Self::Subject => "subject",
        }
    }
}

/// The role a `subject` edge carries from a decision to a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubjectRole {
    Affected,
    Outcome,
}

impl SubjectRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Affected => "affected",
            Self::Outcome => "outcome",
        }
    }
}

/// A typed edge. Each type fixes the kinds at its two ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// A Feature realizes a Component (RQ-49).
    Realizes {
        feature: FeatureId,
        component: ComponentId,
    },
    /// A Component is scoped to a Product (RQ-50).
    ScopedTo {
        component: ComponentId,
        product: ProductId,
    },
    /// Prose of one element refers to another by label (RQ-53).
    Mentions {
        source: ElementId,
        target: ElementId,
    },
    /// A decision names a node as its outcome or as affected (AC-184).
    Subject {
        decision: DecisionId,
        subject: NodeId,
        role: SubjectRole,
    },
}

impl Edge {
    #[must_use]
    pub const fn kind(self) -> EdgeKind {
        match self {
            Self::Realizes { .. } => EdgeKind::Realizes,
            Self::ScopedTo { .. } => EdgeKind::ScopedTo,
            Self::Mentions { .. } => EdgeKind::Mentions,
            Self::Subject { .. } => EdgeKind::Subject,
        }
    }

    #[must_use]
    pub fn source(self) -> ElementId {
        match self {
            Self::Realizes { feature, .. } => feature.into(),
            Self::ScopedTo { component, .. } => component.into(),
            Self::Mentions { source, .. } => source,
            Self::Subject { decision, .. } => decision.into(),
        }
    }

    #[must_use]
    pub fn target(self) -> ElementId {
        match self {
            Self::Realizes { component, .. } => component.into(),
            Self::ScopedTo { product, .. } => product.into(),
            Self::Mentions { target, .. } => target,
            Self::Subject { subject, .. } => subject.into(),
        }
    }

    #[must_use]
    pub const fn role(self) -> Option<SubjectRole> {
        match self {
            Self::Subject { role, .. } => Some(role),
            _ => None,
        }
    }
}

/// An entry of the tag registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagEntry {
    pub id: TagId,
    pub name: TagName,
    pub definition: String,
}

/// An entry of the constraint category registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryEntry {
    pub id: CategoryId,
    pub name: CategoryName,
    pub definition: String,
}

/// The v1 Authored State a converted document was derived from: carried in the manifest and
/// copied into every lock (RQ-17, RQ-20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedFrom {
    pub authored_state_id: AuthoredStateId,
    pub content_address: Digest,
    pub publish_sequence: u64,
}

/// A whole document. Products are in the manifest's order (AC-241); root Components, edges,
/// registry entries and decisions are in no meaningful order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub id: DocumentId,
    pub products: Vec<Product>,
    pub components: Vec<Component>,
    pub edges: Vec<Edge>,
    pub tags: Vec<TagEntry>,
    pub categories: Vec<CategoryEntry>,
    pub decisions: Vec<Decision>,
    pub bindings: Bindings,
    pub counters: Counters,
    pub derived_from: Option<DerivedFrom>,
}

impl Document {
    /// An empty document with the given document key.
    #[must_use]
    pub fn empty(id: DocumentId) -> Self {
        Self {
            id,
            products: Vec::new(),
            components: Vec::new(),
            edges: Vec::new(),
            tags: Vec::new(),
            categories: Vec::new(),
            decisions: Vec::new(),
            bindings: Bindings::default(),
            counters: Counters::default(),
            derived_from: None,
        }
    }
}
