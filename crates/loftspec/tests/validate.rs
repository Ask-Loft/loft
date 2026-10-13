//! The validator (D-11 F-18, F-48, RQ-70): every error a typed document can hold, each naming
//! the elements it concerns.

mod common;

use common::{component, key, label, root, small_document, small_document_with};
use loftspec::bindings::BindingError;
use loftspec::decision::{Decision, DecisionState, NodeRef};
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::identity::Label;
use loftspec::ids::{
    ComponentId, DecisionId, DriverId, ElementId, FeatureId, OptionId, ProductId, RequirementId,
};
use loftspec::index::{IntegrityError, SupersessionCycle};
use loftspec::names::TagName;
use loftspec::validate::{ValidationError, validate};

const DECISION: u32 = 20;

fn decision_id() -> DecisionId {
    DecisionId::new(key(DECISION))
}

fn decision(document: &mut Document) -> &mut Decision {
    &mut document.decisions[0]
}

fn set_state(document: &mut Document, state: DecisionState) {
    decision(document).state = state;
}

/// Superseded by `successor`, for the decision whose own keys are moved up by `offset`.
fn superseded(offset: u32, successor: u32) -> DecisionState {
    DecisionState::Superseded {
        selected_option: OptionId::new(key(23 + offset)),
        outcome: "Replaced.".to_owned(),
        successor: DecisionId::new(key(successor)),
    }
}

fn retired() -> DecisionState {
    DecisionState::Retired {
        selected_option: OptionId::new(key(23)),
        outcome: "Kept for a while.".to_owned(),
        rationale: "No longer needed.".to_owned(),
    }
}

fn affected_former(document: &mut Document, former: &str) -> Label {
    let former = label(former);
    document.edges.push(Edge::Subject {
        decision: decision_id(),
        subject: NodeRef::Former(former),
        role: SubjectRole::Affected,
    });
    former
}

/// Adds the small document's decision with its own keys moved up by `offset`, so that it can sit
/// beside the original, in `state`, with its `outcome` subject.
fn add_decision(document: &mut Document, offset: u32, state: DecisionState) {
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

#[test]
fn a_valid_document_raises_no_error() {
    assert_eq!(validate(&small_document()), []);
}

#[test]
fn an_edge_whose_source_no_element_carries_is_an_error_naming_the_key() {
    let mut document = small_document();
    document.edges.push(Edge::Realizes {
        feature: FeatureId::new(key(77)),
        component: ComponentId::new(key(6)),
    });
    let errors = validate(&document);
    let unknown = ElementId::from(FeatureId::new(key(77)));
    assert_eq!(
        errors,
        [ValidationError::Integrity(
            IntegrityError::UnknownEdgeEndpoint(unknown)
        )]
    );
    assert_eq!(errors[0].keys(), [key(77)]);
}

#[test]
fn an_edge_whose_target_no_element_carries_is_an_error_naming_the_key() {
    let mut document = small_document();
    document.edges.push(Edge::Mentions {
        source: FeatureId::new(key(3)).into(),
        target: RequirementId::new(key(78)).into(),
    });
    assert_eq!(
        validate(&document),
        [ValidationError::Integrity(
            IntegrityError::UnknownEdgeEndpoint(RequirementId::new(key(78)).into())
        )]
    );
}

#[test]
fn a_subject_edge_to_a_former_label_raises_no_unresolved_endpoint_error() {
    let mut document = small_document();
    set_state(&mut document, retired());
    affected_former(&mut document, "RQ-90");
    assert_eq!(validate(&document), []);
}

#[test]
/// A v1 export may scope a nested Component, and the conversion keeps that scope as an edge, so a
/// `scoped_to` edge may start at any Component.
fn a_scoped_to_edge_from_a_nested_component_is_accepted() {
    let mut document = small_document();
    document.components[0]
        .components
        .push(component(key(40), "Nested"));
    document.edges.push(Edge::ScopedTo {
        component: ComponentId::new(key(40)),
        product: ProductId::new(key(1)),
    });
    assert_eq!(validate(&document), []);
}

#[test]
fn a_binding_whose_key_no_element_carries_is_an_error_naming_the_key() {
    let mut document = small_document();
    document
        .bindings
        .bind(RequirementId::new(key(79)), label("RQ-79"))
        .unwrap();
    let errors = validate(&document);
    assert_eq!(
        errors,
        [ValidationError::Integrity(
            IntegrityError::UnknownBindingElement(RequirementId::new(key(79)).into())
        )]
    );
    assert_eq!(errors[0].keys(), [key(79)]);
}

#[test]
fn a_tag_listed_twice_is_an_error_naming_the_node_and_the_tag() {
    let mut document = small_document();
    let tags = &mut document.products[0].details.tags;
    tags.push(tags[0].clone());
    tags.push(tags[0].clone());
    let errors = validate(&document);
    assert_eq!(
        errors,
        [ValidationError::RepeatedTag {
            node: ProductId::new(key(1)).into(),
            tag: TagName::parse("portable").unwrap(),
        }]
    );
    assert_eq!(
        errors[0].to_string(),
        "product 00000000-0000-7000-8000-000000000001 lists the tag `portable` twice"
    );
}

#[test]
fn two_keys_with_one_live_label_are_an_error_naming_both_and_the_label() {
    let mut document = small_document();
    document.components.push(root(component(key(41), "Twin")));
    document
        .bindings
        .bind(ComponentId::new(key(41)), label("CMP-1"))
        .unwrap();
    let errors = validate(&document);
    assert_eq!(
        errors,
        [ValidationError::Binding(BindingError::LabelUnderTwoKeys {
            label: label("CMP-1"),
            first: ComponentId::new(key(6)).into(),
            second: ComponentId::new(key(41)).into(),
        })]
    );
    assert_eq!(errors[0].keys(), [key(6), key(41)]);
}

#[test]
fn a_former_label_in_a_proposed_decision_is_an_error() {
    let mut document = small_document();
    set_state(&mut document, DecisionState::Proposed);
    let former = affected_former(&mut document, "RQ-90");
    assert_eq!(
        validate(&document),
        [ValidationError::FormerLabelInLiveDecision {
            decision: decision_id(),
            state: "proposed",
            label: former,
        }]
    );
}

#[test]
fn a_former_label_in_an_accepted_decision_is_an_error() {
    let mut document = small_document();
    let former = affected_former(&mut document, "RQ-90");
    let error = ValidationError::FormerLabelInLiveDecision {
        decision: decision_id(),
        state: "accepted",
        label: former,
    };
    assert_eq!(validate(&document), std::slice::from_ref(&error));
    assert_eq!(
        error.to_string(),
        "decision 00000000-0000-7000-8000-000000000014 is accepted and names the former label \
         RQ-90"
    );
}

#[test]
fn a_former_label_in_a_rejected_decision_is_an_error() {
    let mut document = small_document();
    set_state(
        &mut document,
        DecisionState::Rejected {
            rationale: "Not now.".to_owned(),
        },
    );
    let former = affected_former(&mut document, "RQ-90");
    assert_eq!(
        validate(&document),
        [ValidationError::FormerLabelInLiveDecision {
            decision: decision_id(),
            state: "rejected",
            label: former,
        }]
    );
}

#[test]
fn a_former_label_in_a_retired_or_superseded_decision_raises_no_such_error() {
    let mut document = small_document();
    set_state(&mut document, retired());
    affected_former(&mut document, "RQ-90");
    assert_eq!(validate(&document), []);
    add_decision(&mut document, 100, DecisionState::Proposed);
    set_state(&mut document, superseded(0, DECISION + 100));
    assert_eq!(validate(&document), []);
}

#[test]
fn a_former_label_equal_to_a_live_label_is_an_error() {
    let mut document = small_document();
    set_state(&mut document, retired());
    let former = affected_former(&mut document, "CMP-1");
    assert_eq!(
        validate(&document),
        [ValidationError::FormerLabelInUse {
            label: former,
            element: ComponentId::new(key(6)).into(),
        }]
    );
}

#[test]
fn a_former_label_equal_to_an_alias_is_an_error() {
    let mut document = small_document();
    set_state(&mut document, retired());
    let former = affected_former(&mut document, "RQ-1");
    assert_eq!(
        validate(&document),
        [ValidationError::FormerLabelInUse {
            label: former,
            element: RequirementId::new(key(4)).into(),
        }]
    );
}

#[test]
fn a_former_label_no_element_carries_raises_no_such_error() {
    let mut document = small_document();
    set_state(&mut document, retired());
    affected_former(&mut document, "RQ-90");
    assert_eq!(validate(&document), []);
}

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
