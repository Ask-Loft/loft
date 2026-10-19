//! What bears on a unit beyond its nodes: the decisions that name, are driven by, bear the
//! consequences of or mention an assertion or the scope, and the terms the assertions use.

use crate::decision::{DecisionState, DriverPayload, NodeRef};
use crate::document::{Document, Edge, SubjectRole, TermEntry};
use crate::ids::{DecisionId, ElementId, NodeId};
use crate::lint::occurs_in;
use crate::node::Node;

use super::{Linked, Via};

/// The node a reference names, where the document holds it.
const fn held(reference: &NodeRef) -> Option<NodeId> {
    match reference {
        NodeRef::Held(id) => Some(*id),
        NodeRef::Former(_) => None,
    }
}

/// The nodes the edges of `document` link to the decision `decision`, and how.
fn edge_links(document: &Document, decision: DecisionId) -> impl Iterator<Item = (NodeId, Via)> {
    let own = ElementId::Decision(decision);
    document.edges.iter().filter_map(move |edge| match edge {
        Edge::Subject {
            decision: of,
            subject,
            role,
        } if *of == decision => {
            let via = match role {
                SubjectRole::Outcome => Via::SubjectOutcome,
                SubjectRole::Affected => Via::SubjectAffected,
            };
            held(subject).map(|id| (id, via))
        }
        Edge::Mentions {
            source,
            target: ElementId::Node(id),
        } if *source == own => Some((*id, Via::Mentions)),
        Edge::Mentions {
            source: ElementId::Node(id),
            target,
        } if *target == own => Some((*id, Via::Mentions)),
        _ => None,
    })
}

/// Each decision of `document` that links to a node of `bearing`, with its links in the order
/// of `bearing` and then of how they link; the decisions by label, then key.
pub(super) fn decisions(document: &Document, bearing: &[NodeId]) -> Vec<Linked> {
    let mut linked: Vec<Linked> = document
        .decisions
        .iter()
        .filter_map(|decision| {
            let drivers = decision
                .drivers
                .iter()
                .filter_map(|driver| match &driver.payload {
                    DriverPayload::Subject { subject } => held(subject).map(|id| (id, Via::Driver)),
                    DriverPayload::Local { .. } => None,
                });
            let consequences = decision.consequences.iter().filter_map(|consequence| {
                let id = held(consequence.subject.as_ref()?)?;
                Some((id, Via::Consequence))
            });
            let mut links: Vec<(usize, Via, NodeId)> = edge_links(document, decision.id)
                .chain(drivers)
                .chain(consequences)
                .filter_map(|(id, via)| {
                    let at = bearing.iter().position(|each| *each == id)?;
                    Some((at, via, id))
                })
                .collect();
            links.sort_unstable();
            links.dedup();
            (!links.is_empty()).then(|| Linked {
                id: decision.id,
                label: document.bindings.label_of(decision.id),
                title: decision.title.clone(),
                state: decision.state.kind(),
                outcome: outcome(&decision.state),
                links: links.into_iter().map(|(_, via, id)| (id, via)).collect(),
            })
        })
        .collect();
    linked.sort_by_key(|each| (each.label.is_none(), each.label, each.id.key()));
    linked
}

fn outcome(state: &DecisionState) -> Option<String> {
    match state {
        DecisionState::Accepted { outcome, .. }
        | DecisionState::Superseded { outcome, .. }
        | DecisionState::Retired { outcome, .. } => Some(outcome.clone()),
        DecisionState::Proposed | DecisionState::Rejected { .. } => None,
    }
}

/// The entries of `terms` whose name or an alias occurs in the name or body of a node of
/// `prose`, by name.
pub(super) fn terms<'a>(
    terms: &[TermEntry],
    prose: impl Iterator<Item = Node<'a>>,
) -> Vec<TermEntry> {
    let texts: Vec<(String, Option<&str>)> = prose
        .map(|node| {
            let body = node.details().and_then(|details| details.body.as_deref());
            (node.name().into_owned(), body)
        })
        .collect();
    let mut used: Vec<TermEntry> = terms
        .iter()
        .filter(|term| {
            std::iter::once(&term.name)
                .chain(&term.aliases)
                .any(|phrase| {
                    texts
                        .iter()
                        .any(|(name, body)| occurs_in(name, *body, phrase))
                })
        })
        .cloned()
        .collect();
    used.sort_by(|a, b| (&a.name, a.id).cmp(&(&b.name, b.id)));
    used
}
