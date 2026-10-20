//! The comparer (Comparing two versions): the nodes and decisions of two versions of a document
//! matched by key, each reported added, changed, moved, removed or reordered with its whole
//! record in each version that holds it, and whether the registries, the term list, the lint
//! configuration and the label bindings differ.

use crate::common::{criterion, key, product, requirement, section, small_document};
use loftspec::diff::{Comparison, DecisionChange, Difference, NodeDifference, Version, compare};
use loftspec::document::{Document, Edge};
use loftspec::ids::{ConstraintId, DecisionId, NodeId, ProductId, RequirementId};
use serde_json::{Value, json};

fn version(document: &Document) -> Version<'_> {
    Version {
        document,
        lint: None,
    }
}

/// The comparison of `older` with `newer`, neither holding `spec/lint.yaml`.
fn compared(older: &Document, newer: &Document) -> Comparison {
    compare(version(older), version(newer)).unwrap()
}

/// The difference reported for the node keyed `n`.
fn node(comparison: &Comparison, n: u32) -> Option<&NodeDifference> {
    comparison.nodes.iter().find(|node| node.id.key() == key(n))
}

fn reported(comparison: &Comparison) -> Vec<(u32, Difference)> {
    let number = |id: NodeId| (0..200).find(|n| key(*n) == id.key()).expect("a small key");
    comparison
        .nodes
        .iter()
        .map(|node| (number(node.id), node.difference))
        .collect()
}

fn feature_body(document: &mut Document, body: &str) {
    document.products[0].feature_groups[0].features[0]
        .details
        .body = Some(body.to_owned());
}

/// The small document with a second Requirement, keyed 40, after the first under its Feature.
fn with_second_requirement() -> Document {
    let mut document = small_document();
    let second = requirement(key(40), "The writer shall sort keys.", Vec::new());
    document.products[0].feature_groups[0].features[0]
        .requirements
        .push(second);
    document
}

fn swap_requirements(document: &mut Document) {
    document.products[0].feature_groups[0].features[0]
        .requirements
        .swap(0, 1);
}

/// The small document with its Feature Group's Constraint moved up to the Product.
fn constraint_moved_to_the_product() -> Document {
    let mut document = small_document();
    let constraint = document.products[0].feature_groups[0].constraints.remove(0);
    document.products[0].constraints.push(constraint);
    document
}

#[test]
fn a_node_only_the_newer_version_holds_is_added_with_its_newer_record() {
    let older = small_document();
    let newer = with_second_requirement();
    let comparison = compared(&older, &newer);
    let added = node(&comparison, 40).unwrap();
    assert_eq!(added.difference, Difference::Added);
    assert!(added.older.is_none());
    let record = &added.newer.as_ref().unwrap().record;
    assert_eq!(record["key"], json!(key(40).to_string()));
}

#[test]
fn a_node_only_the_older_version_holds_is_removed_with_its_older_record() {
    let comparison = compared(&with_second_requirement(), &small_document());
    let removed = node(&comparison, 40).unwrap();
    assert_eq!(removed.difference, Difference::Removed);
    assert!(removed.newer.is_none());
    assert_eq!(
        removed.older.as_ref().unwrap().record["key"],
        json!(key(40).to_string())
    );
}

#[test]
fn a_node_whose_record_differs_is_changed_with_both_records_and_nothing_above_it_is_reported() {
    let older = small_document();
    let mut newer = small_document();
    feature_body(&mut newer, "Keys name nodes.");
    let comparison = compared(&older, &newer);
    assert_eq!(reported(&comparison), [(3, Difference::Changed)]);
    let changed = node(&comparison, 3).unwrap();
    assert_eq!(
        changed.older.as_ref().unwrap().record["body"],
        json!("Keys address nodes.")
    );
    assert_eq!(
        changed.newer.as_ref().unwrap().record["body"],
        json!("Keys name nodes.")
    );
}

#[test]
fn a_node_whose_record_is_the_same_and_whose_parent_differs_is_moved_naming_each_parent() {
    let comparison = compared(&small_document(), &constraint_moved_to_the_product());
    assert_eq!(reported(&comparison), [(9, Difference::Moved)]);
    let moved = node(&comparison, 9).unwrap();
    let parent = |side: &Option<loftspec::diff::Side>| side.as_ref().unwrap().parent;
    assert_eq!(parent(&moved.older), Some(NodeId::FeatureGroup(fg())));
    assert_eq!(
        parent(&moved.newer),
        Some(NodeId::Product(ProductId::new(key(1))))
    );
    assert_eq!(
        moved.older.as_ref().unwrap().record,
        moved.newer.as_ref().unwrap().record
    );
    assert_eq!(moved.id, NodeId::Constraint(ConstraintId::new(key(9))));
}

fn fg() -> loftspec::ids::FeatureGroupId {
    loftspec::ids::FeatureGroupId::new(key(2))
}

#[test]
fn a_product_or_a_root_component_names_no_parent() {
    let mut newer = small_document();
    newer.products[0].details.body = Some("Changed.".to_owned());
    newer.components[0].component.details.body = Some("Changed.".to_owned());
    let comparison = compared(&small_document(), &newer);
    for n in [1, 6] {
        let changed = node(&comparison, n).unwrap();
        assert_eq!(changed.older.as_ref().unwrap().parent, None, "{n}");
        assert_eq!(changed.newer.as_ref().unwrap().parent, None, "{n}");
    }
}

#[test]
fn a_record_holds_its_own_fields_and_edges_and_nothing_of_the_nodes_beneath_it() {
    let mut newer = small_document();
    feature_body(&mut newer, "Keys name nodes.");
    newer.products[0].details.body = Some("Changed.".to_owned());
    newer.products[0].feature_groups[0].details.body = Some("Changed.".to_owned());
    newer.components[0].component.details.body = Some("Changed.".to_owned());
    newer.products[0].feature_groups[0].features[0].requirements[0].criteria[0].name =
        "Every record carries a key.".to_owned();
    let comparison = compared(&small_document(), &newer);
    let record = |n: u32| {
        node(&comparison, n)
            .unwrap()
            .newer
            .as_ref()
            .unwrap()
            .record
            .clone()
    };
    let feature = record(3);
    assert_eq!(feature["realizes"], json!([key(6).to_string()]));
    assert_eq!(feature["name"], json!("Keys"));
    assert!(feature.get("requirements").is_none() && feature.get("qualityAttributes").is_none());
    assert_eq!(record(6)["scopedTo"], json!([key(1).to_string()]));
    assert!(record(6).get("constraints").is_none());
    assert_eq!(record(5)["mentions"], json!([key(4).to_string()]));
    assert!(record(2).get("features").is_none() && record(2).get("constraints").is_none());
    assert!(record(1).get("featureGroups").is_none());
}

#[test]
fn a_node_whose_children_of_both_versions_stand_in_another_order_is_reordered() {
    let older = with_second_requirement();
    let mut newer = with_second_requirement();
    swap_requirements(&mut newer);
    let comparison = compared(&older, &newer);
    assert_eq!(reported(&comparison), [(3, Difference::Reordered)]);
    let children = node(&comparison, 3).unwrap().order.as_ref().unwrap();
    let ids = |keys: &[u32]| -> Vec<NodeId> {
        keys.iter()
            .map(|n| NodeId::Requirement(RequirementId::new(key(*n))))
            .collect()
    };
    let mut before = ids(&[4, 40]);
    before.push(NodeId::QualityAttribute(
        loftspec::ids::QualityAttributeId::new(key(10)),
    ));
    let mut after = ids(&[40, 4]);
    after.push(NodeId::QualityAttribute(
        loftspec::ids::QualityAttributeId::new(key(10)),
    ));
    assert_eq!((&children.older, &children.newer), (&before, &after));
}

#[test]
fn a_changed_or_moved_node_whose_children_stand_in_another_order_carries_their_order() {
    let older = with_second_requirement();
    let mut newer = with_second_requirement();
    swap_requirements(&mut newer);
    feature_body(&mut newer, "Keys name nodes.");
    let comparison = compared(&older, &newer);
    let changed = node(&comparison, 3).unwrap();
    assert_eq!(changed.difference, Difference::Changed);
    assert!(changed.order.is_some());
    let sections = |document: &mut Document| {
        let constraint = &mut document.products[0].feature_groups[0].constraints[0];
        constraint.details.sections = vec![section(42, "One", "A."), section(43, "Two", "B.")];
    };
    let mut older = small_document();
    sections(&mut older);
    let mut newer = older.clone();
    newer.products[0].feature_groups[0].constraints[0]
        .details
        .sections
        .swap(0, 1);
    let constraint = newer.products[0].feature_groups[0].constraints.remove(0);
    newer.products[0].constraints.push(constraint);
    let comparison = compared(&older, &newer);
    let moved = node(&comparison, 9).unwrap();
    assert_eq!(moved.difference, Difference::Moved);
    assert!(moved.order.is_some());
}

#[test]
fn the_order_is_of_the_children_both_versions_hold() {
    let older = with_second_requirement();
    let mut newer = with_second_requirement();
    swap_requirements(&mut newer);
    newer.products[0].feature_groups[0].features[0]
        .quality_attributes
        .clear();
    let comparison = compared(&older, &newer);
    let children = node(&comparison, 3).unwrap().order.as_ref().unwrap();
    assert_eq!(children.older.len(), 2);
    assert_eq!(children.newer.len(), 2);
}

#[test]
fn children_only_added_or_removed_are_no_reordering() {
    let comparison = compared(&small_document(), &with_second_requirement());
    assert_eq!(reported(&comparison), [(40, Difference::Added)]);
    assert!(comparison.nodes.iter().all(|node| node.order.is_none()));
}

#[test]
fn products_in_another_order_are_a_reordering_of_the_document() {
    let mut older = small_document();
    older.products.push(product(key(80), "Second"));
    let mut newer = older.clone();
    newer.products.swap(0, 1);
    let comparison = compared(&older, &newer);
    let products = comparison.products.unwrap();
    let first = NodeId::Product(ProductId::new(key(1)));
    let second = NodeId::Product(ProductId::new(key(80)));
    assert_eq!(products.older, [first, second]);
    assert_eq!(products.newer, [second, first]);
    assert_eq!(compared(&older, &older).products, None);
}

fn added_decision(document: &mut Document) -> DecisionId {
    let decision = crate::common::proposed_decision(key(90), "a-proposal");
    let id = decision.id;
    document.decisions.push(decision);
    id
}

#[test]
fn a_decision_only_one_version_holds_is_added_or_removed_with_its_record() {
    let older = small_document();
    let mut newer = small_document();
    let id = added_decision(&mut newer);
    let added = compared(&older, &newer).decisions;
    assert_eq!(added.len(), 1);
    assert_eq!((added[0].id, added[0].change), (id, DecisionChange::Added));
    assert_eq!(
        added[0].newer.as_ref().unwrap()["title"],
        json!("A proposal")
    );
    assert!(added[0].older.is_none());
    let removed = compared(&newer, &older).decisions;
    assert_eq!(removed[0].change, DecisionChange::Removed);
    assert!(removed[0].older.is_some() && removed[0].newer.is_none());
}

#[test]
fn a_decision_whose_record_differs_is_changed_and_an_unchanged_one_is_not_reported() {
    let older = small_document();
    let mut newer = small_document();
    newer.decisions[0].title.push_str(" (edited)");
    let changed = compared(&older, &newer).decisions;
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].change, DecisionChange::Changed);
    assert_ne!(changed[0].older, changed[0].newer);
    assert_eq!(compared(&older, &older).decisions, []);
}

#[test]
fn a_decision_s_record_holds_every_field_its_subjects_and_mentions_included() {
    let older = small_document();
    let mut newer = small_document();
    newer.edges.push(Edge::Mentions {
        source: DecisionId::new(key(20)).into(),
        target: RequirementId::new(key(4)).into(),
    });
    let changed = compared(&older, &newer).decisions;
    let record = changed[0].newer.as_ref().unwrap();
    assert_eq!(record["mentions"], json!([key(4).to_string()]));
    assert_eq!(record["subjects"].as_array().unwrap().len(), 2);
    for field in [
        "key", "slug", "title", "context", "state", "drivers", "options",
    ] {
        assert!(record.get(field).is_some(), "{field}");
    }
    assert!(record.get("assessments").is_some() && record.get("consequences").is_some());
}

#[test]
fn a_decision_is_never_moved_since_it_has_no_parent() {
    let mut newer = small_document();
    newer.decisions[0].context.push_str(" Edited.");
    let changes: Vec<DecisionChange> = compared(&small_document(), &newer)
        .decisions
        .iter()
        .map(|decision| decision.change)
        .collect();
    assert_eq!(changes, [DecisionChange::Changed]);
}

#[test]
fn nodes_follow_the_newer_document_order_then_the_removed_in_the_older_order() {
    let mut older = with_second_requirement();
    older.products[0].feature_groups[0].features[0].requirements[1]
        .criteria
        .push(criterion(key(41), "Sorted by key."));
    let mut newer = small_document();
    feature_body(&mut newer, "Keys name nodes.");
    newer.products[0].details.body = Some("Changed.".to_owned());
    newer.components[0].component.details.body = Some("Changed.".to_owned());
    let comparison = compared(&older, &newer);
    assert_eq!(
        reported(&comparison),
        [
            (1, Difference::Changed),
            (3, Difference::Changed),
            (6, Difference::Changed),
            (40, Difference::Removed),
            (41, Difference::Removed),
        ]
    );
}

#[test]
fn decisions_follow_the_byte_order_of_their_keys() {
    let older = small_document();
    let mut newer = small_document();
    let mut early = crate::common::proposed_decision(key(15), "early");
    early.title = "Early".to_owned();
    newer
        .decisions
        .insert(0, crate::common::proposed_decision(key(90), "late"));
    newer.decisions.push(early);
    newer.decisions[1].title.push_str(" (edited)");
    let keys: Vec<_> = compared(&older, &newer)
        .decisions
        .iter()
        .map(|decision| decision.id.key())
        .collect();
    assert_eq!(keys, [key(15), key(20), key(90)]);
}

#[test]
fn two_comparisons_of_the_same_versions_are_equal_and_a_version_compared_with_itself_is_empty() {
    let older = small_document();
    let mut newer = with_second_requirement();
    added_decision(&mut newer);
    assert_eq!(compared(&older, &newer), compared(&older, &newer));
    let itself = compared(&newer, &newer);
    assert_eq!(itself.nodes, []);
    assert_eq!(itself.decisions, []);
    assert_eq!(itself.products, None);
    assert_eq!(itself.parts, []);
}

#[test]
fn each_version_is_given_in_memory_and_an_empty_older_one_makes_everything_added() {
    let newer = small_document();
    let empty = Document::empty(newer.id);
    let comparison = compared(&empty, &newer);
    let mut keys = Vec::new();
    newer.walk(&mut |node| keys.extend(node.id()));
    let added: Vec<NodeId> = comparison.nodes.iter().map(|node| node.id).collect();
    assert_eq!(added, keys);
    assert!(
        comparison
            .nodes
            .iter()
            .all(|node| node.difference == Difference::Added)
    );
    assert_eq!(comparison.decisions.len(), 1);
    assert_eq!(comparison.decisions[0].change, DecisionChange::Added);
}

#[test]
fn a_record_is_json_as_the_text_form_writes_it() {
    let mut newer = small_document();
    newer.products[0].details.body = Some("Changed.".to_owned());
    let comparison = compared(&small_document(), &newer);
    let record: &Value = &node(&comparison, 1).unwrap().newer.as_ref().unwrap().record;
    assert_eq!(record["slug"], json!("example-product"));
    assert_eq!(record["tags"], json!(["portable"]));
}
