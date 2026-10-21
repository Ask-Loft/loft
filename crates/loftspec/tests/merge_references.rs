//! A tree merge whose result references a record a side deleted: the record is kept, as the other
//! side holds it, with a conflict record over its existence, so that the result always reads.

use loftspec::codec::{self, SpecPath};
use loftspec::conflict::{Conflict, ConflictOwner, ConflictValue, FieldPath};
use loftspec::document::{Document, Edge};
use loftspec::edit;
use loftspec::ids::{ComponentId, ConsequenceId, ElementId, FeatureId, NodeId, RequirementId};
use loftspec::merge::{merge_file, merge_tree};
use loftspec::tree::Constraint;

use crate::common::merge_tree::{dump, merged, read};
use crate::common::text::FEATURE;
use crate::common::{component, key, requirement, root, small_document};

/// The root Component keyed 90 and the one keyed 92, which it mentions.
const STORAGE: u32 = 90;
const CACHE: u32 = 92;
/// The constraint keyed 91 beneath Storage.
const LIMIT: u32 = 91;
/// A second Requirement of the Feature `Keys`, keyed 60.
const SECOND: u32 = 60;

fn component_id(n: u32) -> ComponentId {
    ComponentId::new(key(n))
}

/// The small document with two more root Components, neither labelled: Storage, holding a
/// constraint and mentioning Cache, and Cache.
fn with_storage() -> Document {
    let mut document = small_document();
    let mut storage = component(key(STORAGE), "Storage");
    let mut limit: Constraint = document.components[0].component.constraints[0].clone();
    limit.id = Some(loftspec::ids::ConstraintId::new(key(LIMIT)));
    storage.constraints = vec![limit];
    document.components.push(root(storage));
    document
        .components
        .push(root(component(key(CACHE), "Cache")));
    document.edges.push(Edge::Mentions {
        source: component_id(STORAGE).into(),
        target: component_id(CACHE).into(),
    });
    document
}

/// `document` without the node `node`, every node beneath it and the edges they write.
fn deleted(document: &Document, node: impl Into<NodeId>) -> Document {
    let mut document = document.clone();
    edit::detach(&mut document, node.into()).expect("the node is held");
    document
}

/// `document` with the Feature keyed 3 realizing the Component keyed `n`.
fn realizing(document: &Document, n: u32) -> Document {
    let mut document = document.clone();
    document.edges.push(Edge::Realizes {
        feature: FeatureId::new(key(3)),
        component: component_id(n),
    });
    document
}

/// The conflict over `record` that a merge writes on the record keyed `n`, which theirs deleted.
fn kept_against_theirs(owner: ConflictOwner, n: u32) -> Conflict {
    let held = || Some(ConflictValue::Scalar(key(n).to_string()));
    Conflict {
        owner,
        field: FieldPath::new("record"),
        base: held(),
        ours: held(),
        theirs: Some(ConflictValue::Deleted),
    }
}

fn component_owner(n: u32) -> ConflictOwner {
    ConflictOwner::Element(ElementId::Node(component_id(n).into()))
}

fn holds_component(document: &Document, n: u32) -> bool {
    document
        .components
        .iter()
        .any(|each| each.component.id == Some(component_id(n)))
}

#[test]
fn a_record_one_side_deleted_that_the_other_side_starts_to_reference_is_kept_and_the_result_reads()
{
    let base = with_storage();
    let ours = realizing(&base, STORAGE);
    let theirs = deleted(&deleted(&base, component_id(STORAGE)), component_id(CACHE));
    let merged = merged(&base, &ours, &theirs);
    let document = read(&merged);
    assert!(holds_component(&document, STORAGE), "{}", dump(&merged));
    assert!(document.edges.contains(&Edge::Realizes {
        feature: FeatureId::new(key(3)),
        component: component_id(STORAGE),
    }));
}

#[test]
fn a_record_kept_for_a_reference_holds_a_conflict_over_record_whose_deleting_side_is_null() {
    let base = with_storage();
    let ours = realizing(&base, STORAGE);
    let theirs = deleted(&base, component_id(STORAGE));
    let document = read(&merged(&base, &ours, &theirs));
    assert!(
        document
            .conflicts
            .contains(&kept_against_theirs(component_owner(STORAGE), STORAGE)),
        "{:?}",
        document.conflicts
    );
}

#[test]
fn a_record_kept_for_a_reference_keeps_each_record_beneath_it_that_the_other_side_holds() {
    let base = with_storage();
    let ours = realizing(&base, STORAGE);
    let theirs = deleted(&base, component_id(STORAGE));
    let document = read(&merged(&base, &ours, &theirs));
    let storage = document
        .components
        .iter()
        .find(|each| each.component.id == Some(component_id(STORAGE)))
        .expect("Storage is kept");
    let held: Vec<_> = storage
        .component
        .constraints
        .iter()
        .map(|each| each.id)
        .collect();
    assert_eq!(held, [Some(loftspec::ids::ConstraintId::new(key(LIMIT)))]);
}

#[test]
fn a_record_that_a_record_kept_for_a_reference_references_is_kept_by_the_same_rule() {
    let base = with_storage();
    let ours = realizing(&base, STORAGE);
    let theirs = deleted(&deleted(&base, component_id(STORAGE)), component_id(CACHE));
    let document = read(&merged(&base, &ours, &theirs));
    assert!(holds_component(&document, CACHE));
    assert!(
        document
            .conflicts
            .contains(&kept_against_theirs(component_owner(CACHE), CACHE))
    );
}

#[test]
fn a_record_one_side_deleted_that_nothing_in_the_result_references_is_still_deleted() {
    let base = with_storage();
    let theirs = deleted(&base, component_id(STORAGE));
    let document = read(&merged(&base, &base, &theirs));
    assert!(!holds_component(&document, STORAGE));
    assert_eq!(document.conflicts, []);
}

#[test]
fn merging_the_same_trees_twice_names_the_conflict_of_a_record_kept_for_a_reference_alike() {
    let base = with_storage();
    let ours = realizing(&base, STORAGE);
    let theirs = deleted(&base, component_id(STORAGE));
    let ids = || {
        let document = read(&merged(&base, &ours, &theirs));
        document
            .conflicts
            .iter()
            .map(Conflict::id)
            .collect::<Vec<_>>()
    };
    let first = ids();
    assert_eq!(
        first,
        [kept_against_theirs(component_owner(STORAGE), STORAGE).id()]
    );
    assert_eq!(ids(), first);
}

/// The small document with a second Requirement in `Keys`, keyed 60, that the consequence keyed
/// 25 names as its subject.
fn with_second_subject() -> Document {
    let mut document = small_document();
    let second = requirement(key(SECOND), "The writer shall write a slug.", Vec::new());
    document.products[0].feature_groups[0].features[0]
        .requirements
        .push(second);
    let consequence = &mut document.decisions[0].consequences[0];
    consequence.subject = Some(RequirementId::new(key(SECOND)).into());
    document
}

/// `document` without the second Requirement, its consequence naming `subject` instead.
fn without_second(document: &Document, subject: Option<u32>) -> Document {
    let mut document = deleted(document, RequirementId::new(key(SECOND)));
    document.decisions[0].consequences[0].subject =
        subject.map(|n| RequirementId::new(key(n)).into());
    document
}

#[test]
fn a_field_both_sides_changed_apart_whose_base_value_names_a_record_both_deleted_holds_ours() {
    let base = with_second_subject();
    let ours = without_second(&base, Some(4));
    let theirs = without_second(&base, None);
    let merged = merged(&base, &ours, &theirs);
    let document = read(&merged);
    let consequence = &document.decisions[0].consequences[0];
    assert_eq!(
        consequence.subject,
        Some(RequirementId::new(key(4)).into()),
        "{}",
        dump(&merged)
    );
    let conflict = Conflict {
        owner: ConflictOwner::Consequence(ConsequenceId::new(key(25))),
        field: FieldPath::new("subject"),
        base: Some(ConflictValue::Scalar(key(SECOND).to_string())),
        ours: Some(ConflictValue::Scalar(key(4).to_string())),
        theirs: None,
    };
    assert_eq!(document.conflicts, [conflict]);
}

#[test]
fn the_merge_driver_deletes_a_record_one_side_deleted_that_the_tree_merger_keeps_for_a_reference() {
    let mut base = small_document();
    let second = requirement(key(SECOND), "The writer shall write a slug.", Vec::new());
    base.products[0].feature_groups[0].features[0]
        .requirements
        .push(second);
    let mut ours = base.clone();
    ours.edges.push(Edge::Mentions {
        source: loftspec::ids::CriterionId::new(key(5)).into(),
        target: RequirementId::new(key(SECOND)).into(),
    });
    let theirs = deleted(&base, RequirementId::new(key(SECOND)));
    let [base, ours, theirs] = [&base, &ours, &theirs].map(codec::write);
    let path = SpecPath::new(FEATURE);
    let driver = merge_file(&path, &base[&path], &ours[&path], &theirs[&path]).expect("merges");
    let tree = merge_tree(&base, &ours, &theirs).expect("the trees merge");
    let line = format!("key: {}\n", key(SECOND));
    assert!(!String::from_utf8_lossy(&driver.bytes).contains(&line));
    assert_eq!(driver.conflicts, 0);
    let file = String::from_utf8_lossy(&tree.files[&path]).into_owned();
    assert!(file.contains(&line), "{file}");
    let owner = ConflictOwner::Element(ElementId::Node(RequirementId::new(key(SECOND)).into()));
    let document = codec::read(&tree.files).expect("the merged tree reads");
    assert_eq!(document.conflicts, [kept_against_theirs(owner, SECOND)]);
}
