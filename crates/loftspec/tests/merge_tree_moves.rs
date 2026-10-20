//! Records the tree merger finds in another file on one side or on both (F-32): written once,
//! where the moving side put them, or with a conflict over their place where the sides put them
//! apart.

use std::fmt::Write as _;

use crate::common::merge_tree::{
    ALIASES_FEATURE, LABELS_FEATURE, carried, dump, features, file, merged, moved, read,
    requirement_4, requirements, three_features, two_features,
};
use crate::common::text::{COMPONENT, FEATURE};
use crate::common::{component, criterion, key, requirement, root, slug};
use loftspec::document::Document;
use loftspec::ids::{ComponentId, CriterionId, RequirementId};

/// `base` with the Requirement keyed 41 moved from `Labels` to the end of `Keys`.
fn moved_41(base: &Document) -> Document {
    let mut ours = base.clone();
    let moving = requirements(&mut ours, 1).remove(0);
    requirements(&mut ours, 0).push(moving);
    ours
}

/// `base` with the Requirement keyed 4 moved from `Keys` to the end of `Aliases`.
fn moved_to_aliases(base: &Document) -> Document {
    let mut theirs = base.clone();
    let moving = requirements(&mut theirs, 0).remove(0);
    requirements(&mut theirs, 2).push(moving);
    theirs
}

/// The conflict lines over `field` with each side's value, none for a side that lacks the
/// field, at the indentation of a Requirement's fields.
fn conflict(field: &str, [base, ours, theirs]: [Option<&str>; 3]) -> String {
    let mut text = format!("    conflicts:\n      - field: {field}\n");
    for (name, side) in [("base", base), ("ours", ours), ("theirs", theirs)] {
        if let Some(side) = side {
            writeln!(text, "        {name}: {side}").unwrap();
        }
    }
    text
}

fn k(n: u32) -> String {
    key(n).to_string()
}

#[test]
fn a_record_one_side_moved_is_under_the_parent_at_the_position_and_in_the_file_it_went_to() {
    let base = two_features();
    let mut ours = base.clone();
    let moving = requirements(&mut ours, 0).remove(0);
    requirements(&mut ours, 1).insert(0, moving);
    let merged = merged(&base, &ours, &base);
    let text = file(&merged, LABELS_FEATURE);
    assert!(
        text.find(&k(4)).unwrap() < text.find(&k(41)).unwrap(),
        "{text}"
    );
    assert!(!file(&merged, FEATURE).contains(&k(5)));
    assert_eq!(carried(&merged, 4), 1);
}

#[test]
fn a_record_beneath_the_moved_record_that_the_other_side_changed_carries_the_change() {
    let base = two_features();
    let mut theirs = base.clone();
    requirement_4(&mut theirs).criteria[0].name = "Every node record carries one key.".to_owned();
    let merged = merged(&base, &moved(&base), &theirs);
    let mut document = read(&merged);
    let moved = &requirements(&mut document, 1)[1];
    assert_eq!(moved.criteria[0].name, "Every node record carries one key.");
    assert_eq!(carried(&merged, 5), 1);
}

#[test]
fn a_record_the_other_side_added_beneath_the_moved_record_is_beneath_it() {
    let base = two_features();
    let mut theirs = base.clone();
    requirement_4(&mut theirs)
        .criteria
        .push(criterion(key(50), "A key is written once."));
    let merged = merged(&base, &moved(&base), &theirs);
    let mut document = read(&merged);
    let moved = &requirements(&mut document, 1)[1];
    let ids: Vec<_> = moved
        .criteria
        .iter()
        .map(|criterion| criterion.id)
        .collect();
    let [first, added] = [5, 50].map(|n| Some(CriterionId::new(key(n))));
    assert_eq!(ids, [first, added]);
}

#[test]
fn a_moved_record_the_other_side_deleted_is_kept_where_it_went_with_a_record_conflict() {
    let base = two_features();
    let mut theirs = base.clone();
    requirements(&mut theirs, 1).clear();
    let merged = merged(&base, &moved_41(&base), &theirs);
    let held = conflict("record", [Some(&k(41)), Some(&k(41)), Some("null")]);
    let text = file(&merged, FEATURE);
    assert!(text.contains(&held), "{}", dump(&merged));
    assert_eq!(carried(&merged, 41), 1);
    assert_eq!(merged.conflicts, 1);
}

#[test]
fn a_new_parent_the_other_side_deleted_is_kept_with_a_record_conflict() {
    let base = two_features();
    let mut theirs = base.clone();
    features(&mut theirs).remove(1);
    let merged = merged(&base, &moved(&base), &theirs);
    let text = file(&merged, LABELS_FEATURE);
    let held = format!(
        "conflicts:\n  - field: record\n    base: {0}\n    ours: {0}\n",
        k(40)
    );
    assert!(text.contains(&held), "{text}");
    assert!(text.contains("    theirs: null\n"), "{text}");
    assert!(text.contains(&k(4)), "{text}");
    let document = read(&merged);
    assert_eq!(document.products[0].feature_groups[0].features.len(), 2);
}

#[test]
fn a_record_both_sides_moved_apart_is_written_once_at_its_base_parent_with_a_parent_conflict() {
    let base = three_features();
    let merged = merged(&base, &moved(&base), &moved_to_aliases(&base));
    assert_eq!(carried(&merged, 4), 1, "{}", dump(&merged));
    let expected = conflict("parent", [Some(&k(3)), Some(&k(40)), Some(&k(46))]);
    let text = file(&merged, FEATURE);
    assert!(text.contains(&expected), "{text}");
    assert!(!file(&merged, LABELS_FEATURE).contains(&k(4)));
    assert!(!file(&merged, ALIASES_FEATURE).contains(&k(4)));
    read(&merged);
}

#[test]
fn a_record_both_sides_moved_apart_from_a_parent_neither_holds_is_where_ours_put_it() {
    let base = three_features();
    let mut ours = moved_41(&base);
    features(&mut ours).remove(1);
    let mut theirs = base.clone();
    let moving = requirements(&mut theirs, 1).remove(0);
    requirements(&mut theirs, 2).push(moving);
    features(&mut theirs).remove(1);
    let merged = merged(&base, &ours, &theirs);
    assert_eq!(carried(&merged, 41), 1, "{}", dump(&merged));
    let text = file(&merged, FEATURE);
    assert!(text.contains(&k(41)), "{text}");
    assert!(text.contains("- field: parent\n"), "{text}");
    assert!(
        !merged
            .files
            .contains_key(&loftspec::codec::SpecPath::new(LABELS_FEATURE))
    );
    read(&merged);
}

#[test]
fn a_record_both_sides_added_at_different_parents_is_where_ours_put_it_without_a_base() {
    let base = two_features();
    let added = || requirement(key(60), "The writer shall add.", Vec::new());
    let mut ours = base.clone();
    requirements(&mut ours, 0).push(added());
    let mut theirs = base.clone();
    requirements(&mut theirs, 1).push(added());
    let merged = merged(&base, &ours, &theirs);
    assert_eq!(carried(&merged, 60), 1, "{}", dump(&merged));
    let expected = conflict("parent", [None, Some(&k(3)), Some(&k(40))]);
    let text = file(&merged, FEATURE);
    assert!(text.contains(&expected), "{text}");
    let document = read(&merged);
    let keys = &document.products[0].feature_groups[0].features[0].requirements;
    assert_eq!(keys[1].id, Some(RequirementId::new(key(60))));
}

#[test]
fn a_feature_both_sides_renamed_apart_keeps_its_base_slug_and_file_with_a_slug_conflict() {
    let base = two_features();
    let mut ours = base.clone();
    features(&mut ours)[1].slug = slug("labelling");
    let mut theirs = base.clone();
    features(&mut theirs)[1].slug = slug("label-rules");
    let merged = merged(&base, &ours, &theirs);
    let text = file(&merged, LABELS_FEATURE);
    let expected = "conflicts:\n  - field: slug\n    base: labels\n    ours: labelling\n    theirs: label-rules\n";
    assert!(
        text.starts_with(&format!("key: {}\nslug: labels\n", k(40))),
        "{text}"
    );
    assert!(text.ends_with(expected), "{text}");
    assert_eq!(merged.files.len(), loftspec::codec::write(&base).len());
}

/// The small document with a second root Component, `Store` keyed 72, and the Components `A`
/// keyed 70 beneath `Document model` and `B` keyed 71 beneath `Store`.
fn two_roots() -> Document {
    let mut document = crate::common::small_document();
    document.components[0]
        .component
        .components
        .push(component(key(70), "A"));
    let mut store = root(component(key(72), "Store"));
    store.component.components.push(component(key(71), "B"));
    document.components.push(store);
    document
}

#[test]
fn moves_that_would_place_a_record_beneath_itself_leave_each_at_its_base_parent_with_a_conflict() {
    let base = two_roots();
    let mut ours = base.clone();
    let a = ours.components[0].component.components.remove(0);
    ours.components[1].component.components[0]
        .components
        .push(a);
    let mut theirs = base.clone();
    let b = theirs.components[1].component.components.remove(0);
    theirs.components[0].component.components[0]
        .components
        .push(b);
    let merged = merged(&base, &ours, &theirs);
    assert_eq!(merged.conflicts, 2, "{}", dump(&merged));
    let document = read(&merged);
    let [first, second] = [0, 1].map(|at| &document.components[at].component.components[0]);
    assert_eq!(first.id, Some(ComponentId::new(key(70))));
    assert_eq!(second.id, Some(ComponentId::new(key(71))));
    assert!(file(&merged, COMPONENT).contains("- field: parent\n"));
    assert!(file(&merged, "components/store.yaml").contains("- field: parent\n"));
}
