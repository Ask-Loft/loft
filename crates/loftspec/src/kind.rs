//! The closed set of node kinds (CON-4).

use std::fmt;
use std::str::FromStr;

use crate::identity::LabelPrefix;

/// Every kind of node the Format defines. No other kind can be represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NodeKind {
    Product,
    FeatureGroup,
    Feature,
    Requirement,
    Criterion,
    Component,
    Constraint,
    QualityAttribute,
    Section,
}

/// A kind name outside the Format's closed set.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a node kind the Format defines")]
pub struct UnknownKind(pub String);

impl NodeKind {
    pub const ALL: [Self; 9] = [
        Self::Product,
        Self::FeatureGroup,
        Self::Feature,
        Self::Requirement,
        Self::Criterion,
        Self::Component,
        Self::Constraint,
        Self::QualityAttribute,
        Self::Section,
    ];

    /// The kind's name as the text form and the content view write it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::FeatureGroup => "feature_group",
            Self::Feature => "feature",
            Self::Requirement => "requirement",
            Self::Criterion => "criterion",
            Self::Component => "component",
            Self::Constraint => "constraint",
            Self::QualityAttribute => "quality_attribute",
            Self::Section => "section",
        }
    }

    /// The kinds a node of this kind holds as structural children, in the order its child
    /// content hashes list them (AC-606..AC-609). Sections, which any node may hold, follow them
    /// (AC-157, AC-605).
    #[must_use]
    pub const fn child_kinds(self) -> &'static [Self] {
        match self {
            Self::Product => &[Self::FeatureGroup],
            Self::FeatureGroup => &[Self::Feature, Self::Constraint],
            Self::Feature => &[Self::Requirement, Self::QualityAttribute],
            Self::Requirement => &[Self::Criterion],
            Self::Component => &[Self::Component, Self::Constraint, Self::QualityAttribute],
            Self::Criterion | Self::Constraint | Self::QualityAttribute | Self::Section => &[],
        }
    }

    /// Whether a node of this kind may hold a child of `child`'s kind (RQ-74).
    #[must_use]
    pub fn may_contain(self, child: Self) -> bool {
        child == Self::Section || self.child_kinds().contains(&child)
    }

    /// The prefix of the labels this kind receives at publish; a Section receives none.
    #[must_use]
    pub const fn label_prefix(self) -> Option<LabelPrefix> {
        match self {
            Self::Product => Some(LabelPrefix::Product),
            Self::FeatureGroup => Some(LabelPrefix::FeatureGroup),
            Self::Feature => Some(LabelPrefix::Feature),
            Self::Requirement => Some(LabelPrefix::Requirement),
            Self::Criterion => Some(LabelPrefix::Criterion),
            Self::Component => Some(LabelPrefix::Component),
            Self::Constraint => Some(LabelPrefix::Constraint),
            Self::QualityAttribute => Some(LabelPrefix::QualityAttribute),
            Self::Section => None,
        }
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for NodeKind {
    type Err = UnknownKind;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == text)
            .ok_or_else(|| UnknownKind(text.to_owned()))
    }
}
