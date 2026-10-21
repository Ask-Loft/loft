//! Settling a conflict over `record`, `parent` or `position` of a document held in memory
//! (Settling a conflict): a record kept or deleted, moved under another parent, or moved within
//! its list, each by a side only.

use loftspec::conflict::{Conflict, ConflictValue, SettleError, Settlement, settle};
use loftspec::document::Document;
use loftspec::ids::{ElementId, RequirementId};
use loftspec::merge::Input;

use crate::common::merge_tree::{merged, moved, read, requirements, three_features, two_features};
use crate::common::{key, label, requirement, small_document};

const SECOND: u32 = 60;
const THIRD: u32 = 61;

fn merge(base: &Document, ours: &Document, theirs: &Document) -> Document {
    read(&merged(base, ours, theirs))
}

fn over<'d>(document: &'d Document, field: &str) -> &'d Conflict {
    let mut found = document
        .conflicts
        .iter()
        .filter(|conflict| conflict.field.as_str() == field);
    let conflict = found
        .next()
        .unwrap_or_else(|| panic!("a conflict over {field}: {:?}", document.conflicts));
    assert!(found.next().is_none(), "{:?}", document.conflicts);
    conflict
}

fn settled(document: &Document, field: &str, settlement: &Settlement) -> Document {
    settle(document, over(document, field).id(), settlement)
        .unwrap_or_else(|refusal| panic!("{refusal:?}"))
}

fn with_requirements(document: &Document, at: usize, keys: &[u32]) -> Document {
    let mut document = document.clone();
    for &n in keys {
        let added = requirement(key(n), "The writer shall write a slug.", Vec::new());
        requirements(&mut document, at).push(added);
    }
    document
}

/// The keys of the Requirements of the group's Feature at `at`.
fn keys_at(document: &Document, at: usize) -> Vec<u32> {
    let mut document = document.clone();
    requirements(&mut document, at)
        .iter()
        .map(|each| {
            let text = each.id.unwrap().key().to_string();
            u32::from_str_radix(&text[text.len() - 8..], 16).unwrap()
        })
        .collect()
}

/// `document` with the Requirements of `Keys` in the order `keys` gives.
fn ordered(document: &Document, keys: &[u32]) -> Document {
    let mut document = document.clone();
    let list = requirements(&mut document, 0);
    let mut held = std::mem::take(list);
    for &n in keys {
        let at = held
            .iter()
            .position(|each| each.id == Some(RequirementId::new(key(n))))
            .unwrap();
        list.push(held.remove(at));
    }
    document
}

/// The small document with two more Requirements in `Keys`: 4, 60, 61.
fn three_requirements() -> Document {
    with_requirements(&small_document(), 0, &[SECOND, THIRD])
}

/// A conflict over the position of the Requirement keyed 61, which ours put first and theirs
/// after 4; it stays after 60.
fn position_conflict() -> Document {
    let base = three_requirements();
    let ours = ordered(&base, &[THIRD, 4, SECOND]);
    let theirs = ordered(&base, &[4, THIRD, SECOND]);
    merge(&base, &ours, &theirs)
}

/// A conflict over the existence of the Requirement keyed 60, which ours changed and theirs
/// deleted; `labelled` binds it to `RQ-3` first.
fn record_conflict(labelled: bool) -> Document {
    let mut base = three_requirements();
    if labelled {
        let second = RequirementId::new(key(SECOND));
        base.bindings.bind(second, label("RQ-3")).unwrap();
    }
    let mut ours = base.clone();
    requirements(&mut ours, 0)[1].details.body = Some("Ours.".to_owned());
    let mut theirs = base.clone();
    requirements(&mut theirs, 0).remove(1);
    merge(&base, &ours, &theirs)
}

/// A conflict over the parent of the Requirement keyed 4, which ours moved to `Labels` and
/// theirs to `Aliases`; it stays in `Keys`.
fn parent_conflict() -> Document {
    let base = with_requirements(&three_features(), 2, &[SECOND]);
    let mut theirs = base.clone();
    let moving = requirements(&mut theirs, 0).remove(0);
    requirements(&mut theirs, 2).push(moving);
    merge(&base, &moved(&base), &theirs)
}

#[test]
fn settling_a_record_conflict_by_a_side_that_holds_its_key_keeps_the_record_as_it_is() {
    let document = record_conflict(false);
    assert_eq!(keys_at(&document, 0), [4, SECOND, THIRD]);
    let kept = settled(&document, "record", &Settlement::Ours);
    let mut expected = document.clone();
    expected.conflicts.clear();
    assert_eq!(kept, expected);
}

#[test]
fn settling_a_record_conflict_by_a_side_that_holds_null_deletes_the_record() {
    let document = record_conflict(false);
    assert_eq!(
        over(&document, "record").theirs,
        Some(ConflictValue::Deleted)
    );
    let deleted = settled(&document, "record", &Settlement::Theirs);
    assert_eq!(keys_at(&deleted, 0), [4, THIRD]);
    assert_eq!(deleted.conflicts, []);
}

#[test]
fn settling_a_record_conflict_by_deletion_deletes_every_record_beneath_it() {
    let mut base = small_document();
    let criterion = crate::common::criterion(key(62), "A slug is written.");
    let second = requirement(
        key(SECOND),
        "The writer shall write a slug.",
        vec![criterion],
    );
    requirements(&mut base, 0).push(second);
    let mut ours = base.clone();
    requirements(&mut ours, 0)[1].details.body = Some("Ours.".to_owned());
    let mut theirs = base.clone();
    requirements(&mut theirs, 0).remove(1);
    let document = merge(&base, &ours, &theirs);
    assert!(loftspec::edit::find(&document, key(62)).is_some());
    let deleted = settled(&document, "record", &Settlement::Theirs);
    assert_eq!(keys_at(&deleted, 0), [4]);
    assert!(loftspec::edit::find(&deleted, key(62)).is_none());
}

#[test]
fn settling_a_record_conflict_by_deletion_departs_the_binding_of_each_element_it_takes_out() {
    let document = record_conflict(true);
    let second = RequirementId::new(key(SECOND));
    assert_eq!(document.bindings.label_of(second), Some(label("RQ-3")));
    let deleted = settled(&document, "record", &Settlement::Theirs);
    assert!(deleted.bindings.is_departed(key(SECOND)));
    assert_eq!(deleted.bindings.label_of(second), None);
}

#[test]
fn settling_a_parent_conflict_moves_the_record_last_of_its_kind_under_the_sides_parent() {
    let document = parent_conflict();
    assert_eq!(keys_at(&document, 0)[0], 4);
    let theirs = settled(&document, "parent", &Settlement::Theirs);
    assert_eq!(keys_at(&theirs, 2), [SECOND, 4]);
    assert!(!keys_at(&theirs, 0).contains(&4));
    let ours = settled(&document, "parent", &Settlement::Ours);
    assert_eq!(keys_at(&ours, 1), [41, 4]);
}

#[test]
fn settling_a_parent_conflict_by_the_side_of_its_present_parent_leaves_the_record_in_place() {
    let document = parent_conflict();
    let base = settled(&document, "parent", &Settlement::Base);
    let mut expected = document.clone();
    expected.conflicts.clear();
    assert_eq!(base, expected);
}

#[test]
fn settling_a_parent_conflict_moves_every_record_beneath_the_record_with_it() {
    let document = parent_conflict();
    let theirs = settled(&document, "parent", &Settlement::Theirs);
    let mut held = theirs.clone();
    let moved = &requirements(&mut held, 2)[1];
    let criteria: Vec<_> = moved.criteria.iter().map(|each| each.id).collect();
    assert_eq!(criteria, [Some(loftspec::ids::CriterionId::new(key(5)))]);
}

#[test]
fn settling_a_position_conflict_puts_the_record_right_after_the_one_the_side_names() {
    let document = position_conflict();
    assert_eq!(keys_at(&document, 0), [4, SECOND, THIRD]);
    let theirs = settled(&document, "position", &Settlement::Theirs);
    assert_eq!(keys_at(&theirs, 0), [4, THIRD, SECOND]);
    let base = settled(&document, "position", &Settlement::Base);
    assert_eq!(keys_at(&base, 0), [4, SECOND, THIRD]);
}

#[test]
fn settling_a_position_conflict_by_a_side_that_holds_first_puts_the_record_first() {
    let document = position_conflict();
    assert_eq!(
        over(&document, "position").ours,
        Some(ConflictValue::Scalar("first".to_owned()))
    );
    let ours = settled(&document, "position", &Settlement::Ours);
    assert_eq!(keys_at(&ours, 0), [THIRD, 4, SECOND]);
}

#[test]
fn a_value_for_a_conflict_over_record_parent_or_position_is_refused_naming_its_field() {
    let value = || Settlement::Value(ConflictValue::Scalar(key(4).to_string()));
    for (document, field) in [
        (record_conflict(false), "record"),
        (parent_conflict(), "parent"),
        (position_conflict(), "position"),
    ] {
        let refused = settle(&document, over(&document, field).id(), &value());
        let path = loftspec::conflict::FieldPath::new(field);
        assert_eq!(refused, Err(SettleError::SideOnly(path)));
    }
}

#[test]
fn a_side_that_holds_nothing_for_a_conflict_over_a_records_place_is_refused_naming_the_side() {
    let base = two_features();
    let added = || requirement(key(SECOND), "The writer shall add.", Vec::new());
    let mut ours = base.clone();
    requirements(&mut ours, 0).push(added());
    let mut theirs = base.clone();
    requirements(&mut theirs, 1).push(added());
    let document = merge(&base, &ours, &theirs);
    let refused = settle(&document, over(&document, "parent").id(), &Settlement::Base);
    let field = loftspec::conflict::FieldPath::new("parent");
    let expected = SettleError::NoSide {
        side: Input::Base,
        field,
    };
    assert_eq!(refused, Err(expected));
}

#[test]
fn a_position_side_that_names_a_record_its_list_lacks_is_refused_naming_that_record() {
    let mut document = position_conflict();
    let conflict = document
        .conflicts
        .iter_mut()
        .find(|each| each.field.as_str() == "position")
        .unwrap();
    conflict.theirs = Some(ConflictValue::Scalar(key(999).to_string()));
    let id = conflict.id();
    let refused = settle(&document, id, &Settlement::Theirs);
    assert_eq!(refused, Err(SettleError::NoRecord(key(999))));
}

#[test]
fn a_settlement_whose_document_would_not_read_is_refused_with_the_read_errors() {
    let base = small_document();
    let mut ours = base.clone();
    let mut storage = crate::common::component(key(90), "Storage");
    storage.details.body = Some("Base.".to_owned());
    let mut with_storage = base.clone();
    with_storage.components.push(crate::common::root(storage));
    ours.clone_from(&with_storage);
    ours.edges.push(loftspec::document::Edge::Realizes {
        feature: loftspec::ids::FeatureId::new(key(3)),
        component: loftspec::ids::ComponentId::new(key(90)),
    });
    let document = merge(&with_storage, &ours, &base);
    let refused = settled_err(&document, "record", &Settlement::Theirs);
    let SettleError::Unreadable(errors) = refused else {
        panic!("{refused:?}");
    };
    let mut deleted = document.clone();
    loftspec::edit::detach(
        &mut deleted,
        loftspec::ids::ComponentId::new(key(90)).into(),
    )
    .ok();
    deleted.conflicts.clear();
    let expected = loftspec::codec::read(&loftspec::codec::write(&deleted)).unwrap_err();
    assert_eq!(errors, expected);
}

fn settled_err(document: &Document, field: &str, settlement: &Settlement) -> SettleError {
    settle(document, over(document, field).id(), settlement).expect_err("refused")
}

#[test]
fn a_record_conflict_names_the_element_it_settles() {
    let document = record_conflict(false);
    let owner = over(&document, "record").owner;
    let element = ElementId::Node(RequirementId::new(key(SECOND)).into());
    assert_eq!(owner, loftspec::conflict::ConflictOwner::Element(element));
}

/// The keys of the consequences of the decision at `at`.
fn consequences(document: &Document, at: usize) -> Vec<u32> {
    document.decisions[at]
        .consequences
        .iter()
        .map(|each| {
            let text = each.id.key().to_string();
            u32::from_str_radix(&text[text.len() - 8..], 16).unwrap()
        })
        .collect()
}

/// The small document's decision with a third consequence keyed 27, and two more decisions.
fn decisions() -> Document {
    let mut document = small_document();
    let mut third = document.decisions[0].consequences[1].clone();
    third.id = loftspec::ids::ConsequenceId::new(key(27));
    document.decisions[0].consequences.push(third);
    for (n, slug) in [(80, "second-decision"), (81, "third-decision")] {
        let decision = crate::common::proposed_decision(key(n), slug);
        document.decisions.push(decision);
    }
    document
}

fn reorder_consequences(document: &Document, keys: &[u32]) -> Document {
    let mut document = document.clone();
    let list = &mut document.decisions[0].consequences;
    list.sort_by_key(|each| {
        let text = each.id.key().to_string();
        let n = u32::from_str_radix(&text[text.len() - 8..], 16).unwrap();
        keys.iter().position(|each| *each == n)
    });
    document
}

#[test]
fn settling_a_position_conflict_of_a_decisions_part_moves_it_within_its_list() {
    let base = decisions();
    let ours = reorder_consequences(&base, &[27, 25, 26]);
    let theirs = reorder_consequences(&base, &[25, 27, 26]);
    let document = merge(&base, &ours, &theirs);
    assert_eq!(consequences(&document, 0), [25, 26, 27]);
    let ours = settled(&document, "position", &Settlement::Ours);
    assert_eq!(consequences(&ours, 0), [27, 25, 26]);
    let theirs = settled(&document, "position", &Settlement::Theirs);
    assert_eq!(consequences(&theirs, 0), [25, 27, 26]);
}

#[test]
fn settling_a_parent_conflict_of_a_decisions_part_moves_it_to_the_sides_decision() {
    let base = decisions();
    let moved_to = |at: usize| {
        let mut document = base.clone();
        let part = document.decisions[0].consequences.remove(2);
        document.decisions[at].consequences.push(part);
        document
    };
    let document = merge(&base, &moved_to(1), &moved_to(2));
    assert_eq!(consequences(&document, 0), [25, 26, 27]);
    let theirs = settled(&document, "parent", &Settlement::Theirs);
    assert_eq!(consequences(&theirs, 0), [25, 26]);
    assert_eq!(consequences(&theirs, 2), [27]);
}
