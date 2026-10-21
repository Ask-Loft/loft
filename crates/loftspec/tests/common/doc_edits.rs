//! Random edits of a document held in memory, each of which leaves a document that reads: a node
//! or a decision deleted with the references to it, a `mentions` edge or a `realizes` edge
//! added, a consequence given a node as its subject, and a body changed. Each edit names its
//! targets by an index into the candidates it finds when it is applied, so a sequence of edits
//! applies to any document and shrinks as proptest shrinks the indices.

use loftspec::decision::{DecisionState, DriverPayload, NodeRef};
use loftspec::document::{Document, Edge, EdgeTarget};
use loftspec::edit;
use loftspec::ids::{DecisionId, ElementId, NodeId};
use loftspec::kind::NodeKind;
use proptest::prelude::*;
use proptest::sample::Index;

/// The bodies an edit gives a node.
const BODIES: [&str; 3] = [
    "Changed on one side.",
    "Changed on the other side.",
    "Changed alike.",
];

#[derive(Debug, Clone)]
pub enum DocEdit {
    /// Delete a node, every node beneath it and every reference to them.
    Delete(Index),
    /// Delete a decision and every reference to it.
    DeleteDecision(Index),
    /// Add a `mentions` edge from a node or a decision to another.
    Mention(Index, Index),
    /// Add a `realizes` edge from a Feature to a Component.
    Realize(Index, Index),
    /// Name a node as the subject of a decision's consequence.
    Subject(Index, Index),
    /// Change the body of a Feature or a root Component.
    Body(Index, usize),
}

pub fn doc_edit() -> impl Strategy<Value = DocEdit> {
    let index = any::<Index>;
    prop_oneof![
        3 => index().prop_map(DocEdit::Delete),
        1 => index().prop_map(DocEdit::DeleteDecision),
        3 => (index(), index()).prop_map(|(from, to)| DocEdit::Mention(from, to)),
        2 => (index(), index()).prop_map(|(from, to)| DocEdit::Realize(from, to)),
        2 => (index(), index()).prop_map(|(from, to)| DocEdit::Subject(from, to)),
        2 => (index(), 0..BODIES.len()).prop_map(|(at, body)| DocEdit::Body(at, body)),
    ]
}

/// `document` after `edits`.
pub fn doc_edited(document: &Document, edits: &[DocEdit]) -> Document {
    let mut document = document.clone();
    for edit in edits {
        apply(&mut document, edit);
    }
    document
}

fn pick<T: Copy>(candidates: &[T], index: Index) -> Option<T> {
    (!candidates.is_empty()).then(|| *index.get(candidates))
}

/// Every keyed node of the document, in document order.
fn nodes(document: &Document) -> Vec<NodeId> {
    let mut ids = Vec::new();
    document.walk(&mut |node| ids.extend(node.id()));
    ids
}

fn of_kind(document: &Document, kind: NodeKind) -> Vec<NodeId> {
    nodes(document)
        .into_iter()
        .filter(|id| id.kind() == kind)
        .collect()
}

fn elements(document: &Document) -> Vec<ElementId> {
    let nodes = nodes(document).into_iter().map(ElementId::Node);
    let decisions = document
        .decisions
        .iter()
        .map(|each| ElementId::Decision(each.id));
    nodes.chain(decisions).collect()
}

fn apply(document: &mut Document, edit: &DocEdit) {
    match *edit {
        DocEdit::Delete(at) => {
            if let Some(node) = pick(&nodes(document), at) {
                delete(document, node);
            }
        }
        DocEdit::DeleteDecision(at) => {
            let decisions: Vec<DecisionId> =
                document.decisions.iter().map(|each| each.id).collect();
            if let Some(decision) = pick(&decisions, at) {
                delete_decision(document, decision);
            }
        }
        DocEdit::Mention(from, to) => {
            let elements = elements(document);
            if let (Some(source), Some(target)) = (pick(&elements, from), pick(&elements, to)) {
                add(document, Edge::Mentions { source, target });
            }
        }
        DocEdit::Realize(from, to) => {
            let features = of_kind(document, NodeKind::Feature);
            let components = of_kind(document, NodeKind::Component);
            if let (Some(NodeId::Feature(feature)), Some(NodeId::Component(component))) =
                (pick(&features, from), pick(&components, to))
            {
                add(document, Edge::Realizes { feature, component });
            }
        }
        DocEdit::Subject(from, to) => subject(document, from, to),
        DocEdit::Body(at, body) => {
            let mut bodies: Vec<&mut Option<String>> = document
                .products
                .iter_mut()
                .flat_map(|product| &mut product.feature_groups)
                .flat_map(|group| &mut group.features)
                .map(|feature| &mut feature.details.body)
                .chain(
                    document
                        .components
                        .iter_mut()
                        .map(|root| &mut root.component.details.body),
                )
                .collect();
            if !bodies.is_empty() {
                let at = at.index(bodies.len());
                *bodies[at] = Some(BODIES[body].to_owned());
            }
        }
    }
}

fn add(document: &mut Document, edge: Edge) {
    let to_itself = EdgeTarget::Element(edge.source()) == edge.target();
    if !to_itself && !document.edges.contains(&edge) {
        document.edges.push(edge);
    }
}

fn subject(document: &mut Document, from: Index, to: Index) {
    let nodes = nodes(document);
    let Some(node) = pick(&nodes, to) else {
        return;
    };
    let mut consequences: Vec<_> = document
        .decisions
        .iter_mut()
        .flat_map(|decision| &mut decision.consequences)
        .collect();
    if !consequences.is_empty() {
        let at = from.index(consequences.len());
        consequences[at].subject = Some(NodeRef::Held(node));
    }
}

/// Takes out `node`, every node beneath it, the edges they write and every reference to them,
/// and marks their bindings departed, as a tool that deletes a node leaves the document.
fn delete(document: &mut Document, node: NodeId) {
    let subtree = edit::detach(document, node).expect("a node the document holds");
    let gone: Vec<ElementId> = subtree.ids().into_iter().map(ElementId::Node).collect();
    forget(document, &gone);
}

fn delete_decision(document: &mut Document, decision: DecisionId) {
    document.decisions.retain(|each| each.id != decision);
    let gone = [ElementId::Decision(decision)];
    document
        .edges
        .retain(|edge| edge.source() != ElementId::Decision(decision));
    for each in &mut document.decisions {
        if let DecisionState::Superseded { successor, .. } = &each.state
            && *successor == decision
        {
            each.state = DecisionState::Proposed;
        }
    }
    forget(document, &gone);
}

/// Drops every reference to the elements `gone` and departs their bindings.
fn forget(document: &mut Document, gone: &[ElementId]) {
    let names_gone = |target: EdgeTarget| matches!(target, EdgeTarget::Element(element) if gone.contains(&element));
    document.edges.retain(|edge| !names_gone(edge.target()));
    let held = |subject: &NodeRef| matches!(subject, NodeRef::Held(node) if gone.contains(&ElementId::Node(*node)));
    for decision in &mut document.decisions {
        for consequence in &mut decision.consequences {
            if consequence.subject.as_ref().is_some_and(held) {
                consequence.subject = None;
            }
        }
        for driver in &mut decision.drivers {
            if let DriverPayload::Subject { subject } = &driver.payload
                && held(subject)
            {
                driver.payload = DriverPayload::Local {
                    label: "A driver".to_owned(),
                    statement: "A node it named was deleted.".to_owned(),
                };
            }
        }
    }
    for element in gone {
        if document.bindings.label_of(*element).is_some() {
            document.bindings.depart(*element).expect("a bound element");
        }
    }
}
