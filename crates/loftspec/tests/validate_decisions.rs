//! The validator on decisions (F-48, RQ-70): a decision's options, assessments and successors
//! are its own, a supersession never forms a ring, and a resolved or accepted decision is
//! complete.

use crate::common::decisions::{
    DECISION, add_decision, decision, decision_id, set_state, superseded,
};
use crate::common::{key, small_document};
use loftspec::decision::DecisionState;
use loftspec::document::{Edge, SubjectRole};
use loftspec::ids::{DecisionId, DriverId, OptionId};
use loftspec::index::{IntegrityError, SupersessionCycle};
use loftspec::validate::{ValidationError, validate};

#[test]
fn a_selected_option_of_another_decision_is_an_error_naming_the_decision() {
    let mut document = small_document();
    set_state(
        &mut document,
        DecisionState::Accepted {
            selected_option: OptionId::new(key(80)),
            outcome: "Chosen.".to_owned(),
        },
    );
    assert_eq!(
        validate(&document),
        [ValidationError::Integrity(
            IntegrityError::UnknownDecisionReference {
                decision: decision_id(),
                reference: key(80),
            }
        )]
    );
}

#[test]
fn an_assessment_of_another_option_is_an_error_naming_the_decision_and_the_option() {
    let mut document = small_document();
    decision(&mut document).assessments[0].option = OptionId::new(key(81));
    let errors = validate(&document);
    assert_eq!(
        errors[0],
        ValidationError::Integrity(IntegrityError::UnknownDecisionReference {
            decision: decision_id(),
            reference: key(81),
        })
    );
    assert!(
        errors[0].to_string().contains("000000000051"),
        "{}",
        errors[0]
    );
}

#[test]
fn an_assessment_of_another_driver_is_an_error_naming_the_decision_and_the_driver() {
    let mut document = small_document();
    decision(&mut document).assessments[0].driver = DriverId::new(key(82));
    assert_eq!(
        validate(&document)[0],
        ValidationError::Integrity(IntegrityError::UnknownDecisionReference {
            decision: decision_id(),
            reference: key(82),
        })
    );
}

#[test]
fn a_successor_no_decision_carries_is_an_error_naming_the_decision_and_the_key() {
    let mut document = small_document();
    set_state(&mut document, superseded(0, 90));
    let errors = validate(&document);
    assert_eq!(
        errors,
        [ValidationError::Integrity(
            IntegrityError::UnknownSuccessor {
                decision: decision_id(),
                successor: DecisionId::new(key(90)),
            }
        )]
    );
    assert!(
        errors[0].to_string().contains("00000000005a"),
        "{}",
        errors[0]
    );
}

#[test]
fn a_decision_that_names_itself_as_its_successor_is_an_error() {
    let mut document = small_document();
    set_state(&mut document, superseded(0, DECISION));
    assert_eq!(
        validate(&document),
        [ValidationError::Integrity(
            IntegrityError::SupersedesItself(decision_id())
        )]
    );
}

#[test]
fn three_decisions_superseding_one_another_in_a_ring_yield_one_error_naming_all_three() {
    let mut document = small_document();
    set_state(&mut document, superseded(0, DECISION + 100));
    add_decision(&mut document, 100, superseded(100, DECISION + 200));
    add_decision(&mut document, 200, superseded(200, DECISION));
    let errors = validate(&document);
    let ring = vec![
        decision_id(),
        DecisionId::new(key(DECISION + 100)),
        DecisionId::new(key(DECISION + 200)),
    ];
    assert_eq!(errors, [ValidationError::Cycle(SupersessionCycle(ring))]);
    assert_eq!(errors[0].keys().len(), 3);
}

#[test]
fn a_chain_of_successors_that_ends_at_a_live_decision_raises_no_such_error() {
    let mut document = small_document();
    set_state(&mut document, superseded(0, DECISION + 100));
    add_decision(&mut document, 100, superseded(100, DECISION + 200));
    add_decision(
        &mut document,
        200,
        DecisionState::Accepted {
            selected_option: OptionId::new(key(DECISION + 203)),
            outcome: "Current.".to_owned(),
        },
    );
    assert_eq!(validate(&document), []);
}

#[test]
fn a_resolved_decision_with_fewer_than_two_options_is_an_error() {
    let mut document = small_document();
    let decision = decision(&mut document);
    decision.options.truncate(1);
    decision
        .assessments
        .retain(|cell| cell.option == OptionId::new(key(23)));
    assert_eq!(
        validate(&document),
        [ValidationError::TooFewOptions(decision_id())]
    );
}

#[test]
fn a_resolved_decision_without_a_driver_is_an_error() {
    let mut document = small_document();
    let decision = decision(&mut document);
    decision.drivers.clear();
    decision.assessments.clear();
    assert_eq!(
        validate(&document),
        [ValidationError::NoDriver(decision_id())]
    );
}

#[test]
fn a_missing_assessment_is_an_error_naming_the_decision_the_option_and_the_driver() {
    let mut document = small_document();
    decision(&mut document).assessments.remove(0);
    assert_eq!(
        validate(&document),
        [ValidationError::MissingAssessment {
            decision: decision_id(),
            option: OptionId::new(key(24)),
            driver: DriverId::new(key(22)),
        }]
    );
}

#[test]
fn a_proposed_decision_raises_no_completeness_error() {
    let mut document = small_document();
    set_state(&mut document, DecisionState::Proposed);
    let decision = decision(&mut document);
    decision.options.clear();
    decision.drivers.clear();
    decision.assessments.clear();
    decision.consequences.clear();
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Subject { .. }));
    assert_eq!(validate(&document), []);
}

#[test]
fn an_accepted_decision_without_a_consequence_is_an_error() {
    let mut document = small_document();
    decision(&mut document).consequences.clear();
    assert_eq!(
        validate(&document),
        [ValidationError::NoConsequence(decision_id())]
    );
}

#[test]
fn an_accepted_decision_without_an_outcome_subject_is_an_error() {
    let mut document = small_document();
    document.edges.retain(|edge| {
        !matches!(
            edge,
            Edge::Subject {
                role: SubjectRole::Outcome,
                ..
            }
        )
    });
    assert_eq!(
        validate(&document),
        [ValidationError::NoOutcomeSubject(decision_id())]
    );
}

#[test]
fn a_rejected_decision_needs_no_consequence_or_outcome_subject() {
    let mut document = small_document();
    set_state(
        &mut document,
        DecisionState::Rejected {
            rationale: "Not now.".to_owned(),
        },
    );
    decision(&mut document).consequences.clear();
    document.edges.retain(|edge| {
        !matches!(
            edge,
            Edge::Subject {
                role: SubjectRole::Outcome,
                ..
            }
        )
    });
    assert_eq!(validate(&document), []);
}

#[test]
fn every_error_is_reported_not_only_the_first() {
    let mut document = small_document();
    decision(&mut document).consequences.clear();
    decision(&mut document).assessments.remove(0);
    let tags = &mut document.products[0].details.tags;
    tags.push(tags[0].clone());
    assert_eq!(validate(&document).len(), 3);
}
