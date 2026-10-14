//! The validator (F-18, F-48, RQ-70): every error a typed document can hold, each naming
//! the elements it concerns.

use crate::common::decisions::{DECISION, add_decision, decision_id, set_state, superseded};
use crate::common::{component, key, label, root, small_document};
use loftspec::bindings::BindingError;
use loftspec::decision::{DecisionState, NodeRef};
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::identity::Label;
use loftspec::ids::{ComponentId, ElementId, FeatureId, OptionId, ProductId, RequirementId};
use loftspec::index::IntegrityError;
use loftspec::names::TagName;
use loftspec::validate::{ValidationError, validate};

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
