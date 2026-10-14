//! The Loft v1 Authored State export, as the converter reads it. The model is strict: a field
//! the v1 export does not define is an error, so a drift in the v1 format surfaces when reading
//! and not as a wrong conversion. v1 identifiers are UUIDs until conversion gives them a kind.

use std::fmt;
use std::num::NonZeroU64;

use serde::{Deserialize, Deserializer, de};
use uuid::Uuid;

use crate::attributes::{
    ConservationKind, QaKind, QualityTarget, TechnicalBinding, VerificationFate,
};
use crate::canonical::Digest;
use crate::decision::{ConsequenceKind, Rating};
use crate::document::SubjectRole;
use crate::identity::{Label, LabelError, LabelPrefix};
use crate::names::CategoryName;

/// An export the reader refuses.
#[derive(Debug, thiserror::Error)]
#[error("not a Loft v1 Authored State export: {0}")]
pub struct ExportError(#[from] serde_json::Error);

/// A whole v1 Authored State.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Export {
    pub authored_state_id: Uuid,
    pub publish_sequence: u64,
    pub published_at: String,
    pub content_address: Digest,
    pub graph_address: Digest,
    pub vocabulary_address: Digest,
    #[serde(default)]
    pub technical_constraint_category_address: Option<Digest>,
    #[serde(default)]
    pub architecture_decision_address: Option<Digest>,
    pub nodes: Vec<Node>,
    pub realization_links: Vec<RealizationLink>,
    pub vocabulary_entries: Vec<VocabularyEntry>,
    #[serde(default)]
    pub technical_constraint_categories: Vec<Category>,
    #[serde(default)]
    pub architecture_decisions: Vec<DecisionAggregate>,
    pub annotations: Vec<serde_json::Value>,
}

impl Export {
    /// Reads an export from its JSON text.
    pub fn from_json(text: &str) -> Result<Self, ExportError> {
        Ok(serde_json::from_str(text)?)
    }
}

/// A v1 external ID: an upper-case prefix, a hyphen and a positive integer, such as `RQ-12`
/// or `FT-3`. Facet Tags carry one even though the Format gives them no label.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalId {
    prefix: String,
    number: NonZeroU64,
}

impl ExternalId {
    /// Reads `PREFIX-n`, refusing anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (prefix, digits) = text.split_once('-')?;
        let well_formed = !prefix.is_empty()
            && prefix.bytes().all(|byte| byte.is_ascii_uppercase())
            && !digits.starts_with('0')
            && digits.bytes().all(|byte| byte.is_ascii_digit());
        let number = digits.parse().ok().filter(|_| well_formed)?;
        Some(Self {
            prefix: prefix.to_owned(),
            number,
        })
    }

    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    #[must_use]
    pub const fn number(&self) -> NonZeroU64 {
        self.number
    }

    /// The Format label this external ID is, when the Format defines its prefix.
    pub fn label(&self) -> Result<Label, LabelError> {
        LabelPrefix::parse(&self.prefix)
            .map(|prefix| Label::new(prefix, self.number))
            .ok_or_else(|| LabelError::UnknownPrefix(self.to_string()))
    }
}

impl fmt::Display for ExternalId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}-{}", self.prefix, self.number)
    }
}

impl<'de> Deserialize<'de> for ExternalId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text)
            .ok_or_else(|| de::Error::custom(format!("`{text}` is not a v1 external ID")))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum NodeType {
    TierSection,
    AcceptanceCriterion,
    Component,
    Constraint,
    QualityAttribute,
    FacetTag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Tier {
    Product,
    FeatureGroup,
    Feature,
    Requirement,
}

/// What a v1 Component is scoped to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ComponentScope {
    Document,
    External,
    Product { product_id: Uuid },
}

/// One v1 node version.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Node {
    pub node_id: Uuid,
    pub external_id: ExternalId,
    #[serde(rename = "type")]
    pub node_type: NodeType,
    pub document_id: Uuid,
    pub name: String,
    pub body: Option<String>,
    pub tier: Option<Tier>,
    pub verification_fate: Option<VerificationFate>,
    pub conservation_kind: Option<ConservationKind>,
    pub qa_kind: Option<QaKind>,
    pub fate_confirmed: Option<bool>,
    pub proposed_fate: Option<VerificationFate>,
    #[serde(default)]
    pub technical_binding: Option<TechnicalBinding>,
    #[serde(default)]
    pub component_scope: Option<ComponentScope>,
    #[serde(default)]
    pub quality_target: Option<QualityTarget>,
    pub ordinal: i64,
    pub children_hashes: Vec<ChildHash>,
    pub node_hash: Digest,
}

/// A child a v1 node lists, by UUID and content hash, in ordinal order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChildHash {
    pub node_id: Uuid,
    pub node_hash: Digest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RealizationLink {
    pub feature_id: Uuid,
    pub component_id: Uuid,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VocabularyEntry {
    pub entry_hash: Digest,
    pub entry_id: Uuid,
    pub document_id: Uuid,
    /// Free text in v1; the converter derives the tag name (RQ-71).
    pub name: String,
    pub definition: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Category {
    pub category_entry_hash: Digest,
    pub id: Uuid,
    pub document_id: Uuid,
    pub name: CategoryName,
    pub definition: String,
}

/// A node an Architecture Decision names.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubjectRef {
    pub external_id: ExternalId,
    pub node_id: Uuid,
    pub tier: Option<Tier>,
    #[serde(rename = "type")]
    pub node_type: NodeType,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionIdentity {
    pub decision_id: Uuid,
    pub document_id: Uuid,
    pub external_id: ExternalId,
}

/// A v1 Architecture Decision with its elements, assessments and subject links.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionAggregate {
    pub identity: DecisionIdentity,
    pub decision_hash: Digest,
    pub elements: Vec<Element>,
    pub assessments: Vec<Assessment>,
    pub subject_links: Vec<SubjectLink>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Element {
    pub id: Uuid,
    pub document_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub ordinal: i64,
    pub element_hash: Digest,
    pub payload: ElementPayload,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum ElementPayload {
    #[serde(rename_all = "camelCase")]
    ArchitectureDecision {
        title: String,
        context: String,
        state: DecisionState,
    },
    #[serde(rename = "option")]
    DecisionOption {
        title: String,
        description: String,
    },
    Driver(DriverPayload),
    Consequence {
        kind: ConsequenceKind,
        statement: String,
        subject: Option<SubjectRef>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DecisionState {
    Proposed,
    Accepted {
        selected_option_id: Uuid,
        outcome: String,
    },
    Rejected {
        rationale: String,
    },
    Superseded {
        selected_option_id: Uuid,
        outcome: String,
        successor_decision_id: Uuid,
    },
    Retired {
        selected_option_id: Uuid,
        outcome: String,
        rationale: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DriverPayload {
    Local { label: String, statement: String },
    SpecificationSubject { subject: SubjectRef },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Assessment {
    pub decision_id: Uuid,
    pub option_id: Uuid,
    pub driver_id: Uuid,
    pub rating: Rating,
    pub rationale: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubjectLink {
    pub decision_id: Uuid,
    pub role: SubjectRole,
    pub subject: SubjectRef,
}
