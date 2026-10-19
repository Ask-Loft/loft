//! The review packet (the `loft review` Feature), over the small document: P-1 (1) > FG-1 (2) >
//! F-1 (3) > RQ-2 (4) > criterion 5, with CON-2 (9) under FG-1 and QA-2 (10) under F-1; root
//! Component CMP-1 (6) with CON-1 (7) and QA-1 (8), realized by F-1; ADR-1 (20) names RQ-2 as
//! its outcome, through a driver and a consequence, and CMP-1 as affected.

use crate::common::{component, criterion, key, small_document};
use loftspec::decision::DecisionState;
use loftspec::document::{Document, Edge, TermEntry};
use loftspec::identity::Key;
use loftspec::ids::{
    ComponentId, ConstraintId, CriterionId, DecisionId, FeatureId, NodeId, TermId,
};
use loftspec::kind::NodeKind;
use loftspec::review::{Change, Packet, Unit, Via, pack};
use loftspec::tree::{Constraint, Details};

fn keys(facts: impl IntoIterator<Item = NodeId>) -> Vec<Key> {
    facts.into_iter().map(NodeId::key).collect()
}

fn ns(numbers: &[u32]) -> Vec<Key> {
    numbers.iter().map(|n| key(*n)).collect()
}

fn packed(document: &Document, base: Option<&Document>, node: Option<NodeId>) -> Packet {
    pack(document, base, node).unwrap()
}

/// The scope of each unit, and the assertions of each.
fn outline(packet: &Packet) -> Vec<(Key, Vec<Key>)> {
    packet
        .units
        .iter()
        .map(|unit| {
            let assertions = keys(unit.assertions.iter().map(|assertion| assertion.id));
            (unit.scope.node.id.key(), assertions)
        })
        .collect()
}

fn unit(packet: &Packet, scope: u32) -> &Unit {
    packet
        .units
        .iter()
        .find(|unit| unit.scope.node.id.key() == key(scope))
        .unwrap()
}

fn criterion_mut(document: &mut Document) -> &mut loftspec::tree::Criterion {
    &mut document.products[0].feature_groups[0].features[0].requirements[0].criteria[0]
}

#[test]
fn without_a_base_every_assertion_lands_in_the_unit_of_its_scope_in_document_order() {
    let packet = packed(&small_document(), None, None);
    assert_eq!(
        outline(&packet),
        [
            (key(2), ns(&[9])),
            (key(3), ns(&[4, 5, 10])),
            (key(6), ns(&[7, 8])),
        ]
    );
    let changes: Vec<_> = packet
        .units
        .iter()
        .flat_map(|unit| &unit.assertions)
        .map(|assertion| assertion.change)
        .collect();
    assert!(changes.iter().all(Option::is_none), "{changes:?}");
}

#[test]
fn a_feature_s_unit_gives_its_ancestors_requirements_qualities_and_realized_components() {
    let packet = packed(&small_document(), None, None);
    let feature = unit(&packet, 3);
    let ancestors: Vec<_> = feature
        .ancestors
        .iter()
        .map(|holder| {
            (
                holder.node.id.key(),
                keys(holder.constraints.iter().map(|c| c.id)),
            )
        })
        .collect();
    assert_eq!(ancestors, [(key(1), vec![]), (key(2), ns(&[9]))]);
    assert_eq!(
        keys(feature.scope.quality_attributes.iter().map(|q| q.id)),
        ns(&[10])
    );
    let requirements: Vec<_> = feature
        .requirements
        .iter()
        .map(|each| {
            (
                each.requirement.id.key(),
                keys(each.criteria.iter().map(|c| c.id)),
            )
        })
        .collect();
    assert_eq!(requirements, [(key(4), ns(&[5]))]);
    let realized = &feature.realizes[0];
    assert_eq!(realized.component.node.id.key(), key(6));
    assert_eq!(
        keys(realized.component.constraints.iter().map(|c| c.id)),
        ns(&[7])
    );
    assert_eq!(
        keys(realized.component.quality_attributes.iter().map(|q| q.id)),
        ns(&[8])
    );
    assert_eq!(realized.above, []);
}

#[test]
fn a_realized_component_comes_with_the_components_above_it() {
    let mut document = small_document();
    let mut part = component(key(40), "Key minting");
    part.constraints.push(Constraint {
        id: Some(ConstraintId::new(key(41))),
        name: "Key minting shall use the system clock.".to_owned(),
        details: Details::default(),
        binding: None,
        conservation_kind: None,
        verification_fate: None,
    });
    document.components[0].component.components.push(part);
    document.edges.push(Edge::Realizes {
        feature: FeatureId::new(key(3)),
        component: ComponentId::new(key(40)),
    });
    let packet = packed(&document, None, None);
    let realized = &unit(&packet, 3).realizes;
    let part = realized
        .iter()
        .find(|each| each.component.node.id.key() == key(40))
        .unwrap();
    assert_eq!(
        keys(part.component.constraints.iter().map(|c| c.id)),
        ns(&[41])
    );
    let above: Vec<_> = part
        .above
        .iter()
        .map(|holder| {
            (
                holder.node.id.key(),
                keys(holder.constraints.iter().map(|c| c.id)),
            )
        })
        .collect();
    assert_eq!(above, [(key(6), ns(&[7]))]);
}

#[test]
fn each_node_carries_its_label_kind_name_and_own_content_view() {
    let packet = packed(&small_document(), None, None);
    let feature = unit(&packet, 3);
    let requirement = &feature.requirements[0].requirement;
    assert_eq!(
        requirement.label.map(|label| label.to_string()).as_deref(),
        Some("RQ-2")
    );
    assert_eq!(requirement.kind(), NodeKind::Requirement);
    assert_eq!(
        requirement.name,
        "The writer shall write a key on every node."
    );
    assert!(requirement.view.is_array(), "{}", requirement.view);
    assert_ne!(requirement.view, feature.requirements[0].criteria[0].view);
}

/// The links of the decision `decision` in `unit`, by node key and how each links.
fn links(unit: &Unit, decision: u32) -> Vec<(Key, Via)> {
    unit.decisions
        .iter()
        .find(|linked| linked.id.key() == key(decision))
        .unwrap()
        .links
        .iter()
        .map(|(node, via)| (node.key(), *via))
        .collect()
}

#[test]
fn a_decision_links_by_subject_driver_consequence_and_mention() {
    let mut document = small_document();
    document.edges.push(Edge::Mentions {
        source: DecisionId::new(key(20)).into(),
        target: loftspec::ids::QualityAttributeId::new(key(10)).into(),
    });
    let packet = packed(&document, None, None);
    assert_eq!(
        links(unit(&packet, 3), 20),
        [
            (key(4), Via::SubjectOutcome),
            (key(4), Via::Driver),
            (key(4), Via::Consequence),
            (key(10), Via::Mentions),
        ]
    );
    assert_eq!(
        links(unit(&packet, 6), 20),
        [(key(6), Via::SubjectAffected)]
    );
    assert_eq!(unit(&packet, 2).decisions, []);
}

#[test]
fn a_node_that_mentions_a_decision_links_it() {
    let mut document = small_document();
    document.edges.push(Edge::Mentions {
        source: ConstraintId::new(key(9)).into(),
        target: DecisionId::new(key(20)).into(),
    });
    let packet = packed(&document, None, None);
    assert_eq!(links(unit(&packet, 2), 20), [(key(9), Via::Mentions)]);
}

#[test]
fn a_decision_carries_its_label_title_state_and_outcome() {
    let packet = packed(&small_document(), None, None);
    let linked = &unit(&packet, 3).decisions[0];
    assert_eq!(
        linked.label.map(|label| label.to_string()).as_deref(),
        Some("ADR-1")
    );
    assert_eq!(linked.title, "Keys are UUIDs");
    assert_eq!(linked.state.as_str(), "accepted");
    assert_eq!(
        linked.outcome.as_deref(),
        Some("New keys are UUID version 7.")
    );
}

#[test]
fn decisions_follow_their_labels_then_their_keys() {
    let mut document = small_document();
    for n in [50, 49] {
        let mut decision = crate::common::proposed_decision(key(n), &format!("proposal-{n}"));
        decision.title = format!("Proposal {n}");
        document.decisions.push(decision);
        document.edges.push(Edge::Mentions {
            source: DecisionId::new(key(n)).into(),
            target: CriterionId::new(key(5)).into(),
        });
    }
    let packet = packed(&document, None, None);
    let order: Vec<Key> = unit(&packet, 3)
        .decisions
        .iter()
        .map(|linked| linked.id.key())
        .collect();
    assert_eq!(order, ns(&[20, 49, 50]));
    let proposed = &unit(&packet, 3).decisions[1];
    assert_eq!(proposed.state, loftspec::decision::StateKind::Proposed);
    assert_eq!(proposed.outcome, None);
    assert!(matches!(
        document.decisions[1].state,
        DecisionState::Proposed
    ));
}

fn term(n: u32, name: &str, aliases: &[&str]) -> TermEntry {
    TermEntry {
        id: TermId::new(key(n)),
        name: name.to_owned(),
        definition: format!("What {name} means."),
        aliases: aliases.iter().map(|alias| (*alias).to_owned()).collect(),
    }
}

#[test]
fn a_term_whose_name_or_alias_occurs_in_an_assertion_is_listed_by_name() {
    let mut document = small_document();
    document.terms = vec![
        term(60, "node", &[]),
        term(61, "label", &["tag"]),
        term(62, "entry", &["record"]),
    ];
    let packet = packed(&document, None, None);
    let names: Vec<&str> = unit(&packet, 3)
        .terms
        .iter()
        .map(|term| term.name.as_str())
        .collect();
    assert_eq!(names, ["entry", "node"]);
    let names: Vec<&str> = unit(&packet, 6)
        .terms
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(names, Vec::<&str>::new());
}

#[test]
fn with_a_base_only_a_changed_assertion_is_reviewed_beside_the_base_s_node() {
    let base = small_document();
    let mut document = small_document();
    criterion_mut(&mut document).name = "Every node record carries a key.".to_owned();
    let packet = packed(&document, Some(&base), None);
    assert_eq!(outline(&packet), [(key(3), ns(&[5]))]);
    let assertion = &unit(&packet, 3).assertions[0];
    assert_eq!(assertion.change, Some(Change::Changed));
    let before = assertion.before.as_ref().unwrap();
    assert_eq!(before.name, "Every node record carries a `key` field.");
    let requirement = &unit(&packet, 3).requirements[0];
    assert_eq!(requirement.requirement.change, None);
    assert_eq!(requirement.criteria[0].change, Some(Change::Changed));
}

#[test]
fn an_added_assertion_is_reviewed_with_nothing_before_it() {
    let mut document = small_document();
    document.products[0].feature_groups[0].features[0].requirements[0]
        .criteria
        .push(criterion(key(42), "A minted key is unique."));
    let packet = packed(&document, Some(&small_document()), None);
    assert_eq!(outline(&packet), [(key(3), ns(&[42]))]);
    let assertion = &unit(&packet, 3).assertions[0];
    assert_eq!(assertion.change, Some(Change::Added));
    assert!(assertion.before.is_none());
}

#[test]
fn a_moved_assertion_is_reviewed_in_its_new_scope_beside_the_base_s_node() {
    let mut document = small_document();
    let product = &mut document.products[0];
    let moved = product.feature_groups[0].constraints.remove(0);
    product.constraints.push(moved);
    let packet = packed(&document, Some(&small_document()), None);
    assert_eq!(outline(&packet), [(key(1), ns(&[9]))]);
    let assertion = &unit(&packet, 1).assertions[0];
    assert_eq!(assertion.change, Some(Change::Moved));
    assert!(assertion.before.is_some());
}

#[test]
fn a_removed_assertion_lands_in_the_unit_of_its_scope_at_the_base() {
    let mut document = small_document();
    document.products[0].feature_groups[0].features[0].requirements[0]
        .criteria
        .clear();
    document.components.clear();
    document.edges.retain(|edge| {
        !matches!(edge, Edge::Realizes { .. } | Edge::ScopedTo { .. })
            && !matches!(edge, Edge::Mentions { source, .. } if source.key() == key(5))
    });
    document.decisions.clear();
    let packet = packed(&document, Some(&small_document()), None);
    assert_eq!(
        outline(&packet),
        [(key(3), ns(&[5])), (key(6), ns(&[7, 8]))]
    );
    let removed = &unit(&packet, 3).assertions[0];
    assert_eq!(removed.change, Some(Change::Removed));
    assert_eq!(
        removed.before.as_ref().unwrap().name,
        "Every node record carries a `key` field."
    );
    assert_eq!(unit(&packet, 6).scope.node.change, Some(Change::Removed));
}

#[test]
fn a_node_narrows_the_review_to_the_assertions_at_or_below_it() {
    let document = small_document();
    let feature: NodeId = FeatureId::new(key(3)).into();
    assert_eq!(
        outline(&packed(&document, None, Some(feature))),
        [(key(3), ns(&[4, 5, 10]))]
    );
    let requirement: NodeId = loftspec::ids::RequirementId::new(key(4)).into();
    assert_eq!(
        outline(&packed(&document, None, Some(requirement))),
        [(key(3), ns(&[4, 5]))]
    );
    let mut changed = small_document();
    criterion_mut(&mut changed).name = "Every node record carries a key.".to_owned();
    changed.components[0].component.constraints[0].name =
        "The document model shall refuse unknown kinds.".to_owned();
    assert_eq!(
        outline(&packed(&changed, Some(&document), Some(feature))),
        [(key(3), ns(&[5]))]
    );
}

#[test]
fn a_node_without_a_key_is_refused_by_kind_and_name() {
    let mut document = small_document();
    criterion_mut(&mut document).id = None;
    let unkeyed = pack(&document, None, None).unwrap_err();
    assert_eq!(unkeyed.len(), 1);
    assert_eq!(unkeyed[0].kind, NodeKind::Criterion);
    assert_eq!(unkeyed[0].name, "Every node record carries a `key` field.");
}

#[test]
fn the_same_tree_and_base_give_the_same_packet() {
    let mut document = small_document();
    criterion_mut(&mut document).name = "Every node record carries a key.".to_owned();
    let base = small_document();
    assert_eq!(
        packed(&document, Some(&base), None),
        packed(&document, Some(&base), None)
    );
}
