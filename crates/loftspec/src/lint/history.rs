//! The rules of decision history (F-21): an edit to the meaning of a decision published
//! resolved (`V2-010`), and a change of state that the published state does not allow, deleting
//! a resolved decision among them (`V2-011`). Both judge from what the last publish recorded in
//! each decision's binding in `spec/labels.yaml`, which is part of the tree, so they read no
//! history of a repository. A decision is changed by a new decision that supersedes it.

use std::collections::BTreeMap;

use super::context::Context;
use super::{Raw, RuleId};
use crate::bindings::Binding;
use crate::canonical::Digest;
use crate::decision::{Decision, DecisionState, StateKind};
use crate::hash::decision_history_hash;
use crate::ids::{DecisionId, ElementId};

pub(super) fn check(context: &Context<'_>, out: &mut Vec<Raw>) {
    let document = context.document;
    let decisions: BTreeMap<DecisionId, &Decision> = document
        .decisions
        .iter()
        .map(|decision| (decision.id, decision))
        .collect();
    for binding in document.bindings.iter() {
        let (ElementId::Decision(id), Some(recorded)) = (binding.element, binding.published_state)
        else {
            continue;
        };
        if binding.departed {
            out.extend(departed(binding, recorded));
        } else if let Some(decision) = decisions.get(&id) {
            out.extend(edited(decision, recorded, binding.published_history));
            out.extend(moved(decision, recorded));
        }
    }
}

/// `V2-010` for a decision whose meaning has changed since the publish that recorded it in
/// `recorded`, with the history hash `history`: its history hash now, or, for a decision
/// recorded as accepted and since superseded or retired, the hash of its view taken back to
/// accepted.
fn edited(decision: &Decision, recorded: StateKind, history: Option<Digest>) -> Option<Raw> {
    let history = history?;
    let now = decision.state.kind();
    let hash = if now == recorded {
        decision_history_hash(decision)
    } else if recorded == StateKind::Accepted {
        decision_history_hash(&as_accepted(decision)?)
    } else {
        return None;
    };
    (hash != history).then(|| {
        let message = format!(
            "The decision was published as `{}` and its meaning has changed since; record \
             the change in a new decision that supersedes it.",
            recorded.as_str()
        );
        Raw::new(RuleId::V2010, vec![decision.id.key()], message)
    })
}

/// A superseded or retired decision with its state taken back to accepted, under the same
/// selected option and outcome.
fn as_accepted(decision: &Decision) -> Option<Decision> {
    let (DecisionState::Superseded {
        selected_option,
        outcome,
        ..
    }
    | DecisionState::Retired {
        selected_option,
        outcome,
        ..
    }) = &decision.state
    else {
        return None;
    };
    let mut accepted = decision.clone();
    accepted.state = DecisionState::Accepted {
        selected_option: *selected_option,
        outcome: outcome.clone(),
    };
    Some(accepted)
}

/// `V2-011` for a decision whose state has changed since the publish that recorded it in
/// `recorded` in a way that state does not allow: a proposal may be accepted or rejected, and an
/// accepted decision superseded or retired; nothing else moves.
fn moved(decision: &Decision, recorded: StateKind) -> Option<Raw> {
    use StateKind::{Accepted, Proposed, Rejected, Retired, Superseded};
    let now = decision.state.kind();
    let allowed = now == recorded
        || matches!(
            (recorded, now),
            (Proposed, Accepted | Rejected) | (Accepted, Superseded | Retired)
        );
    (!allowed).then(|| {
        let message = format!(
            "The decision was published as `{}` and is now `{}`, a change that its \
             published state does not allow.",
            recorded.as_str(),
            now.as_str()
        );
        Raw::new(RuleId::V2011, vec![decision.id.key()], message)
    })
}

/// `V2-011` for a decision that left the tree after a publish recorded it resolved.
fn departed(binding: &Binding, recorded: StateKind) -> Option<Raw> {
    (recorded != StateKind::Proposed).then(|| {
        let message = format!(
            "The decision {} was published as `{}` and has left the tree; retiring a \
             decision is the way to end it.",
            binding.label,
            recorded.as_str()
        );
        Raw::new(RuleId::V2011, vec![binding.element.key()], message)
    })
}
