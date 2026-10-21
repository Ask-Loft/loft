//! What names the elements a settled deletion took out: a retired or superseded decision names a
//! node it took out by the node's former label, the live label its binding held, as formatting
//! names it; every other edge that names one is removed; and each element's binding is marked
//! departed. A decision that is not settled and still names a node it took out keeps that name,
//! so the settled document does not read.

use std::collections::{BTreeMap, BTreeSet};

use crate::decision::{Decision, DriverPayload, NodeRef};
use crate::document::{Document, Edge, EdgeTarget};
use crate::identity::Label;
use crate::ids::{ElementId, NodeId};

/// Forgets the elements `gone`, which a deletion took out of `document`.
pub(super) fn forget_elements(document: &mut Document, gone: &BTreeSet<ElementId>) {
    if gone.is_empty() {
        return;
    }
    let labels: BTreeMap<NodeId, Label> = gone
        .iter()
        .filter_map(|element| match element {
            ElementId::Node(node) => Some((*node, document.bindings.label_of(*element)?)),
            ElementId::Decision(_) => None,
        })
        .collect();
    let settled = |decision: &Decision| decision.state.kind().is_settled();
    let mut named_formerly = BTreeSet::new();
    for decision in &mut document.decisions {
        if settled(decision) {
            name_formerly(decision, &labels);
            named_formerly.insert(decision.id);
        }
    }
    for edge in &mut document.edges {
        if let Edge::Subject {
            decision, subject, ..
        } = edge
            && named_formerly.contains(decision)
        {
            formerly(subject, &labels);
        }
    }
    let named = |edge: &Edge| matches!(edge.target(), EdgeTarget::Element(element) if gone.contains(&element));
    document.edges.retain(|edge| !named(edge));
    for element in gone {
        if document.bindings.label_of(*element).is_some() {
            document
                .bindings
                .depart(*element)
                .expect("a bound element departs");
        }
    }
}

/// Names each node of `labels` that `decision`'s consequences and drivers hold by its label.
fn name_formerly(decision: &mut Decision, labels: &BTreeMap<NodeId, Label>) {
    for consequence in &mut decision.consequences {
        if let Some(subject) = &mut consequence.subject {
            formerly(subject, labels);
        }
    }
    for driver in &mut decision.drivers {
        if let DriverPayload::Subject { subject } = &mut driver.payload {
            formerly(subject, labels);
        }
    }
}

fn formerly(reference: &mut NodeRef, labels: &BTreeMap<NodeId, Label>) {
    if let NodeRef::Held(node) = reference
        && let Some(label) = labels.get(node)
    {
        *reference = NodeRef::Former(*label);
    }
}
