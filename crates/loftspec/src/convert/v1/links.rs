//! The links of a v1 export (D-11 F-13): Realization Links become `realizes` edges (RQ-49), a
//! Component's Product scope a `scoped_to` edge (RQ-50), and each Architecture Decision a
//! decision with its elements and `subject` edges (RQ-51), labelled by its external ID (AC-244).

use uuid::Uuid;

use super::export::{
    self, ComponentScope, DecisionAggregate, Element, ElementPayload, Export, ExternalId,
    SubjectRef,
};
use super::sidecar::DecisionFacts;
use super::tree::{NodeIds, node_id};
use super::{Conversion, ConvertError, label};
use crate::decision::{
    Assessment, Consequence, Decision, DecisionOption, DecisionState, Driver, DriverPayload,
    NodeRef,
};
use crate::document::Edge;
use crate::identity::{Key, Label};
use crate::ids::{ConsequenceId, DecisionId, DriverId, NodeId, OptionId};
use crate::slug::Slug;

/// Adds the export's `realizes` and `scoped_to` edges, then its decisions with their `subject`
/// edges, bindings and sidecar facts.
pub(super) fn convert(
    export: &Export,
    nodes: &NodeIds,
    conversion: &mut Conversion,
) -> Result<(), ConvertError> {
    let edges = &mut conversion.document.edges;
    edges.extend(realizations(export, nodes)?);
    edges.extend(scopes(export, nodes)?);
    for aggregate in &export.architecture_decisions {
        decision(aggregate, nodes, conversion)?;
    }
    Ok(())
}

/// A `realizes` edge per Realization Link, each from a Feature to a Component.
fn realizations(export: &Export, nodes: &NodeIds) -> Result<Vec<Edge>, ConvertError> {
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
fn scopes(export: &Export, nodes: &NodeIds) -> Result<Vec<Edge>, ConvertError> {
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

/// One decision with its `subject` edges, its binding, its element ordinals and the v1 node
/// behind each former label it names.
fn decision(
    aggregate: &DecisionAggregate,
    nodes: &NodeIds,
    conversion: &mut Conversion,
) -> Result<(), ConvertError> {
    let mut reader = Reader {
        aggregate,
        nodes,
        facts: DecisionFacts::default(),
        former: Vec::new(),
    };
    let decision = reader.decision()?;
    for link in &aggregate.subject_links {
        let subject = reader.subject(&link.subject)?;
        conversion.document.edges.push(Edge::Subject {
            decision: decision.id,
            subject,
            role: link.role,
        });
    }
    let external_id = &aggregate.identity.external_id;
    conversion
        .document
        .bindings
        .bind(decision.id, label(external_id)?)?;
    conversion.sidecar.former_labels.extend(reader.former);
    conversion
        .sidecar
        .decisions
        .insert(decision.id, reader.facts);
    conversion.document.decisions.push(decision);
    Ok(())
}

struct Reader<'a> {
    aggregate: &'a DecisionAggregate,
    nodes: &'a NodeIds,
    facts: DecisionFacts,
    /// Each former label the decision names, with the v1 node UUID behind it.
    former: Vec<(Label, Uuid)>,
}

impl<'a> Reader<'a> {
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
    /// longer holds, as a retired decision keeps its history; that reference becomes the node's
    /// v1 external ID as a former label (ADR-13).
    fn subject(&mut self, subject: &SubjectRef) -> Result<NodeRef, ConvertError> {
        if let Some(id) = self.nodes.get(&subject.node_id) {
            return Ok(NodeRef::Held(*id));
        }
        if node_id(subject.node_type, subject.tier, subject.node_id).is_none() {
            return Err(ConvertError::DecisionSubject {
                decision: self.external_id(),
                subject: subject.node_id,
            });
        }
        let former = label(&subject.external_id)?;
        if !self.former.iter().any(|(known, _)| *known == former) {
            self.former.push((former, subject.node_id));
        }
        Ok(NodeRef::Former(former))
    }

    /// The decision's root element and its members in ordinal order; each member's parent is
    /// the root.
    fn elements(&self) -> Result<(&'a Element, Vec<&'a Element>), ConvertError> {
        let aggregate = self.aggregate;
        let id = aggregate.identity.decision_id;
        let mut root = None;
        let mut members = Vec::new();
        for element in &aggregate.elements {
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

    fn decision(&mut self) -> Result<Decision, ConvertError> {
        let (root, members) = self.elements()?;
        let ElementPayload::ArchitectureDecision {
            title,
            context,
            state,
        } = &root.payload
        else {
            return Err(self.element_error(root.id, "is not an architecture decision"));
        };
        self.facts.ordinal = root.ordinal;
        let mut decision = Decision {
            id: DecisionId::new(root.id.into()),
            slug: Slug::derive(title, "decision"),
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

    fn add_member(
        &mut self,
        decision: &mut Decision,
        member: &Element,
    ) -> Result<(), ConvertError> {
        let key = Key::from(member.id);
        match &member.payload {
            ElementPayload::ArchitectureDecision { .. } => {
                return Err(self.element_error(member.id, "is a second decision"));
            }
            ElementPayload::DecisionOption { title, description } => {
                let id = OptionId::new(key);
                self.facts.options.insert(id, member.ordinal);
                decision.options.push(DecisionOption {
                    id,
                    title: title.clone(),
                    description: description.clone(),
                });
            }
            ElementPayload::Driver(payload) => {
                let id = DriverId::new(key);
                self.facts.drivers.insert(id, member.ordinal);
                let payload = match payload {
                    export::DriverPayload::Local { label, statement } => DriverPayload::Local {
                        label: label.clone(),
                        statement: statement.clone(),
                    },
                    export::DriverPayload::SpecificationSubject { subject } => {
                        DriverPayload::Subject {
                            subject: self.subject(subject)?,
                        }
                    }
                };
                decision.drivers.push(Driver { id, payload });
            }
            ElementPayload::Consequence {
                kind,
                statement,
                subject,
            } => {
                let id = ConsequenceId::new(key);
                self.facts.consequences.insert(id, member.ordinal);
                let subject = subject
                    .as_ref()
                    .map(|subject| self.subject(subject))
                    .transpose()?;
                decision.consequences.push(Consequence {
                    id,
                    kind: *kind,
                    statement: statement.clone(),
                    subject,
                });
            }
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
                rating: assessment.rating,
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
