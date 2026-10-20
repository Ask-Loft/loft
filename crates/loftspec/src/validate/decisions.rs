//! Former labels (RQ-179, RQ-180) and the completeness a decision's state requires (RQ-184,
//! RQ-185). References inside a decision and its successors are checked by the index.

use super::ValidationError;
use crate::decision::{Decision, DriverPayload, NodeRef, StateKind};
use crate::document::{Document, Edge, SubjectRole};
use crate::identity::Label;

pub(super) fn check(document: &Document, errors: &mut Vec<ValidationError>) {
    for decision in &document.decisions {
        former_labels(document, decision, errors);
        completeness(document, decision, errors);
    }
}

/// Every former label the document's decisions name, in one pass over the edges and the
/// decisions: in `subject` edges, drivers and consequences.
pub(crate) fn all_former_labels<'a>(
    edges: &'a [Edge],
    decisions: &'a [Decision],
) -> impl Iterator<Item = Label> + 'a {
    let edges = edges.iter().filter_map(|edge| match *edge {
        Edge::Subject { subject, .. } => Some(subject),
        _ => None,
    });
    let elements = decisions.iter().flat_map(|decision| {
        let drivers = decision
            .drivers
            .iter()
            .filter_map(|driver| match driver.payload {
                DriverPayload::Subject { subject } => Some(subject),
                DriverPayload::Local { .. } => None,
            });
        drivers.chain(decision.consequences.iter().filter_map(|c| c.subject))
    });
    edges.chain(elements).filter_map(|subject| match subject {
        NodeRef::Former(label) => Some(label),
        NodeRef::Held(_) => None,
    })
}

/// Every former label a decision names: in its `subject` edges, its drivers and its
/// consequences.
pub(crate) fn former_labels_of<'a>(
    document: &'a Document,
    decision: &'a Decision,
) -> impl Iterator<Item = Label> + 'a {
    let edges = document.edges.iter().filter_map(move |edge| match *edge {
        Edge::Subject {
            decision: from,
            subject,
            ..
        } if from == decision.id => Some(subject),
        _ => None,
    });
    let drivers = decision
        .drivers
        .iter()
        .filter_map(|driver| match driver.payload {
            DriverPayload::Subject { subject } => Some(subject),
            DriverPayload::Local { .. } => None,
        });
    let consequences = decision.consequences.iter().filter_map(|c| c.subject);
    edges
        .chain(drivers)
        .chain(consequences)
        .filter_map(|subject| match subject {
            NodeRef::Former(label) => Some(label),
            NodeRef::Held(_) => None,
        })
}

fn former_labels(document: &Document, decision: &Decision, errors: &mut Vec<ValidationError>) {
    let kind = decision.state.kind();
    let terminal = matches!(kind, StateKind::Retired | StateKind::Superseded);
    for label in former_labels_of(document, decision) {
        if !terminal {
            errors.push(ValidationError::FormerLabelInLiveDecision {
                decision: decision.id,
                state: kind.as_str(),
                label,
            });
        }
        if let Some(element) = document.bindings.resolve(label) {
            errors.push(ValidationError::FormerLabelInUse { label, element });
        }
    }
}

/// A resolved decision weighs at least two options against a driver, with every assessment
/// (RQ-184); an accepted, superseded or retired one also has a consequence and an `outcome`
/// subject (RQ-185).
fn completeness(document: &Document, decision: &Decision, errors: &mut Vec<ValidationError>) {
    let kind = decision.state.kind();
    if kind == StateKind::Proposed {
        return;
    }
    let id = decision.id;
    if decision.options.len() < 2 {
        errors.push(ValidationError::TooFewOptions(id));
    }
    if decision.drivers.is_empty() {
        errors.push(ValidationError::NoDriver(id));
    }
    for option in &decision.options {
        for driver in &decision.drivers {
            let assessed = decision
                .assessments
                .iter()
                .any(|cell| cell.option == option.id && cell.driver == driver.id);
            if !assessed {
                errors.push(ValidationError::MissingAssessment {
                    decision: id,
                    option: option.id,
                    driver: driver.id,
                });
            }
        }
    }
    if kind == StateKind::Rejected {
        return;
    }
    if decision.consequences.is_empty() {
        errors.push(ValidationError::NoConsequence(id));
    }
    let has_outcome = document.edges.iter().any(|edge| {
        matches!(*edge, Edge::Subject { decision: from, role: SubjectRole::Outcome, .. } if from == id)
    });
    if !has_outcome {
        errors.push(ValidationError::NoOutcomeSubject(id));
    }
}
