//! Typed domain IDs. Each wraps the element's key and serialises as it, so the files, views and
//! digests see one lowercase UUID whatever the type; the types only keep the code from passing one
//! kind of element where another is meant. A kind change keeps the key (AC-249) through an
//! explicit conversion such as [`RequirementId::promote`].

use std::fmt;

use serde::Serialize;

use crate::identity::{Key, LabelPrefix};
use crate::kind::NodeKind;

macro_rules! domain_ids {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(Key);

        impl $name {
            #[must_use]
            pub const fn new(key: Key) -> Self {
                Self(key)
            }

            /// A fresh version 7 key.
            #[must_use]
            pub fn mint() -> Self {
                Self(Key::mint())
            }

            #[must_use]
            pub const fn key(self) -> Key {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, formatter)
            }
        }
    )*};
}

domain_ids! {
    /// The key of a whole document, named in its manifest.
    DocumentId,
    ProductId,
    FeatureGroupId,
    FeatureId,
    RequirementId,
    CriterionId,
    ComponentId,
    ConstraintId,
    QualityAttributeId,
    SectionId,
    DecisionId,
    OptionId,
    DriverId,
    ConsequenceId,
    /// The key of a tag registry entry.
    TagId,
    /// The key of a constraint category registry entry.
    CategoryId,
    /// The UUID of a Loft v1 Authored State a document was derived from.
    AuthoredStateId,
}

impl RequirementId {
    /// The same key as a Feature's, for a Requirement promoted to a Feature (RQ-63).
    #[must_use]
    pub const fn promote(self) -> FeatureId {
        FeatureId(self.0)
    }
}

/// The ID of a node of any kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeId {
    Product(ProductId),
    FeatureGroup(FeatureGroupId),
    Feature(FeatureId),
    Requirement(RequirementId),
    Criterion(CriterionId),
    Component(ComponentId),
    Constraint(ConstraintId),
    QualityAttribute(QualityAttributeId),
    Section(SectionId),
}

impl NodeId {
    #[must_use]
    pub const fn key(self) -> Key {
        match self {
            Self::Product(id) => id.key(),
            Self::FeatureGroup(id) => id.key(),
            Self::Feature(id) => id.key(),
            Self::Requirement(id) => id.key(),
            Self::Criterion(id) => id.key(),
            Self::Component(id) => id.key(),
            Self::Constraint(id) => id.key(),
            Self::QualityAttribute(id) => id.key(),
            Self::Section(id) => id.key(),
        }
    }

    /// The ID of a node of `kind` with `key`.
    #[must_use]
    pub const fn new(kind: NodeKind, key: Key) -> Self {
        match kind {
            NodeKind::Product => Self::Product(ProductId(key)),
            NodeKind::FeatureGroup => Self::FeatureGroup(FeatureGroupId(key)),
            NodeKind::Feature => Self::Feature(FeatureId(key)),
            NodeKind::Requirement => Self::Requirement(RequirementId(key)),
            NodeKind::Criterion => Self::Criterion(CriterionId(key)),
            NodeKind::Component => Self::Component(ComponentId(key)),
            NodeKind::Constraint => Self::Constraint(ConstraintId(key)),
            NodeKind::QualityAttribute => Self::QualityAttribute(QualityAttributeId(key)),
            NodeKind::Section => Self::Section(SectionId(key)),
        }
    }

    #[must_use]
    pub const fn kind(self) -> NodeKind {
        match self {
            Self::Product(_) => NodeKind::Product,
            Self::FeatureGroup(_) => NodeKind::FeatureGroup,
            Self::Feature(_) => NodeKind::Feature,
            Self::Requirement(_) => NodeKind::Requirement,
            Self::Criterion(_) => NodeKind::Criterion,
            Self::Component(_) => NodeKind::Component,
            Self::Constraint(_) => NodeKind::Constraint,
            Self::QualityAttribute(_) => NodeKind::QualityAttribute,
            Self::Section(_) => NodeKind::Section,
        }
    }
}

/// The ID of an element that edges and bindings name: a node or a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ElementId {
    Node(NodeId),
    Decision(DecisionId),
}

impl ElementId {
    #[must_use]
    pub const fn key(self) -> Key {
        match self {
            Self::Node(id) => id.key(),
            Self::Decision(id) => id.key(),
        }
    }

    /// The prefix of the labels this element receives at publish; a Section receives none.
    #[must_use]
    pub const fn label_prefix(self) -> Option<LabelPrefix> {
        match self {
            Self::Node(id) => id.kind().label_prefix(),
            Self::Decision(_) => Some(LabelPrefix::Decision),
        }
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} {}", self.kind(), self.key())
    }
}

impl fmt::Display for ElementId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node(id) => fmt::Display::fmt(id, formatter),
            Self::Decision(id) => write!(formatter, "decision {id}"),
        }
    }
}

macro_rules! into_ids {
    ($($id:ident => $variant:ident),* $(,)?) => {$(
        impl From<$id> for NodeId {
            fn from(id: $id) -> Self {
                Self::$variant(id)
            }
        }

        impl From<$id> for ElementId {
            fn from(id: $id) -> Self {
                Self::Node(NodeId::$variant(id))
            }
        }
    )*};
}

into_ids! {
    ProductId => Product,
    FeatureGroupId => FeatureGroup,
    FeatureId => Feature,
    RequirementId => Requirement,
    CriterionId => Criterion,
    ComponentId => Component,
    ConstraintId => Constraint,
    QualityAttributeId => QualityAttribute,
    SectionId => Section,
}

impl From<NodeId> for ElementId {
    fn from(id: NodeId) -> Self {
        Self::Node(id)
    }
}

impl From<DecisionId> for ElementId {
    fn from(id: DecisionId) -> Self {
        Self::Decision(id)
    }
}
