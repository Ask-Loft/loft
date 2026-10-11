//! Any node of the tree, borrowed, and its children in canonical order: the one place that says
//! which list of a node holds which kind of child.

use crate::ids::NodeId;
use crate::kind::NodeKind;
use crate::tree::{
    Component, Constraint, Criterion, Feature, FeatureGroup, Product, QualityAttribute,
    Requirement, Section,
};

/// A borrowed node of any kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node<'a> {
    Product(&'a Product),
    FeatureGroup(&'a FeatureGroup),
    Feature(&'a Feature),
    Requirement(&'a Requirement),
    Criterion(&'a Criterion),
    Component(&'a Component),
    Constraint(&'a Constraint),
    QualityAttribute(&'a QualityAttribute),
    Section(&'a Section),
}

impl<'a> Node<'a> {
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

    /// The node's typed ID; none while the node has no key (AC-209).
    #[must_use]
    pub fn id(self) -> Option<NodeId> {
        match self {
            Self::Product(node) => node.id.map(NodeId::from),
            Self::FeatureGroup(node) => node.id.map(NodeId::from),
            Self::Feature(node) => node.id.map(NodeId::from),
            Self::Requirement(node) => node.id.map(NodeId::from),
            Self::Criterion(node) => node.id.map(NodeId::from),
            Self::Component(node) => node.id.map(NodeId::from),
            Self::Constraint(node) => node.id.map(NodeId::from),
            Self::QualityAttribute(node) => node.id.map(NodeId::from),
            Self::Section(node) => node.id.map(NodeId::from),
        }
    }

    /// The Sections the node holds.
    #[must_use]
    pub fn sections(self) -> &'a [Section] {
        match self {
            Self::Product(node) => &node.details.sections,
            Self::FeatureGroup(node) => &node.details.sections,
            Self::Feature(node) => &node.details.sections,
            Self::Requirement(node) => &node.details.sections,
            Self::Criterion(node) => &node.details.sections,
            Self::Component(node) => &node.details.sections,
            Self::Constraint(node) => &node.details.sections,
            Self::QualityAttribute(node) => &node.details.sections,
            Self::Section(node) => &node.sections,
        }
    }

    /// The node's children in canonical order: each structural list in the order
    /// [`NodeKind::child_kinds`] names, then the Sections (AC-605..AC-609).
    #[must_use]
    pub fn children(self) -> Vec<Self> {
        let mut children = Vec::new();
        match self {
            Self::Product(node) => {
                children.extend(node.feature_groups.iter().map(Self::FeatureGroup));
            }
            Self::FeatureGroup(node) => {
                children.extend(node.features.iter().map(Self::Feature));
                children.extend(node.constraints.iter().map(Self::Constraint));
            }
            Self::Feature(node) => {
                children.extend(node.requirements.iter().map(Self::Requirement));
                children.extend(node.quality_attributes.iter().map(Self::QualityAttribute));
            }
            Self::Requirement(node) => {
                children.extend(node.criteria.iter().map(Self::Criterion));
            }
            Self::Component(node) => {
                children.extend(node.components.iter().map(Self::Component));
                children.extend(node.constraints.iter().map(Self::Constraint));
                children.extend(node.quality_attributes.iter().map(Self::QualityAttribute));
            }
            Self::Criterion(_)
            | Self::Constraint(_)
            | Self::QualityAttribute(_)
            | Self::Section(_) => {}
        }
        children.extend(self.sections().iter().map(Self::Section));
        children
    }

    /// Visits the node and every node beneath it, parents before children, in canonical order.
    pub fn walk(self, visit: &mut impl FnMut(Self)) {
        visit(self);
        for child in self.children() {
            child.walk(visit);
        }
    }
}

macro_rules! from_refs {
    ($($kind:ident),*) => {$(
        impl<'a> From<&'a $kind> for Node<'a> {
            fn from(node: &'a $kind) -> Self {
                Self::$kind(node)
            }
        }
    )*};
}

from_refs!(
    Product,
    FeatureGroup,
    Feature,
    Requirement,
    Criterion,
    Component,
    Constraint,
    QualityAttribute,
    Section
);
