//! Decision views (RQ-161): every reference inside a decision is by position or by content hash,
//! never by key, so fresh element keys leave the view unchanged (AC-611, AC-612). The decision
//! history view is the decision view with each node and successor it names given by key, so
//! that editing a node a decision weighs leaves it unchanged. The decisions come from an indexed
//! document, so every reference resolves and successors form no cycle.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::canonical::{Digest, Domain, digest};
use crate::decision::{Decision, DecisionState, DriverPayload, NodeRef};
use crate::identity::Key;
use crate::ids::{DecisionId, DriverId, OptionId};

use super::NodeIndex;

/// Hashes every decision once, following supersession to its successor.
pub(crate) struct DecisionHasher<'a> {
    decisions: BTreeMap<DecisionId, &'a Decision>,
    nodes: &'a NodeIndex,
    hashes: BTreeMap<DecisionId, Digest>,
    views: BTreeMap<DecisionId, Value>,
}

/// The view and its hash for every decision of a document.
pub(crate) type DecisionViews = (BTreeMap<DecisionId, Digest>, BTreeMap<DecisionId, Value>);

impl<'a> DecisionHasher<'a> {
    pub(crate) fn new(decisions: &'a [Decision], nodes: &'a NodeIndex) -> Self {
        Self {
            decisions: decisions
                .iter()
                .map(|decision| (decision.id, decision))
                .collect(),
            nodes,
            hashes: BTreeMap::new(),
            views: BTreeMap::new(),
        }
    }

    pub(crate) fn hash_all(mut self) -> DecisionViews {
        let keys: Vec<DecisionId> = self.decisions.keys().copied().collect();
        for key in keys {
            self.hash(key);
        }
        (self.hashes, self.views)
    }

    fn hash(&mut self, key: DecisionId) -> Digest {
        if let Some(hash) = self.hashes.get(&key) {
            return *hash;
        }
        let decision = self.decisions[&key];
        let state = state_view(decision, &decision.state, &mut |successor| {
            json!(self.hash(successor))
        });
        let nodes = self.nodes;
        let node = |subject: NodeRef| match subject {
            NodeRef::Held(node) => json!(super::node_hash(nodes, node)),
            NodeRef::Former(label) => super::former(label),
        };
        let view = decision_view(decision, &state, &node);
        let hash = digest(Domain::Decision, &view);
        self.hashes.insert(key, hash);
        self.views.insert(key, view);
        hash
    }
}

/// The decision history view of a decision in `state`, its own or one it is taken back to: its
/// decision view with each node a driver or a consequence names, and its successor, given by
/// key; a former label as the decision view gives it.
pub(crate) fn history_view(decision: &Decision, state: &DecisionState) -> Value {
    let state = state_view(decision, state, &mut |successor| json!(successor.key()));
    let node = |subject: NodeRef| match subject {
        NodeRef::Held(node) => json!(node.key()),
        NodeRef::Former(label) => super::former(label),
    };
    decision_view(decision, &state, &node)
}

/// The view of a decision's state, its successor given by `successor`.
fn state_view(
    decision: &Decision,
    state: &DecisionState,
    successor: &mut dyn FnMut(DecisionId) -> Value,
) -> Value {
    let option = |selected: OptionId| option_position(decision, selected);
    let kind = state.kind().as_str();
    match state {
        DecisionState::Proposed => json!({ "kind": kind }),
        DecisionState::Accepted {
            selected_option,
            outcome,
        } => json!({
            "kind": kind,
            "selectedOption": option(*selected_option),
            "outcome": outcome,
        }),
        DecisionState::Rejected { rationale } => {
            json!({ "kind": kind, "rationale": rationale })
        }
        DecisionState::Superseded {
            selected_option,
            outcome,
            successor: next,
        } => json!({
            "kind": kind,
            "selectedOption": option(*selected_option),
            "outcome": outcome,
            "successor": successor(*next),
        }),
        DecisionState::Retired {
            selected_option,
            outcome,
            rationale,
        } => json!({
            "kind": kind,
            "selectedOption": option(*selected_option),
            "outcome": outcome,
            "rationale": rationale,
        }),
    }
}

/// The decision view: title, context, state, drivers, options, assessments and consequences, in
/// that order (AC-610), each node named as `node` names it. Options and drivers keep their
/// authored order; an assessment names its option and driver by position and the assessments
/// are sorted by those positions.
fn decision_view(decision: &Decision, state: &Value, node: &dyn Fn(NodeRef) -> Value) -> Value {
    let drivers = decision
        .drivers
        .iter()
        .map(|driver| (driver.payload.kind().as_str(), &driver.payload))
        .map(|(kind, payload)| match payload {
            DriverPayload::Local { label, statement } => {
                json!({ "kind": kind, "label": label, "statement": statement })
            }
            DriverPayload::Subject { subject } => {
                json!({ "kind": kind, "subject": node(*subject) })
            }
        })
        .collect::<Vec<_>>();
    let options: Vec<[&str; 2]> = decision
        .options
        .iter()
        .map(|option| [option.title.as_str(), option.description.as_str()])
        .collect();
    let consequences = decision
        .consequences
        .iter()
        .map(|consequence| {
            let subject = consequence.subject.map(node);
            json!([consequence.kind.as_str(), consequence.statement, subject])
        })
        .collect::<Vec<_>>();
    json!([
        decision.title,
        decision.context,
        state,
        drivers,
        options,
        assessment_views(decision),
        consequences,
    ])
}

fn assessment_views(decision: &Decision) -> Vec<Value> {
    let mut rows: Vec<_> = decision
        .assessments
        .iter()
        .map(|assessment| {
            (
                option_position(decision, assessment.option),
                driver_position(decision, assessment.driver),
                assessment.rating.as_str(),
                assessment.rationale.as_str(),
            )
        })
        .collect();
    rows.sort_unstable();
    rows.into_iter()
        .map(|(option, driver, rating, rationale)| json!([option, driver, rating, rationale]))
        .collect()
}

fn option_position(decision: &Decision, option: OptionId) -> usize {
    let keys = decision.options.iter().map(|option| option.id.key());
    position(keys, option.key())
}

fn driver_position(decision: &Decision, driver: DriverId) -> usize {
    let keys = decision.drivers.iter().map(|driver| driver.id.key());
    position(keys, driver.key())
}

/// The position of `key` among a decision's element keys, which indexing guarantees it is among.
fn position(mut keys: impl Iterator<Item = Key>, key: Key) -> usize {
    keys.position(|candidate| candidate == key)
        .expect("an indexed decision names only its own options and drivers")
}
