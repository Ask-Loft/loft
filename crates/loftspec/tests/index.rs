//! The document index: every key, the element that carries it, each node's place, and every
//! integrity error found at once.

mod common;

use common::{component, key, label, root, small_document, small_document_with};
use loftspec::decision::{Decision, DecisionState};
use loftspec::document::Edge;
use loftspec::ids::{
    ComponentId, CriterionId, DecisionId, ElementId, FeatureGroupId, FeatureId, NodeId, OptionId,
    ProductId, RequirementId,
};
use loftspec::index::{Index, IntegrityError, SupersessionCycle};

fn superseded_by(decision: &Decision, successor: u32) -> Decision {
    let mut decision = decision.clone();
    decision.state = DecisionState::Superseded {
        selected_option: OptionId::new(key(23)),
        outcome: "Superseded.".to_owned(),
        successor: DecisionId::new(key(successor)),
    };
    decision
}

/// The small document's decision with its own keys (20 to 29) moved up by `offset`, superseded
/// by `successor`.
fn copy(offset: u32, successor: u32) -> Decision {
    let k = |n: u32| {
        if (20..30).contains(&n) {
            key(n + offset)
        } else {
            key(n)
        }
    };
    let mut decision = small_document_with(&k).decisions.remove(0);
    decision.state = DecisionState::Superseded {
        selected_option: OptionId::new(k(23)),
        outcome: "Superseded.".to_owned(),
        successor: DecisionId::new(key(successor)),
    };
    decision
}

#[test]
fn a_clean_document_indexes_every_element_and_its_place() {
    let document = small_document();
    let index = document.index().unwrap().index().clone();
    let feature = NodeId::from(FeatureId::new(key(3)));
    assert_eq!(index.element(key(3)), Some(ElementId::Node(feature)));
    assert_eq!(
        index.element(key(20)),
        Some(ElementId::Decision(DecisionId::new(key(20))))
    );
    assert_eq!(index.element(key(31)), None, "a tag entry is no element");
    let criterion = NodeId::from(CriterionId::new(key(5)));
    let above: Vec<NodeId> = index.ancestors(criterion).collect();
    assert_eq!(
        above,
        [
            RequirementId::new(key(4)).into(),
            feature,
            FeatureGroupId::new(key(2)).into(),
            ProductId::new(key(1)).into(),
        ]
    );
    assert_eq!(index.parent(ComponentId::new(key(6)).into()), None);
}

#[test]
fn every_integrity_error_is_reported_not_only_the_first() {
    let mut document = small_document();
    document.components.push(root(component(key(6), "Another")));
    document.edges.push(Edge::Realizes {
        feature: FeatureId::new(key(3)),
        component: ComponentId::new(key(77)),
    });
    document
        .bindings
        .bind(RequirementId::new(key(78)), label("RQ-78"))
        .unwrap();
    let (_, errors, _) = Index::build(&document);
    assert_eq!(
        errors,
        [
            IntegrityError::DuplicateKey(key(6)),
            IntegrityError::UnknownEdgeEndpoint(ComponentId::new(key(77)).into()),
            IntegrityError::UnknownBindingElement(RequirementId::new(key(78)).into()),
        ]
    );
}

#[test]
fn a_key_carried_by_a_decision_element_and_a_node_is_carried_twice() {
    let mut document = small_document();
    document.decisions[0].options[1].id = OptionId::new(key(3));
    let errors = document.index().unwrap_err().integrity;
    assert!(
        errors.contains(&IntegrityError::DuplicateKey(key(3))),
        "{errors:?}"
    );
}

#[test]
fn a_successor_the_document_does_not_hold_is_unknown() {
    let mut document = small_document();
    document.decisions[0] = superseded_by(&document.decisions[0], 90);
    assert_eq!(
        document.index().unwrap_err().integrity,
        [IntegrityError::UnknownSuccessor {
            decision: DecisionId::new(key(20)),
            successor: DecisionId::new(key(90)),
        }]
    );
}

#[test]
fn a_decision_that_supersedes_itself_is_reported_once() {
    let mut document = small_document();
    document.decisions[0] = superseded_by(&document.decisions[0], 20);
    let errors = document.index().unwrap_err();
    assert_eq!(
        errors.integrity,
        [IntegrityError::SupersedesItself(DecisionId::new(key(20)))]
    );
    assert_eq!(errors.cycles, Vec::new());
}

#[test]
fn a_ring_of_successors_is_one_cycle_naming_every_member() {
    let mut document = small_document();
    document.decisions = vec![copy(0, 120), copy(100, 220), copy(200, 20)];
    let errors = document.index().unwrap_err();
    assert!(errors.integrity.is_empty(), "{:?}", errors.integrity);
    assert_eq!(
        errors.cycles,
        [SupersessionCycle(vec![
            DecisionId::new(key(20)),
            DecisionId::new(key(120)),
            DecisionId::new(key(220)),
        ])]
    );
}
