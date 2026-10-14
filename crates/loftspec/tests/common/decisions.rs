//! The small document's decision, and the edits of it that the validator's tests share.

use super::{key, small_document_with};
use loftspec::decision::{Decision, DecisionState};
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::ids::{DecisionId, OptionId, RequirementId};

pub const DECISION: u32 = 20;

pub fn decision_id() -> DecisionId {
    DecisionId::new(key(DECISION))
}

pub fn decision(document: &mut Document) -> &mut Decision {
    &mut document.decisions[0]
}

pub fn set_state(document: &mut Document, state: DecisionState) {
    decision(document).state = state;
}

/// Superseded by `successor`, for the decision whose own keys are moved up by `offset`.
pub fn superseded(offset: u32, successor: u32) -> DecisionState {
    DecisionState::Superseded {
        selected_option: OptionId::new(key(23 + offset)),
        outcome: "Replaced.".to_owned(),
        successor: DecisionId::new(key(successor)),
    }
}

/// Adds the small document's decision with its own keys moved up by `offset`, so that it can sit
/// beside the original, in `state`, with its `outcome` subject.
pub fn add_decision(document: &mut Document, offset: u32, state: DecisionState) {
    let own = |n: u32| {
        if (20..30).contains(&n) {
            key(n + offset)
        } else {
            key(n)
        }
    };
    let mut decision = small_document_with(&own).decisions.remove(0);
    decision.state = state;
    document.edges.push(Edge::Subject {
        decision: decision.id,
        subject: RequirementId::new(key(4)).into(),
        role: SubjectRole::Outcome,
    });
    document.decisions.push(decision);
}
