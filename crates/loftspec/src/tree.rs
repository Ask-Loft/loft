//! The node tree, one struct per kind, so that containment is in the types: a Product holds
//! Feature Groups; a Feature Group holds Features and Constraints; a Feature holds Requirements
//! and Quality Attributes; a Requirement holds criteria; and a Component holds Components,
//! Constraints and Quality Attributes. Children of different kinds are kept in one list per
//! kind, the order the content view lists them in (AC-606..AC-609). Any node with a body may
//! hold Sections (AC-157). A node read without a key has none until the formatter mints one
//! (AC-209).

use crate::attributes::{
    ConservationKind, QaKind, QualityTarget, TechnicalBinding, VerificationFate,
};
use crate::ids::{
    ComponentId, ConstraintId, CriterionId, FeatureGroupId, FeatureId, ProductId,
    QualityAttributeId, RequirementId, SectionId,
};
use std::ops::{Deref, DerefMut};

use crate::names::TagName;
use crate::slug::Slug;
use crate::statement::Statement;

/// What every node kind but the Section carries besides its name and children: a body, the tags
/// applied to it, and its Sections.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Details {
    pub body: Option<String>,
    pub tags: Vec<TagName>,
    pub sections: Vec<Section>,
}

impl Details {
    #[must_use]
    pub fn with_body(body: &str) -> Self {
        Self {
            body: Some(body.to_owned()),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    pub id: Option<ProductId>,
    pub slug: Slug,
    pub name: String,
    pub details: Details,
    pub feature_groups: Vec<FeatureGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureGroup {
    pub id: Option<FeatureGroupId>,
    pub slug: Slug,
    pub name: String,
    pub details: Details,
    pub features: Vec<Feature>,
    pub constraints: Vec<Constraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Feature {
    pub id: Option<FeatureId>,
    pub slug: Slug,
    pub name: String,
    pub details: Details,
    pub requirements: Vec<Requirement>,
    pub quality_attributes: Vec<QualityAttribute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    pub id: Option<RequirementId>,
    pub statement: Statement,
    pub details: Details,
    pub criteria: Vec<Criterion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criterion {
    pub id: Option<CriterionId>,
    pub name: String,
    pub details: Details,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    pub id: Option<ComponentId>,
    pub name: String,
    pub details: Details,
    pub components: Vec<Component>,
    pub constraints: Vec<Constraint>,
    pub quality_attributes: Vec<QualityAttribute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub id: Option<ConstraintId>,
    pub name: String,
    pub details: Details,
    pub binding: Option<TechnicalBinding>,
    pub conservation_kind: Option<ConservationKind>,
    pub verification_fate: Option<VerificationFate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityAttribute {
    pub id: Option<QualityAttributeId>,
    pub name: String,
    pub details: Details,
    pub kind: QaKind,
    pub target: QualityTarget,
    pub verification_fate: Option<VerificationFate>,
}

/// A Component with no parent, written to its own file under its slug (RQ-23, AC-757).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootComponent {
    pub slug: Slug,
    pub component: Component,
}

impl Deref for RootComponent {
    type Target = Component;

    fn deref(&self) -> &Component {
        &self.component
    }
}

impl DerefMut for RootComponent {
    fn deref_mut(&mut self) -> &mut Component {
        &mut self.component
    }
}

/// Headed prose inside a node: a name, a body and ordered child Sections (AC-155, AC-156).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub id: Option<SectionId>,
    pub name: String,
    pub body: String,
    pub sections: Vec<Section>,
}
