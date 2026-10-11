//! Decision views (RQ-62): every reference inside a decision is by position or by content hash,
//! never by key, so fresh element keys leave the view unchanged (AC-219, AC-220).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::HashError;
use crate::canonical::{Digest, Domain, digest};
use crate::decision::{Decision, DecisionState, DriverPayload};
use crate::identity::Key;

/// Hashes every decision once, following supersession to its successor.
pub(crate) struct DecisionHasher<'a> {
    decisions: BTreeMap<Key, &'a Decision>,
    nodes: &'a BTreeMap<Key, Digest>,
    hashes: BTreeMap<Key, Digest>,
    views: BTreeMap<Key, Value>,
    in_progress: BTreeSet<Key>,
}

/// The view and its hash for every decision of a document.
pub(crate) type DecisionViews = (BTreeMap<Key, Digest>, BTreeMap<Key, Value>);

impl<'a> DecisionHasher<'a> {
    pub(crate) fn new(
        decisions: &'a [Decision],
        nodes: &'a BTreeMap<Key, Digest>,
    ) -> Result<Self, HashError> {
        let mut by_key = BTreeMap::new();
        for decision in decisions {
            if nodes.contains_key(&decision.key) || by_key.insert(decision.key, decision).is_some()
            {
                return Err(HashError::DuplicateKey(decision.key));
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
        let keys: Vec<Key> = self.decisions.keys().copied().collect();
        for key in keys {
            self.hash(key)?;
        }
        Ok((self.hashes, self.views))
    }

    fn hash(&mut self, key: Key) -> Result<Digest, HashError> {
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
        let hash = digest(Domain::Decisions, &view);
        self.in_progress.remove(&key);
        self.hashes.insert(key, hash);
        self.views.insert(key, view);
        Ok(hash)
    }

    fn state_view(&mut self, decision: &Decision) -> Result<Value, HashError> {
        let option = |selected: Key| position(decision, &decision.options, selected, |o| o.key);
        Ok(match &decision.state {
            DecisionState::Proposed => json!({ "kind": "proposed" }),
            DecisionState::Accepted {
                selected_option,
                outcome,
            } => json!({
                "kind": "accepted",
                "selectedOption": option(*selected_option)?,
                "outcome": outcome,
            }),
            DecisionState::Rejected { rationale } => {
                json!({ "kind": "rejected", "rationale": rationale })
            }
            DecisionState::Superseded {
                selected_option,
                outcome,
                successor,
            } => json!({
                "kind": "superseded",
                "selectedOption": option(*selected_option)?,
                "outcome": outcome,
                "successor": self.hash(*successor)?,
            }),
            DecisionState::Retired {
                selected_option,
                outcome,
                rationale,
            } => json!({
                "kind": "retired",
                "selectedOption": option(*selected_option)?,
                "outcome": outcome,
                "rationale": rationale,
            }),
        })
    }
}

/// The decision view: title, context, state, drivers, options, assessments and consequences, in
/// that order (AC-218). Options and drivers keep their authored order; an assessment names its
/// option and driver by position and the assessments are sorted by those positions.
fn decision_view(
    decision: &Decision,
    state: &Value,
    nodes: &BTreeMap<Key, Digest>,
) -> Result<Value, HashError> {
    let node = |subject: Key| {
        nodes
            .get(&subject)
            .copied()
            .ok_or(HashError::UnknownDecisionReference {
                decision: decision.key,
                reference: subject,
            })
    };
    let drivers = decision
        .drivers
        .iter()
        .map(|driver| match &driver.payload {
            DriverPayload::Local { label, statement } => {
                Ok(json!({ "kind": "local", "label": label, "statement": statement }))
            }
            DriverPayload::Subject { subject } => {
                Ok(json!({ "kind": "subject", "subject": node(*subject)? }))
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
            let option = position(decision, &decision.options, assessment.option, |o| o.key)?;
            let driver = position(decision, &decision.drivers, assessment.driver, |d| d.key)?;
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

fn position<T>(
    decision: &Decision,
    elements: &[T],
    key: Key,
    key_of: impl Fn(&T) -> Key,
) -> Result<usize, HashError> {
    elements
        .iter()
        .position(|element| key_of(element) == key)
        .ok_or(HashError::UnknownDecisionReference {
            decision: decision.key,
            reference: key,
        })
}
