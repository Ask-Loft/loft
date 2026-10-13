//! The merge of one ordered list (D-11 F-33): both sides' insertions kept in base order, a
//! move on one side applied, and a record both sides moved apart left at its base position with
//! a conflict over its place.

use std::collections::BTreeSet;

use loftspec::merge::order::{Anchor, PositionConflict, merge_order};
use proptest::prelude::*;

/// The standard rule a caller gives for elements that both sides leave unchanged: an element
/// of base that either side deleted is removed; every other element survives.
fn merged<'a>(
    base: &[&'a str],
    ours: &[&'a str],
    theirs: &[&'a str],
) -> (Vec<&'a str>, Vec<PositionConflict<&'a str>>) {
    let survives = |element: &&str| {
        !base.contains(element) || (ours.contains(element) && theirs.contains(element))
    };
    let merged = merge_order(base, ours, theirs, &survives);
    (merged.order, merged.conflicts)
}

fn order<'a>(base: &[&'a str], ours: &[&'a str], theirs: &[&'a str]) -> Vec<&'a str> {
    let (order, conflicts) = merged(base, ours, theirs);
    assert_eq!(conflicts, Vec::new());
    order
}

#[test]
fn each_side_s_insertion_follows_its_base_predecessor() {
    assert_eq!(
        order(
            &["a", "b", "c"],
            &["a", "x", "b", "c"],
            &["a", "b", "c", "y"]
        ),
        ["a", "x", "b", "c", "y"]
    );
}

#[test]
fn no_existing_element_is_reordered() {
    assert_eq!(
        order(
            &["a", "b", "c", "d"],
            &["a", "b", "x", "c", "d"],
            &["y", "a", "b", "c", "d"]
        ),
        ["y", "a", "b", "x", "c", "d"]
    );
}

#[test]
fn an_element_deleted_on_one_side_and_unmoved_on_the_other_is_removed() {
    assert_eq!(
        order(&["a", "b", "c"], &["a", "c"], &["a", "b", "c"]),
        ["a", "c"]
    );
}

#[test]
fn when_both_sides_insert_after_one_base_element_our_insertions_precede_theirs() {
    assert_eq!(
        order(&["a", "b"], &["a", "x", "x2", "b"], &["a", "y", "b"]),
        ["a", "x", "x2", "y", "b"]
    );
}

#[test]
fn a_record_one_side_moved_is_at_the_moved_position() {
    assert_eq!(
        order(&["a", "b", "c"], &["c", "a", "b"], &["a", "b", "c"]),
        ["c", "a", "b"]
    );
    assert_eq!(
        order(&["a", "b", "c"], &["a", "b", "c"], &["a", "c", "b"]),
        ["a", "c", "b"]
    );
}

#[test]
fn a_record_both_sides_moved_apart_conflicts_and_stays_at_its_base_position() {
    let (order, conflicts) = merged(&["a", "b", "c"], &["c", "a", "b"], &["a", "c", "b"]);
    assert_eq!(order, ["a", "b", "c"]);
    assert_eq!(
        conflicts,
        [PositionConflict {
            element: "c",
            base: Some(Anchor::After("b")),
            ours: Some(Anchor::First),
            theirs: Some(Anchor::After("a")),
        }]
    );
}

#[test]
fn a_record_both_sides_moved_to_one_place_is_moved_without_a_conflict() {
    assert_eq!(
        order(&["a", "b", "c"], &["c", "a", "b"], &["c", "a", "b"]),
        ["c", "a", "b"]
    );
}

#[test]
fn an_insertion_whose_predecessor_the_other_side_deleted_follows_the_nearest_survivor() {
    assert_eq!(
        order(&["a", "b", "c"], &["a", "b", "x", "c"], &["a", "c"]),
        ["a", "x", "c"]
    );
}

#[test]
fn an_insertion_follows_its_predecessor_where_the_other_side_moved_it() {
    assert_eq!(
        order(&["a", "b", "c"], &["a", "b", "x", "c"], &["b", "a", "c"]),
        ["b", "x", "a", "c"]
    );
}

#[test]
fn an_element_both_sides_insert_is_kept_once() {
    assert_eq!(order(&["a"], &["a", "x"], &["a", "x"]), ["a", "x"]);
}

#[test]
fn an_element_one_side_moved_and_the_other_deleted_stays_where_the_mover_put_it_if_it_survives() {
    let base = ["a", "b", "c"];
    let ours = ["c", "a", "b"];
    let theirs = ["a", "b"];
    let kept = merge_order(&base, &ours, &theirs, &|_| true);
    assert_eq!(kept.order, ["c", "a", "b"]);
    let removed = merge_order(&base, &ours, &theirs, &|element| *element != "c");
    assert_eq!(removed.order, ["a", "b"]);
}

#[test]
fn moves_that_would_place_each_element_after_the_other_pin_both_at_their_base_position() {
    let (order, conflicts) = merged(&["x", "m", "y"], &["x", "y", "m"], &["m", "y", "x"]);
    assert_eq!(order, ["x", "m", "y"]);
    let pinned: Vec<&str> = conflicts.iter().map(|conflict| conflict.element).collect();
    assert!(
        !pinned.is_empty(),
        "a conflict for the moves that cannot both hold"
    );
}

/// A list of distinct small numbers, and two edits of it: each keeps a random subset in a
/// random order and adds new numbers.
fn edits() -> impl Strategy<Value = (Vec<u8>, Vec<u8>, Vec<u8>)> {
    let side = |base: Vec<u8>, start: u8| {
        (
            Just(base.clone()).prop_shuffle(),
            proptest::collection::vec(any::<bool>(), base.len()),
            proptest::collection::vec(start..start + 20, 0..4),
        )
            .prop_map(|(shuffled, keep, added)| {
                let kept = shuffled
                    .into_iter()
                    .zip(keep)
                    .filter_map(|(element, kept)| kept.then_some(element));
                let added: BTreeSet<u8> = added.into_iter().collect();
                kept.chain(added).collect::<Vec<u8>>()
            })
    };
    proptest::collection::btree_set(0u8..40, 0..8)
        .prop_map(|set| set.into_iter().collect::<Vec<u8>>())
        .prop_flat_map(move |base| (Just(base.clone()), side(base.clone(), 100), side(base, 150)))
}

fn standard(base: &[u8], ours: &[u8], theirs: &[u8]) -> Vec<u8> {
    let survives = |element: &u8| {
        !base.contains(element) || (ours.contains(element) && theirs.contains(element))
    };
    merge_order(base, ours, theirs, &survives).order
}

proptest! {
    #[test]
    fn a_side_merged_with_an_unchanged_side_is_that_side((base, ours, theirs) in edits()) {
        prop_assert_eq!(standard(&base, &ours, &base), ours.clone());
        prop_assert_eq!(standard(&base, &base, &theirs), theirs.clone());
    }

    #[test]
    fn two_equal_sides_merge_to_that_side((base, ours, _theirs) in edits()) {
        prop_assert_eq!(standard(&base, &ours, &ours), ours.clone());
    }

    #[test]
    fn every_survivor_appears_exactly_once((base, ours, theirs) in edits()) {
        let order = standard(&base, &ours, &theirs);
        let unique: BTreeSet<u8> = order.iter().copied().collect();
        prop_assert_eq!(unique.len(), order.len());
        for element in ours.iter().chain(&theirs) {
            let survives = !base.contains(element) || (ours.contains(element) && theirs.contains(element));
            prop_assert_eq!(order.contains(element), survives);
        }
    }

    #[test]
    fn elements_neither_side_moved_keep_their_base_order((base, ours, theirs) in edits()) {
        let order = standard(&base, &ours, &theirs);
        let positions: Vec<usize> = base
            .iter()
            .filter(|element| ours.contains(element) && theirs.contains(element))
            .filter(|element| unmoved(&base, &ours, **element) && unmoved(&base, &theirs, **element))
            .filter_map(|element| order.iter().position(|each| each == element))
            .collect();
        prop_assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

/// Whether `element` keeps the same order against every other element both lists hold.
fn unmoved(base: &[u8], side: &[u8], element: u8) -> bool {
    let at = |list: &[u8], element: u8| list.iter().position(|each| *each == element);
    base.iter()
        .filter(|other| **other != element && side.contains(other))
        .all(|other| {
            (at(base, element) < at(base, *other)) == (at(side, element) < at(side, *other))
        })
}
