//! Decision views (RQ-161): every reference inside a decision is by position or by content hash,
//! never by key, so fresh element keys leave the view unchanged (AC-611, AC-612).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::HashError;
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
    in_progress: BTreeSet<DecisionId>,
}

/// The view and its hash for every decision of a document.
pub(crate) type DecisionViews = (BTreeMap<DecisionId, Digest>, BTreeMap<DecisionId, Value>);

impl<'a> DecisionHasher<'a> {
    pub(crate) fn new(decisions: &'a [Decision], nodes: &'a NodeIndex) -> Result<Self, HashError> {
        let mut by_key = BTreeMap::new();
        for decision in decisions {
            let key = decision.id.key();
            if nodes.contains_key(&key) || by_key.insert(decision.id, decision).is_some() {
                return Err(HashError::DuplicateKey(key));
            }
        }
        Ok(Self {
            decisions: by_key,
            nodes,
            hashes: BTreeMap::new(),
            views: BTreeMap::new(),
            in_progress: BTreeSet::new(),
        })
    }

    pub(crate) fn hash_all(mut self) -> Result<DecisionViews, HashError> {
        let keys: Vec<DecisionId> = self.decisions.keys().copied().collect();
        for key in keys {
            self.hash(key)?;
        }
        Ok((self.hashes, self.views))
    }

    fn hash(&mut self, key: DecisionId) -> Result<Digest, HashError> {
        if let Some(hash) = self.hashes.get(&key) {
            return Ok(*hash);
        }
        if !self.in_progress.insert(key) {
            return Err(HashError::SupersessionCycle(key));
        }
        let decision = *self
            .decisions
            .get(&key)
            .ok_or(HashError::UnknownDecision(key))?;
        let state = self.state_view(decision)?;
        let view = decision_view(decision, &state, self.nodes)?;
        let hash = digest(Domain::Decision, &view);
        self.in_progress.remove(&key);
        self.hashes.insert(key, hash);
        self.views.insert(key, view);
        Ok(hash)
    }

    fn state_view(&mut self, decision: &Decision) -> Result<Value, HashError> {
        let option = |selected: OptionId| option_position(decision, selected);
        let kind = decision.state.kind().as_str();
        Ok(match &decision.state {
            DecisionState::Proposed => json!({ "kind": kind }),
            DecisionState::Accepted {
                selected_option,
                outcome,
            } => json!({
                "kind": kind,
                "selectedOption": option(*selected_option)?,
                "outcome": outcome,
            }),
            DecisionState::Rejected { rationale } => {
                json!({ "kind": kind, "rationale": rationale })
            }
            DecisionState::Superseded {
                selected_option,
                outcome,
                successor,
            } => json!({
                "kind": kind,
                "selectedOption": option(*selected_option)?,
                "outcome": outcome,
                "successor": self.hash(*successor)?,
            }),
            DecisionState::Retired {
                selected_option,
                outcome,
                rationale,
            } => json!({
                "kind": kind,
                "selectedOption": option(*selected_option)?,
                "outcome": outcome,
                "rationale": rationale,
            }),
        })
    }
}

/// The decision view: title, context, state, drivers, options, assessments and consequences, in
/// that order (AC-610). Options and drivers keep their authored order; an assessment names its
/// option and driver by position and the assessments are sorted by those positions.
fn decision_view(
    decision: &Decision,
    state: &Value,
    nodes: &NodeIndex,
) -> Result<Value, HashError> {
    let node = |subject: NodeRef| match subject {
        NodeRef::Held(node) => super::node_hash(nodes, node).map(|hash| json!(hash)).ok_or(
            HashError::UnknownDecisionReference {
                decision: decision.id,
                reference: node.key(),
            },
        ),
        NodeRef::Former(label) => Ok(super::former(label)),
    };
    let drivers = decision
        .drivers
        .iter()
        .map(|driver| (driver.payload.kind().as_str(), &driver.payload))
        .map(|(kind, payload)| match payload {
            DriverPayload::Local { label, statement } => {
                Ok(json!({ "kind": kind, "label": label, "statement": statement }))
            }
            DriverPayload::Subject { subject } => {
                Ok(json!({ "kind": kind, "subject": node(*subject)? }))
            }
        })
        .collect::<Result<Vec<_>, HashError>>()?;
    let options: Vec<[&str; 2]> = decision
        .options
        .iter()
        .map(|option| [option.title.as_str(), option.description.as_str()])
        .collect();
    let consequences = decision
        .consequences
        .iter()
        .map(|consequence| {
            let subject = consequence.subject.map(node).transpose()?;
            Ok(json!([
                consequence.kind.as_str(),
                consequence.statement,
                subject
            ]))
        })
        .collect::<Result<Vec<_>, HashError>>()?;
    Ok(json!([
        decision.title,
        decision.context,
        state,
        drivers,
        options,
        assessment_views(decision)?,
        consequences,
    ]))
}

fn assessment_views(decision: &Decision) -> Result<Vec<Value>, HashError> {
    let mut rows = decision
        .assessments
        .iter()
        .map(|assessment| {
            let option = option_position(decision, assessment.option)?;
            let driver = driver_position(decision, assessment.driver)?;
            Ok((
                option,
                driver,
                assessment.rating.as_str(),
                assessment.rationale.as_str(),
            ))
        })
        .collect::<Result<Vec<_>, HashError>>()?;
    rows.sort_unstable();
    Ok(rows
        .into_iter()
        .map(|(option, driver, rating, rationale)| json!([option, driver, rating, rationale]))
        .collect())
}

fn option_position(decision: &Decision, option: OptionId) -> Result<usize, HashError> {
    let keys = decision.options.iter().map(|option| option.id.key());
    position(decision, keys, option.key())
}

fn driver_position(decision: &Decision, driver: DriverId) -> Result<usize, HashError> {
    let keys = decision.drivers.iter().map(|driver| driver.id.key());
    position(decision, keys, driver.key())
}

/// The position of `key` among a decision's element keys.
fn position(
    decision: &Decision,
    mut keys: impl Iterator<Item = Key>,
    key: Key,
) -> Result<usize, HashError> {
    keys.position(|candidate| candidate == key)
        .ok_or(HashError::UnknownDecisionReference {
            decision: decision.id,
            reference: key,
        })
}
