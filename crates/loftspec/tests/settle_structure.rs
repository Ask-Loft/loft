//! Settling a conflict over `record`, `parent` or `position` of a document held in memory
//! (Settling a conflict): a record kept or deleted, moved under another parent, or moved within
//! its list, each by a side only.

use loftspec::conflict::{Conflict, ConflictValue, SettleError, Settlement, settle};
use loftspec::document::Document;
use loftspec::ids::{ElementId, RequirementId};
use loftspec::merge::Input;

use crate::common::merge_tree::{moved, requirements, three_features, two_features};
use crate::common::settle::{
    SECOND, THIRD, merge, over, refusal, settled, three_requirements, with_requirements,
};
use crate::common::{key, key_number, label, requirement, small_document};

/// The keys of the Requirements of the group's Feature at `at`.
fn keys_at(document: &Document, at: usize) -> Vec<u32> {
    let mut document = document.clone();
    requirements(&mut document, at)
        .iter()
        .map(|each| key_number(each.id.unwrap().key()))
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
    let mut document = kept_for_a_mention();
    let second = RequirementId::new(key(SECOND));
    document.decisions[0].consequences[1].subject = Some(second.into());
    let document = loftspec::codec::read(&loftspec::codec::write(&document)).unwrap();
    let refused = refusal(&document, "record", &Settlement::Theirs);
    let SettleError::Unreadable(errors) = refused else {
        panic!("{refused:?}");
    };
    let mut deleted = document.clone();
    loftspec::edit::detach(&mut deleted, second.into()).unwrap();
    deleted.edges.retain(|edge| !names(edge, SECOND));
    deleted.bindings.depart(second).unwrap();
    deleted.conflicts.clear();
    let expected = loftspec::codec::read(&loftspec::codec::write(&deleted)).unwrap_err();
    assert_eq!(errors, expected);
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
        .map(|each| key_number(each.id.key()))
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
    list.sort_by_key(|each| keys.iter().position(|n| *n == key_number(each.id.key())));
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

/// A conflict over the existence of the Requirement keyed 60, bound to `RQ-3`, that the tree
/// merger kept because ours made the criterion keyed 5 mention it while theirs deleted it.
fn kept_for_a_mention() -> Document {
    let mut base = three_requirements();
    let second = RequirementId::new(key(SECOND));
    base.bindings.bind(second, label("RQ-3")).unwrap();
    let mut ours = base.clone();
    ours.edges.push(loftspec::document::Edge::Mentions {
        source: loftspec::ids::CriterionId::new(key(5)).into(),
        target: second.into(),
    });
    let mut theirs = base.clone();
    requirements(&mut theirs, 0).remove(1);
    merge(&base, &ours, &theirs)
}

fn names(edge: &loftspec::document::Edge, n: u32) -> bool {
    edge.target()
        == loftspec::document::EdgeTarget::Element(ElementId::Node(
            RequirementId::new(key(n)).into(),
        ))
}

#[test]
fn settling_a_record_conflict_by_deletion_removes_every_edge_that_names_what_it_deletes() {
    let document = kept_for_a_mention();
    assert!(document.edges.iter().any(|edge| names(edge, SECOND)));
    let deleted = settled(&document, "record", &Settlement::Theirs);
    assert_eq!(keys_at(&deleted, 0), [4, THIRD]);
    assert!(!deleted.edges.iter().any(|edge| names(edge, SECOND)));
    assert_eq!(deleted.edges.len(), document.edges.len() - 1);
}

/// [`kept_for_a_mention`] with a retired decision keyed 82 whose consequence and subject name the
/// Requirement keyed 60.
fn named_by_a_retired_decision() -> Document {
    let mut document = kept_for_a_mention();
    let mut retired = crate::common::proposed_decision(key(82), "retired-decision");
    let mut option = document.decisions[0].options[0].clone();
    option.id = loftspec::ids::OptionId::new(key(83));
    retired.state = loftspec::decision::DecisionState::Retired {
        selected_option: option.id,
        outcome: "Kept for a while.".to_owned(),
        rationale: "No longer needed.".to_owned(),
    };
    retired.options = vec![option];
    let mut consequence = document.decisions[0].consequences[1].clone();
    consequence.id = loftspec::ids::ConsequenceId::new(key(84));
    consequence.subject = Some(RequirementId::new(key(SECOND)).into());
    retired.consequences = vec![consequence];
    document.edges.push(loftspec::document::Edge::Subject {
        decision: retired.id,
        subject: RequirementId::new(key(SECOND)).into(),
        role: loftspec::document::SubjectRole::Affected,
    });
    document.decisions.push(retired);
    loftspec::codec::read(&loftspec::codec::write(&document)).expect("the document reads")
}

#[test]
fn settling_by_deletion_names_a_node_a_retired_decision_references_by_its_former_label() {
    let document = named_by_a_retired_decision();
    let deleted = settled(&document, "record", &Settlement::Theirs);
    let former = loftspec::decision::NodeRef::Former(label("RQ-3"));
    let retired = deleted
        .decisions
        .iter()
        .find(|each| each.id.key() == key(82));
    assert_eq!(retired.unwrap().consequences[0].subject, Some(former));
    let subject = loftspec::document::Edge::Subject {
        decision: loftspec::ids::DecisionId::new(key(82)),
        subject: former,
        role: loftspec::document::SubjectRole::Affected,
    };
    assert!(deleted.edges.contains(&subject), "{:?}", deleted.edges);
}

#[test]
fn settling_by_deletion_takes_the_conflict_records_of_what_it_deletes_with_them() {
    let mut document = with_requirements(&small_document(), 0, &[SECOND]);
    let criterion = crate::common::criterion(key(62), "A slug is written.");
    requirements(&mut document, 0)[1].criteria.push(criterion);
    let owner = |n: u32, kind: fn(loftspec::identity::Key) -> loftspec::ids::NodeId| {
        loftspec::conflict::ConflictOwner::Element(ElementId::Node(kind(key(n))))
    };
    let held = || Some(ConflictValue::Scalar(key(SECOND).to_string()));
    document.conflicts = vec![
        Conflict {
            owner: owner(SECOND, |key| RequirementId::new(key).into()),
            field: loftspec::conflict::FieldPath::new("record"),
            base: held(),
            ours: held(),
            theirs: Some(ConflictValue::Deleted),
        },
        Conflict {
            owner: owner(62, |key| loftspec::ids::CriterionId::new(key).into()),
            field: loftspec::conflict::FieldPath::new("name"),
            base: None,
            ours: Some(ConflictValue::Scalar("Ours.".to_owned())),
            theirs: Some(ConflictValue::Scalar("Theirs.".to_owned())),
        },
        Conflict {
            owner: owner(4, |key| RequirementId::new(key).into()),
            field: loftspec::conflict::FieldPath::new("body"),
            base: None,
            ours: Some(ConflictValue::Scalar("Ours.".to_owned())),
            theirs: None,
        },
    ];
    let document = loftspec::codec::read(&loftspec::codec::write(&document)).unwrap();
    let kept = over(&document, "body").clone();
    let deleted = settled(&document, "record", &Settlement::Theirs);
    assert_eq!(deleted.conflicts, [kept]);
}
