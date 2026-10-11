//! The v1 sidecar (D-11 F-15): every v1 fact the content view excludes that reproducing the
//! source state needs (RQ-55, RQ-57), written deterministically (RQ-58) to `v1-sidecar.json`
//! beside `spec/` (AC-703).

use std::collections::BTreeMap;
use std::num::NonZeroU64;

use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::export::{ComponentScope, Export, ExternalId, NodeType, SubjectRef};
use super::links::NodeIds;
use super::tree::{kind_of, typed_id};
use crate::attributes::VerificationFate;
use crate::identity::Key;
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
    /// The external ID of each node a decision names that the export does not hold.
    pub absent_subjects: BTreeMap<NodeId, ExternalId>,
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

impl V1Sidecar {
    /// Collects the sidecar of a converted export. `nodes` holds the typed ID of every node the
    /// export holds but its Facet Tags.
    pub(super) fn collect(export: &Export, document: DocumentId, nodes: &NodeIds) -> Self {
        let mut sidecar = Self {
            document,
            highest: BTreeMap::new(),
            nodes: BTreeMap::new(),
            scopes: BTreeMap::new(),
            tags: BTreeMap::new(),
            decisions: BTreeMap::new(),
            absent_subjects: BTreeMap::new(),
        };
        let by_uuid: BTreeMap<Uuid, _> = export
            .nodes
            .iter()
            .map(|node| (node.node_id, node))
            .collect();
        for node in &export.nodes {
            sidecar.see(&node.external_id);
            let Some(id) = nodes.get(&node.node_id).copied() else {
                continue;
            };
            sidecar.nodes.insert(
                id,
                NodeFacts {
                    ordinal: node.ordinal,
                    fate_confirmed: node.fate_confirmed,
                    proposed_fate: node.proposed_fate,
                },
            );
            if let (NodeId::Component(component), Some(scope)) = (id, node.component_scope) {
                sidecar.scopes.insert(component, scope_kind(scope));
            }
            let mut applications: Vec<TagApplication> = node
                .children_hashes
                .iter()
                .filter_map(|child| by_uuid.get(&child.node_id))
                .filter(|child| child.node_type == NodeType::FacetTag)
                .filter_map(|child| {
                    Some(TagApplication {
                        tag: TagName::parse(&child.name).ok()?,
                        node_id: child.node_id,
                        external_id: child.external_id.clone(),
                        ordinal: child.ordinal,
                    })
                })
                .collect();
            applications.sort_by_key(|application| application.ordinal);
            if !applications.is_empty() {
                sidecar.tags.insert(id, applications);
            }
        }
        for aggregate in &export.architecture_decisions {
            sidecar.see(&aggregate.identity.external_id);
            sidecar.decision(aggregate);
            for subject in subjects(aggregate) {
                sidecar.absent(subject, nodes);
            }
        }
        sidecar
    }

    fn see(&mut self, external_id: &ExternalId) {
        let highest = self
            .highest
            .entry(external_id.prefix().to_owned())
            .or_insert(external_id.number());
        *highest = (*highest).max(external_id.number());
    }

    fn absent(&mut self, subject: &SubjectRef, nodes: &NodeIds) {
        if nodes.contains_key(&subject.node_id) {
            return;
        }
        let Some(id) = kind_of(subject.node_type, subject.tier)
            .and_then(|kind| typed_id(kind, subject.node_id))
        else {
            return;
        };
        self.see(&subject.external_id);
        self.absent_subjects.insert(id, subject.external_id.clone());
    }

    fn decision(&mut self, aggregate: &super::export::DecisionAggregate) {
        use super::export::ElementPayload;
        let id = aggregate.identity.decision_id;
        let mut facts = DecisionFacts::default();
        for element in &aggregate.elements {
            let key = Key::from(element.id);
            match element.payload {
                ElementPayload::ArchitectureDecision { .. } if element.id == id => {
                    facts.ordinal = element.ordinal;
                }
                ElementPayload::ArchitectureDecision { .. } => {}
                ElementPayload::DecisionOption { .. } => {
                    facts.options.insert(OptionId::new(key), element.ordinal);
                }
                ElementPayload::Driver(_) => {
                    facts.drivers.insert(DriverId::new(key), element.ordinal);
                }
                ElementPayload::Consequence { .. } => {
                    facts
                        .consequences
                        .insert(ConsequenceId::new(key), element.ordinal);
                }
            }
        }
        self.decisions.insert(DecisionId::new(Key::from(id)), facts);
    }
}

/// Every node reference a decision makes: its subject links, subject drivers and consequence
/// subjects.
fn subjects(aggregate: &super::export::DecisionAggregate) -> impl Iterator<Item = &SubjectRef> {
    use super::export::{DriverPayload, ElementPayload};
    let links = aggregate.subject_links.iter().map(|link| &link.subject);
    let elements = aggregate
        .elements
        .iter()
        .filter_map(|element| match &element.payload {
            ElementPayload::Driver(DriverPayload::SpecificationSubject { subject }) => {
                Some(subject)
            }
            ElementPayload::Consequence { subject, .. } => subject.as_ref(),
            _ => None,
        });
    links.chain(elements)
}

const fn scope_kind(scope: ComponentScope) -> ScopeKind {
    match scope {
        ComponentScope::Document => ScopeKind::Document,
        ComponentScope::External => ScopeKind::External,
        ComponentScope::Product { .. } => ScopeKind::Product,
    }
}

impl V1Sidecar {
    /// The sidecar as JSON: objects keyed by key or prefix in ascending order, two-space indent,
    /// one trailing newline. Equal sidecars give equal bytes (AC-206).
    #[must_use]
    pub fn to_json(&self) -> String {
        let value = json!({
            "absentSubjects": object(self.absent_subjects.iter().map(|(id, external_id)| {
                (id.key().to_string(), json!(external_id.to_string()))
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
