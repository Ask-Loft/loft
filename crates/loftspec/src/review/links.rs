//! What bears on a unit beyond its nodes: the decisions that name, are driven by, bear the
//! consequences of or mention an assertion or the scope, and the terms the assertions use. The
//! links of every decision are found once for a side and the phrases of every term once for a
//! packet, then looked up for each unit.

use std::collections::{BTreeMap, BTreeSet};

use crate::decision::{DriverPayload, NodeRef};
use crate::document::{Document, Edge, SubjectRole, TermEntry};
use crate::ids::{DecisionId, ElementId, NodeId};
use crate::lint::{Prose, phrase};
use crate::node::Node;

use super::{Linked, Via};

/// Each node a decision links to, with every decision that does, by its place in the
/// document's decisions, and how.
pub(super) type DecisionLinks = BTreeMap<NodeId, Vec<(usize, Via)>>;

/// The node a reference names, where the document holds it.
const fn held(reference: &NodeRef) -> Option<NodeId> {
    match reference {
        NodeRef::Held(id) => Some(*id),
        NodeRef::Former(_) => None,
    }
}

const fn subject_via(role: SubjectRole) -> Via {
    match role {
        SubjectRole::Outcome => Via::SubjectOutcome,
        SubjectRole::Affected => Via::SubjectAffected,
    }
}

/// The links of every decision of `document`: its subject and mention edges, the drivers that
/// name a node and the consequences that bear on one.
pub(super) fn decision_links(document: &Document) -> DecisionLinks {
    let places: BTreeMap<DecisionId, usize> = document
        .decisions
        .iter()
        .enumerate()
        .map(|(at, decision)| (decision.id, at))
        .collect();
    let mut links = DecisionLinks::new();
    let mut add = |decision: DecisionId, id: Option<NodeId>, via: Via| {
        if let (Some(at), Some(id)) = (places.get(&decision), id) {
            links.entry(id).or_default().push((*at, via));
        }
    };
    for edge in &document.edges {
        match edge {
            Edge::Subject {
                decision,
                subject,
                role,
            } => add(*decision, held(subject), subject_via(*role)),
            Edge::Mentions {
                source: ElementId::Decision(decision),
                target: ElementId::Node(id),
            }
            | Edge::Mentions {
                source: ElementId::Node(id),
                target: ElementId::Decision(decision),
            } => add(*decision, Some(*id), Via::Mentions),
            _ => {}
        }
    }
    for decision in &document.decisions {
        for driver in &decision.drivers {
            if let DriverPayload::Subject { subject } = &driver.payload {
                add(decision.id, held(subject), Via::Driver);
            }
        }
        for consequence in &decision.consequences {
            let id = consequence.subject.as_ref().and_then(held);
            add(decision.id, id, Via::Consequence);
        }
    }
    links
}

/// Each decision of `document` that links to a node of `bearing`, by `links`, with its links in
/// the order of `bearing` and then of how they link; the decisions by label, then key.
pub(super) fn decisions(
    document: &Document,
    links: &DecisionLinks,
    bearing: &[NodeId],
) -> Vec<Linked> {
    let mut seen = BTreeSet::new();
    let mut by_decision: BTreeMap<usize, Vec<(usize, Via, NodeId)>> = BTreeMap::new();
    for (at, id) in bearing.iter().enumerate() {
        if !seen.insert(*id) {
            continue;
        }
        for (decision, via) in links.get(id).into_iter().flatten() {
            by_decision
                .entry(*decision)
                .or_default()
                .push((at, *via, *id));
        }
    }
    let mut linked: Vec<Linked> = by_decision
        .into_iter()
        .map(|(at, mut links)| {
            links.sort_unstable();
            links.dedup();
            let decision = &document.decisions[at];
            Linked {
                id: decision.id,
                label: document.bindings.label_of(decision.id),
                title: decision.title.clone(),
                state: decision.state.kind(),
                outcome: decision.state.outcome().map(str::to_owned),
                links: links.into_iter().map(|(_, via, id)| (id, via)).collect(),
            }
        })
        .collect();
    linked.sort_by_key(|each| (each.label.is_none(), each.label, each.id.key()));
    linked
}

/// Each entry of a term list with its name and aliases as compared, those with no words left
/// out.
pub(super) type Phrases<'a> = Vec<(&'a TermEntry, Vec<Vec<String>>)>;

/// The phrases of `terms`, read once for a packet.
pub(super) fn phrases(terms: &[TermEntry]) -> Phrases<'_> {
    terms
        .iter()
        .map(|term| {
            let phrases = std::iter::once(&term.name)
                .chain(&term.aliases)
                .map(|text| phrase(text))
                .filter(|words| !words.is_empty())
                .collect();
            (term, phrases)
        })
        .collect()
}

/// The entries of `phrases` whose name or an alias occurs in the name or body of a node of
/// `prose`, by name.
pub(super) fn terms<'a>(
    phrases: &Phrases<'_>,
    prose: impl Iterator<Item = Node<'a>>,
) -> Vec<TermEntry> {
    let texts: Vec<Prose> = prose
        .map(|node| {
            let body = node.details().and_then(|details| details.body.as_deref());
            Prose::of(&node.name(), body)
        })
        .collect();
    let mut used: Vec<TermEntry> = phrases
        .iter()
        .filter(|(_, phrases)| {
            phrases
                .iter()
                .any(|phrase| texts.iter().any(|text| text.uses(phrase)))
        })
        .map(|(term, _)| (*term).clone())
        .collect();
    used.sort_by(|a, b| (&a.name, a.id).cmp(&(&b.name, b.id)));
    used
}
