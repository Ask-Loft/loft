//! The v1 sidecar (D-11 F-15): every v1 fact the content view excludes that reproducing the
//! source state needs (RQ-55, RQ-57), written deterministically (RQ-58) to `v1-sidecar.json`
//! beside `spec/` (AC-703).

use std::collections::BTreeMap;
use std::num::NonZeroU64;

use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::export::{ComponentScope, Export, ExternalId, Node};
use crate::attributes::VerificationFate;
use crate::identity::{Key, Label};
use crate::ids::{ComponentId, ConsequenceId, DecisionId, DocumentId, DriverId, NodeId, OptionId};
use crate::names::TagName;

/// The sidecar's format identifier, written as its `format` field.
pub const FORMAT: &str = "loftspec-v1-sidecar/1";

/// What a conversion keeps beside the document for the way back to v1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V1Sidecar {
    /// The v1 document UUID, which is also the document key (AC-197).
    pub document: DocumentId,
    /// The highest value seen per v1 external ID prefix, Facet Tags' `FT` included (AC-198).
    pub highest: BTreeMap<String, NonZeroU64>,
    /// The v1 facts of every node but the Facet Tags (AC-194, AC-195, AC-258).
    pub nodes: BTreeMap<NodeId, NodeFacts>,
    /// The scope kind of each Component that has one (AC-259).
    pub scopes: BTreeMap<ComponentId, ScopeKind>,
    /// The Facet Tag nodes behind each node's tags, in ordinal order (AC-196).
    pub tags: BTreeMap<NodeId, Vec<TagApplication>>,
    /// The ordinals of each decision's elements.
    pub decisions: BTreeMap<DecisionId, DecisionFacts>,
    /// The v1 node UUID behind each former label a decision names (ADR-13).
    pub former_labels: BTreeMap<Label, Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeFacts {
    pub ordinal: i64,
    pub fate_confirmed: Option<bool>,
    pub proposed_fate: Option<VerificationFate>,
}

/// What a v1 Component is scoped to; a Product scope's Product is the `scoped_to` edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Document,
    External,
    Product,
}

impl ScopeKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::External => "external",
            Self::Product => "product",
        }
    }
}

/// The v1 Facet Tag node a tag on a node came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagApplication {
    pub tag: TagName,
    pub node_id: Uuid,
    pub external_id: ExternalId,
    pub ordinal: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DecisionFacts {
    pub ordinal: i64,
    pub options: BTreeMap<OptionId, i64>,
    pub drivers: BTreeMap<DriverId, i64>,
    pub consequences: BTreeMap<ConsequenceId, i64>,
}

impl NodeFacts {
    pub(super) const fn of(node: &Node) -> Self {
        Self {
            ordinal: node.ordinal,
            fate_confirmed: node.fate_confirmed,
            proposed_fate: node.proposed_fate,
        }
    }
}

impl From<ComponentScope> for ScopeKind {
    fn from(scope: ComponentScope) -> Self {
        match scope {
            ComponentScope::Document => Self::Document,
            ComponentScope::External => Self::External,
            ComponentScope::Product { .. } => Self::Product,
        }
    }
}

impl V1Sidecar {
    /// An empty sidecar for the document with this v1 UUID; the conversion's passes fill it.
    pub(super) const fn new(document: DocumentId) -> Self {
        Self {
            document,
            highest: BTreeMap::new(),
            nodes: BTreeMap::new(),
            scopes: BTreeMap::new(),
            tags: BTreeMap::new(),
            decisions: BTreeMap::new(),
            former_labels: BTreeMap::new(),
        }
    }

    /// Records the highest value of each prefix among the export's nodes and decisions and the
    /// former labels already recorded.
    pub(super) fn record_highest(&mut self, export: &Export) {
        let nodes = export.nodes.iter().map(|node| &node.external_id);
        let decisions = export
            .architecture_decisions
            .iter()
            .map(|decision| &decision.identity.external_id);
        let seen = nodes
            .chain(decisions)
            .map(|external_id| (external_id.prefix(), external_id.number()));
        let former = self
            .former_labels
            .keys()
            .map(|label| (label.prefix().as_str(), label.number()));
        for (prefix, number) in seen.chain(former) {
            let value = self.highest.entry(prefix.to_owned()).or_insert(number);
            *value = (*value).max(number);
        }
    }

    /// The sidecar as JSON: objects keyed by key or prefix in ascending order, two-space indent,
    /// one trailing newline. Equal sidecars give equal bytes (AC-206).
    #[must_use]
    pub fn to_json(&self) -> String {
        let value = json!({
            "formerLabels": object(self.former_labels.iter().map(|(label, node_id)| {
                (label.to_string(), json!(node_id))
            })),
            "decisions": object(self.decisions.iter().map(|(id, facts)| {
                (id.key().to_string(), decision_json(facts))
            })),
            "document": self.document.key().to_string(),
            "format": FORMAT,
            "highest": object(self.highest.iter().map(|(prefix, value)| (prefix.clone(), json!(value.get())))),
            "nodes": object(self.nodes.iter().map(|(id, facts)| {
                (id.key().to_string(), json!({
                    "fateConfirmed": facts.fate_confirmed,
                    "ordinal": facts.ordinal,
                    "proposedFate": facts.proposed_fate,
                }))
            })),
            "scopes": object(self.scopes.iter().map(|(id, scope)| (id.key().to_string(), json!(scope.as_str())))),
            "tags": object(self.tags.iter().map(|(id, applications)| {
                (id.key().to_string(), applications.iter().map(|application| json!({
                    "externalId": application.external_id.to_string(),
                    "nodeId": application.node_id,
                    "ordinal": application.ordinal,
                    "tag": application.tag.as_str(),
                })).collect())
            })),
        });
        let mut text = serde_json::to_string_pretty(&value).unwrap_or_default();
        text.push('\n');
        text
    }
}

fn decision_json(facts: &DecisionFacts) -> Value {
    let ordinals = |entries: Vec<(Key, i64)>| {
        object(
            entries
                .into_iter()
                .map(|(key, ordinal)| (key.to_string(), json!(ordinal))),
        )
    };
    json!({
        "consequences": ordinals(facts.consequences.iter().map(|(id, ordinal)| (id.key(), *ordinal)).collect()),
        "drivers": ordinals(facts.drivers.iter().map(|(id, ordinal)| (id.key(), *ordinal)).collect()),
        "options": ordinals(facts.options.iter().map(|(id, ordinal)| (id.key(), *ordinal)).collect()),
        "ordinal": facts.ordinal,
    })
}

/// A JSON object with its members in ascending key order, whatever order they arrive in.
fn object(members: impl Iterator<Item = (String, Value)>) -> Value {
    let sorted: BTreeMap<String, Value> = members.collect();
    Value::Object(sorted.into_iter().collect::<Map<String, Value>>())
}
