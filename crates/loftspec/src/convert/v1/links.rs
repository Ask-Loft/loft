//! The links of a v1 export (D-11 F-13): Realization Links become `realizes` edges (RQ-49), a
//! Component's Product scope a `scoped_to` edge (RQ-50), and each Architecture Decision a
//! decision with its elements and `subject` edges (RQ-51), labelled by its external ID (AC-244).

use std::collections::BTreeMap;

use uuid::Uuid;

use super::export::{
    self, ComponentScope, DecisionAggregate, Element, ElementPayload, Export, ExternalId,
    SubjectRef,
};
use super::tree::{kind_of, typed_id};
use super::{ConvertError, Finding};
use crate::decision::{
    Assessment, Consequence, ConsequenceKind, Decision, DecisionOption, DecisionState, Driver,
    DriverPayload, Rating,
};
use crate::document::{Document, Edge, SubjectRole};
use crate::identity::Label;
use crate::ids::{ConsequenceId, DecisionId, DriverId, NodeId, OptionId};

pub(super) type NodeIds = BTreeMap<Uuid, NodeId>;

/// A `realizes` edge per Realization Link, each from a Feature to a Component.
pub(super) fn realizations(export: &Export, nodes: &NodeIds) -> Result<Vec<Edge>, ConvertError> {
    export
        .realization_links
        .iter()
        .map(
            |link| match (nodes.get(&link.feature_id), nodes.get(&link.component_id)) {
                (Some(NodeId::Feature(feature)), Some(NodeId::Component(component))) => {
                    Ok(Edge::Realizes {
                        feature: *feature,
                        component: *component,
                    })
                }
                _ => Err(ConvertError::Link {
                    feature: link.feature_id,
                    component: link.component_id,
                }),
            },
        )
        .collect()
}

/// A `scoped_to` edge per Component scoped to a Product; other scopes yield no edge.
pub(super) fn scopes(export: &Export, nodes: &NodeIds) -> Result<Vec<Edge>, ConvertError> {
    let mut edges = Vec::new();
    for node in &export.nodes {
        let Some(ComponentScope::Product { product_id }) = node.component_scope else {
            continue;
        };
        match (nodes.get(&node.node_id), nodes.get(&product_id)) {
            (Some(NodeId::Component(component)), Some(NodeId::Product(product))) => {
                edges.push(Edge::ScopedTo {
                    component: *component,
                    product: *product,
                });
            }
            _ => {
                return Err(ConvertError::Scope {
                    node: node.external_id.clone(),
                    product: product_id,
                });
            }
        }
    }
    Ok(edges)
}

/// Every decision with its `subject` edges and its binding.
pub(super) fn decisions(
    export: &Export,
    nodes: &NodeIds,
    document: &mut Document,
    findings: &mut Vec<Finding>,
) -> Result<(), ConvertError> {
    for aggregate in &export.architecture_decisions {
        let reader = Reader {
            aggregate,
            nodes,
            absent: std::cell::RefCell::new(Vec::new()),
        };
        let decision = reader.decision()?;
        for link in &aggregate.subject_links {
            document.edges.push(Edge::Subject {
                decision: decision.id,
                subject: reader.subject(&link.subject)?,
                role: match link.role {
                    export::SubjectRole::Outcome => SubjectRole::Outcome,
                    export::SubjectRole::Affected => SubjectRole::Affected,
                },
            });
        }
        let external_id = &aggregate.identity.external_id;
        let label =
            Label::parse(&external_id.to_string()).map_err(|source| ConvertError::Label {
                node: external_id.clone(),
                source,
            })?;
        document.bindings.bind(decision.id, label)?;
        findings.extend(
            reader
                .absent
                .take()
                .into_iter()
                .map(|subject| Finding::AbsentSubject {
                    decision: decision.id,
                    subject,
                }),
        );
        document.decisions.push(decision);
    }
    Ok(())
}

struct Reader<'a> {
    aggregate: &'a DecisionAggregate,
    nodes: &'a NodeIds,
    /// The subjects the decision names that the export does not hold, in order.
    absent: std::cell::RefCell<Vec<NodeId>>,
}

impl Reader<'_> {
    fn external_id(&self) -> ExternalId {
        self.aggregate.identity.external_id.clone()
    }

    fn element_error(&self, element: Uuid, reason: &'static str) -> ConvertError {
        ConvertError::DecisionElement {
            decision: self.external_id(),
            element,
            reason,
        }
    }

    /// The node a subject reference names. A v1 decision may still name a node the export no
    /// longer holds (a retired decision keeps its history); that reference is kept under its
    /// v1 key and kind and reported (D-11 gap G19).
    fn subject(&self, subject: &SubjectRef) -> Result<NodeId, ConvertError> {
        if let Some(id) = self.nodes.get(&subject.node_id) {
            return Ok(*id);
        }
        let id = kind_of(subject.node_type, subject.tier)
            .and_then(|kind| typed_id(kind, subject.node_id))
            .ok_or_else(|| ConvertError::DecisionSubject {
                decision: self.external_id(),
                subject: subject.node_id,
            })?;
        let mut absent = self.absent.borrow_mut();
        if !absent.contains(&id) {
            absent.push(id);
        }
        Ok(id)
    }

    /// The decision's root element and its members in ordinal order; each member's parent is
    /// the root.
    fn elements(&self) -> Result<(&Element, Vec<&Element>), ConvertError> {
        let id = self.aggregate.identity.decision_id;
        let mut root = None;
        let mut members = Vec::new();
        for element in &self.aggregate.elements {
            match element.parent_id {
                None if element.id == id && root.is_none() => root = Some(element),
                Some(parent) if parent == id => members.push(element),
                _ => {
                    return Err(
                        self.element_error(element.id, "is neither the decision nor its child")
                    );
                }
            }
        }
        let root = root.ok_or_else(|| self.element_error(id, "is missing"))?;
        members.sort_by_key(|element| element.ordinal);
        Ok((root, members))
    }

    fn decision(&self) -> Result<Decision, ConvertError> {
        let (root, members) = self.elements()?;
        let ElementPayload::ArchitectureDecision {
            title,
            context,
            state,
        } = &root.payload
        else {
            return Err(self.element_error(root.id, "is not an architecture decision"));
        };
        let mut decision = Decision {
            id: DecisionId::new(root.id.into()),
            title: title.clone(),
            context: context.clone(),
            state: decision_state(state),
            drivers: Vec::new(),
            options: Vec::new(),
            assessments: self.assessments(),
            consequences: Vec::new(),
        };
        for member in members {
            self.add_member(&mut decision, member)?;
        }
        Ok(decision)
    }

    fn add_member(&self, decision: &mut Decision, member: &Element) -> Result<(), ConvertError> {
        match &member.payload {
            ElementPayload::ArchitectureDecision { .. } => {
                return Err(self.element_error(member.id, "is a second decision"));
            }
            ElementPayload::DecisionOption { title, description } => {
                decision.options.push(DecisionOption {
                    id: OptionId::new(member.id.into()),
                    title: title.clone(),
                    description: description.clone(),
                });
            }
            ElementPayload::Driver(payload) => decision.drivers.push(Driver {
                id: DriverId::new(member.id.into()),
                payload: match payload {
                    export::DriverPayload::Local { label, statement } => DriverPayload::Local {
                        label: label.clone(),
                        statement: statement.clone(),
                    },
                    export::DriverPayload::SpecificationSubject { subject } => {
                        DriverPayload::Subject {
                            subject: self.subject(subject)?,
                        }
                    }
                },
            }),
            ElementPayload::Consequence {
                kind,
                statement,
                subject,
            } => decision.consequences.push(Consequence {
                id: ConsequenceId::new(member.id.into()),
                kind: consequence_kind(*kind),
                statement: statement.clone(),
                subject: subject
                    .as_ref()
                    .map(|subject| self.subject(subject))
                    .transpose()?,
            }),
        }
        Ok(())
    }

    fn assessments(&self) -> Vec<Assessment> {
        self.aggregate
            .assessments
            .iter()
            .map(|assessment| Assessment {
                option: OptionId::new(assessment.option_id.into()),
                driver: DriverId::new(assessment.driver_id.into()),
                rating: match assessment.rating {
                    export::Rating::Supports => Rating::Supports,
                    export::Rating::Neutral => Rating::Neutral,
                    export::Rating::Conflicts => Rating::Conflicts,
                    export::Rating::Unknown => Rating::Unknown,
                },
                rationale: assessment.rationale.clone(),
            })
            .collect()
    }
}

fn decision_state(state: &export::DecisionState) -> DecisionState {
    let option = |id: &Uuid| OptionId::new((*id).into());
    match state {
        export::DecisionState::Proposed => DecisionState::Proposed,
        export::DecisionState::Accepted {
            selected_option_id,
            outcome,
        } => DecisionState::Accepted {
            selected_option: option(selected_option_id),
            outcome: outcome.clone(),
        },
        export::DecisionState::Rejected { rationale } => DecisionState::Rejected {
            rationale: rationale.clone(),
        },
        export::DecisionState::Superseded {
            selected_option_id,
            outcome,
            successor_decision_id,
        } => DecisionState::Superseded {
            selected_option: option(selected_option_id),
            outcome: outcome.clone(),
            successor: DecisionId::new((*successor_decision_id).into()),
        },
        export::DecisionState::Retired {
            selected_option_id,
            outcome,
            rationale,
        } => DecisionState::Retired {
            selected_option: option(selected_option_id),
            outcome: outcome.clone(),
            rationale: rationale.clone(),
        },
    }
}

const fn consequence_kind(kind: export::ConsequenceKind) -> ConsequenceKind {
    match kind {
        export::ConsequenceKind::Benefit => ConsequenceKind::Benefit,
        export::ConsequenceKind::Cost => ConsequenceKind::Cost,
        export::ConsequenceKind::Risk => ConsequenceKind::Risk,
        export::ConsequenceKind::Obligation => ConsequenceKind::Obligation,
    }
}
