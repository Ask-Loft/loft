//! The nodes a document added, changed and removed against its base (the `loft status`
//! Feature), over the small document: P-1 (1) > FG-1 (2) > F-1 (3) > RQ-2 (4) > criterion 5,
//! with CON-2 (9) under FG-1 and QA-2 (10) under F-1.

use crate::common::{criterion, key, small_document};
use loftspec::diff::{NodeChanges, content_changes, node_changes};
use loftspec::document::Document;
use loftspec::ids::{ConstraintId, CriterionId, FeatureId, NodeId};
use loftspec::tree::Requirement;

fn requirement(document: &mut Document) -> &mut Requirement {
    &mut document.products[0].feature_groups[0].features[0].requirements[0]
}

fn criterion_id(n: u32) -> NodeId {
    CriterionId::new(key(n)).into()
}

#[test]
fn a_document_equal_to_its_base_changes_no_node() {
    let changes = node_changes(&small_document(), &small_document());
    assert!(changes.is_empty(), "{changes:?}");
}

#[test]
fn a_node_whose_key_only_the_document_holds_is_added() {
    let mut document = small_document();
    requirement(&mut document)
        .criteria
        .push(criterion(key(40), "A new criterion holds."));
    let changes = node_changes(&small_document(), &document);
    assert_eq!(
        changes,
        NodeChanges {
            added: vec![criterion_id(40)],
            ..NodeChanges::default()
        }
    );
}

#[test]
fn a_node_whose_key_only_the_base_holds_is_removed() {
    let mut document = small_document();
    requirement(&mut document).criteria.clear();
    let changes = node_changes(&small_document(), &document);
    assert_eq!(
        changes,
        NodeChanges {
            removed: vec![criterion_id(5)],
            ..NodeChanges::default()
        }
    );
}

#[test]
fn a_node_whose_own_content_differs_is_changed_and_its_ancestors_are_not() {
    let mut document = small_document();
    requirement(&mut document).criteria[0].name = "Every node record carries a key.".to_owned();
    let changes = node_changes(&small_document(), &document);
    assert_eq!(
        changes,
        NodeChanges {
            changed: vec![criterion_id(5)],
            ..NodeChanges::default()
        }
    );
}

#[test]
fn a_changed_body_is_a_change_of_its_node_alone() {
    let mut document = small_document();
    document.products[0].feature_groups[0].features[0]
        .details
        .body = Some("Keys name nodes.".to_owned());
    let changes = node_changes(&small_document(), &document);
    assert_eq!(changes.changed, vec![FeatureId::new(key(3)).into()]);
    assert!(changes.added.is_empty() && changes.removed.is_empty());
}

#[test]
fn a_node_that_only_moved_to_another_parent_is_not_changed() {
    let mut document = small_document();
    let product = &mut document.products[0];
    let moved = product.feature_groups[0].constraints.remove(0);
    product.constraints.push(moved);
    let changes = node_changes(&small_document(), &document);
    assert!(changes.is_empty(), "{changes:?}");
    assert!(
        document.products[0]
            .constraints
            .iter()
            .any(|constraint| constraint.id == Some(ConstraintId::new(key(9))))
    );
}

#[test]
fn the_added_and_changed_follow_the_document_and_the_removed_follow_the_base() {
    let mut document = small_document();
    let requirement = requirement(&mut document);
    requirement.criteria[0].name = "Every record carries a key.".to_owned();
    requirement.criteria.insert(0, criterion(key(41), "First."));
    requirement.criteria.push(criterion(key(42), "Last."));
    document.products[0].feature_groups[0].constraints.clear();
    let changes = node_changes(&small_document(), &document);
    assert_eq!(changes.added, vec![criterion_id(41), criterion_id(42)]);
    assert_eq!(changes.changed, vec![criterion_id(5)]);
    assert_eq!(changes.removed, vec![ConstraintId::new(key(9)).into()]);
}

#[test]
fn a_node_whose_content_is_the_base_s_and_whose_parent_is_not_is_moved() {
    let mut document = small_document();
    let product = &mut document.products[0];
    let moved = product.feature_groups[0].constraints.remove(0);
    product.constraints.push(moved);
    let changes = node_changes(&small_document(), &document);
    assert_eq!(changes.moved, vec![ConstraintId::new(key(9)).into()]);
}

#[test]
fn a_node_that_moved_and_changed_is_changed_and_not_moved() {
    let mut document = small_document();
    let product = &mut document.products[0];
    let mut moved = product.feature_groups[0].constraints.remove(0);
    moved.name = "The part shall be bounded twice.".to_owned();
    product.constraints.push(moved);
    let changes = node_changes(&small_document(), &document);
    assert_eq!(changes.changed, vec![ConstraintId::new(key(9)).into()]);
    assert_eq!(changes.moved, Vec::<NodeId>::new());
}

#[test]
fn the_changes_of_content_alone_are_those_of_every_change_without_the_moved() {
    let mut document = small_document();
    let product = &mut document.products[0];
    let moved = product.feature_groups[0].constraints.remove(0);
    product.constraints.push(moved);
    requirement(&mut document).criteria[0].name = "Every record carries a key.".to_owned();
    let every = node_changes(&small_document(), &document);
    let content = content_changes(&small_document(), &document);
    assert_eq!(every.moved, vec![ConstraintId::new(key(9)).into()]);
    assert_eq!(
        content,
        NodeChanges {
            moved: Vec::new(),
            ..every
        }
    );
}
