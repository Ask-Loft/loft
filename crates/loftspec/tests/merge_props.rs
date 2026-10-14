//! Properties of the file merge over random edits of the full document's files: a Feature
//! file, a Component file, a Product file and the tag registry. Each case is a base made by
//! random edits and two sides made from it by independent random edits. Each law runs here
//! under `merge_file`, leaving out only the asymmetry both merges keep by design, and under
//! `merge_file_legacy`, leaving out as well the classes of case the legacy merge gets wrong.

use loftspec::codec::schema::field;
use loftspec::merge::{MergeError, Merged, merge_file, merge_file_legacy};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

use crate::common::edit_tree::{Y, conflicts, read};
use crate::common::merge_cases::{Case, Engine, cases, unsettled};
use crate::common::merge_defects::{
    a_key_changes_kind, a_side_deletes_beneath_a_record_the_other_side_deleted,
    a_side_renames_an_entry_and_the_other_deletes_one, moves_a_record_into_a_list_it_also_reorders,
    moves_a_record_into_one_it_held, moves_on_the_two_sides_form_a_cycle,
};
use crate::common::merge_keys::{Flat, Kept, deletions, flatten};

/// The merge a law runs, and the cases it merges as `merge_file_legacy` does, on which the law
/// leaves out the classes of case the legacy merge gets wrong.
#[derive(Clone, Copy)]
pub struct Under {
    pub engine: Engine,
    pub legacy: fn(&Case) -> bool,
}

pub const LEGACY: Under = Under {
    engine: merge_file_legacy,
    legacy: |_| true,
};

pub const FILE: Under = Under {
    engine: merge_file,
    legacy: |_| false,
};

impl Under {
    /// Whether the law runs the case under the legacy merge.
    fn legacy(self, case: &Case) -> bool {
        (self.legacy)(case)
    }
}

pub fn config() -> ProptestConfig {
    ProptestConfig {
        // `PROPTEST_CASES` runs more, as when hunting for a new class of failure.
        cases: std::env::var("PROPTEST_CASES")
            .ok()
            .and_then(|cases| cases.parse().ok())
            .unwrap_or(256),
        ..ProptestConfig::default()
    }
}

/// How many conflict records a version of the case's file carries.
fn carried(case: &Case, bytes: &[u8]) -> usize {
    conflicts(&read(case.fixture.top, bytes))
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn every_generated_version_reads_back_and_is_a_formatter_fixed_point(case in cases()) {
        for bytes in [&case.base, &case.ours, &case.theirs] {
            let problem = unsettled(case.fixture.path, bytes);
            prop_assert!(problem.is_none(), "{}\n{case:?}", problem.unwrap_or_default());
        }
    }

    #[test]
    fn a_side_merged_with_an_unchanged_side_or_with_itself_is_that_side(case in cases()) {
        for under in [FILE, LEGACY] {
            idempotent(&case, under)?;
        }
    }
}

/// A side merged with base either way round, or with itself, is that side.
pub fn idempotent(case: &Case, under: Under) -> Result<(), TestCaseError> {
    let versions = case.versions();
    prop_assume!(!under.legacy(case) || !a_key_changes_kind(&versions));
    for (side, flat) in [
        (&case.ours, &versions.ours),
        (&case.theirs, &versions.theirs),
    ] {
        let mut pairs = vec![(side, &case.base), (&case.base, side)];
        let defect = moves_a_record_into_one_it_held(&versions.base, flat)
            || moves_a_record_into_a_list_it_also_reorders(&versions.base, flat);
        if !under.legacy(case) || !defect {
            pairs.push((side, side));
        }
        for (ours, theirs) in pairs {
            gives(case, under, ours, theirs, side)?;
        }
    }
    Ok(())
}

/// Whether merging `ours` and `theirs` over the case's base gives `expected`, with only the
/// conflict records it carries.
fn gives(
    case: &Case,
    under: Under,
    ours: &[u8],
    theirs: &[u8],
    expected: &[u8],
) -> Result<(), TestCaseError> {
    let merged = case.merge(under.engine, &case.base, ours, theirs);
    prop_assert!(merged.is_ok(), "{merged:?}\n{case:?}");
    let merged = merged.unwrap();
    prop_assert_eq!(
        String::from_utf8_lossy(&merged.bytes),
        String::from_utf8_lossy(expected),
        "{:?}",
        case
    );
    prop_assert_eq!(merged.conflicts, carried(case, expected));
    Ok(())
}

fn flat(case: &Case, bytes: &[u8]) -> Flat {
    flatten(case.fixture.top, bytes)
}

/// The legacy merge: moves on the two sides that nest records into records moved or deleted
/// beneath them, short of a cycle, may make the move pre-pass refuse a key it then carries
/// twice; the merge by key has no such refusal. Lifted when the legacy merge is removed.
pub fn refuses_nested_moves(merged: &Result<Merged, MergeError>) -> bool {
    matches!(merged, Err(MergeError::DuplicateKey(_)))
}

/// The merge of a case, which must write a file.
fn merged(case: &Case, under: Under) -> Result<Merged, TestCaseError> {
    if under.legacy(case) {
        let versions = case.versions();
        prop_assume!(!a_key_changes_kind(&versions));
        prop_assume!(!moves_on_the_two_sides_form_a_cycle(&versions));
    }
    let merged = case.merged(under.engine);
    prop_assume!(!under.legacy(case) || !refuses_nested_moves(&merged));
    prop_assert!(merged.is_ok(), "{merged:?}\n{case:?}");
    Ok(merged.unwrap())
}

/// Keys both sides hold, or one side added, survive; keys only base holds are gone; and so is a
/// key one side deleted where the other side left it, and everything beneath it, untouched.
fn keys_conserved(case: &Case, merged: &Merged) -> Result<(), TestCaseError> {
    let [base, ours, theirs] =
        [&case.base, &case.ours, &case.theirs].map(|bytes| flat(case, bytes));
    let result = flat(case, &merged.bytes);
    let written = String::from_utf8_lossy(&merged.bytes);
    for key in ours.keys().union(&theirs.keys()) {
        let survives = (ours.has(key) && theirs.has(key)) || !base.has(key);
        if survives {
            prop_assert!(result.has(key), "{key} is lost\n{written}\n{case:?}");
        }
    }
    for key in base.keys() {
        if !ours.has(key) && !theirs.has(key) {
            prop_assert!(
                !result.has(key),
                "{key} is brought back\n{written}\n{case:?}"
            );
        }
    }
    for (key, kept) in deletions(&base, &ours, &theirs) {
        if kept != Kept::Changed {
            prop_assert!(
                !result.has(&key),
                "{key} is brought back\n{written}\n{case:?}"
            );
        }
    }
    Ok(())
}

/// Whether each record of base the merge keeps has the same parent and, where base has a
/// field, the same field in both merges. Records base lacks are left out: ours hosts a record
/// both sides added, and ours wins a field base lacks.
fn agree(base: &Flat, one: &Flat, other: &Flat) -> bool {
    one.keyed.iter().all(|(key, record)| {
        let (Some(before), Some(swapped)) = (base.keyed.get(key), other.keyed.get(key)) else {
            return !base.has(key);
        };
        record.place.0 == swapped.place.0 && agree_where(&before.own, &record.own, &swapped.own)
    })
}

fn agree_where(base: &Y, one: &Y, other: &Y) -> bool {
    let (Y::Map(fields), Y::Map(_), Y::Map(_)) = (base, one, other) else {
        return one == other;
    };
    fields
        .iter()
        .all(|(name, before)| match (one.get(name), other.get(name)) {
            (Some(one), Some(other)) => agree_where(before, one, other),
            (one, other) => one == other,
        })
}

/// Whether both sides add one key under different parents: ours hosts it, and which side's
/// parent survives, and so how many conflicts the merge writes, follows from that, by design.
fn both_sides_add_a_key_apart(case: &Case) -> bool {
    let [base, ours, theirs] =
        [&case.base, &case.ours, &case.theirs].map(|bytes| flat(case, bytes));
    ours.keyed.iter().any(|(key, added)| {
        !base.has(key)
            && theirs
                .keyed
                .get(key)
                .is_some_and(|other| other.place != added.place)
    })
}

/// Swapping the sides keeps the conflicts, the keys and, in a merge without conflicts, each
/// record's parent and fields.
pub fn symmetric(case: &Case, under: Under) -> Result<(), TestCaseError> {
    if under.legacy(case) {
        let versions = case.versions();
        prop_assume!(!a_key_changes_kind(&versions));
        prop_assume!(!moves_on_the_two_sides_form_a_cycle(&versions));
    }
    prop_assume!(!both_sides_add_a_key_apart(case));
    let (one, other) = (case.merged(under.engine), case.swapped(under.engine));
    prop_assume!(
        !under.legacy(case) || !(refuses_nested_moves(&one) || refuses_nested_moves(&other))
    );
    prop_assert_eq!(
        one.is_ok(),
        other.is_ok(),
        "{:?}\n{:?}\n{:?}",
        one,
        other,
        case
    );
    let (Ok(one), Ok(other)) = (one, other) else {
        return Ok(());
    };
    prop_assert_eq!(one.conflicts, other.conflicts, "{:?}", case);
    let (one, other) = (flat(case, &one.bytes), flat(case, &other.bytes));
    prop_assert_eq!(one.keys(), other.keys(), "{:?}", case);
    if conflict_free(&one) && conflict_free(&other) {
        let base = flat(case, &case.base);
        prop_assert!(agree(&base, &one, &other), "{one:#?}\n{other:#?}\n{case:?}");
    }
    Ok(())
}

fn conflict_free(flat: &Flat) -> bool {
    flat.keyed
        .values()
        .all(|record| record.own.get(field::CONFLICTS).is_none())
}

/// A merge reads back as a formatter fixed point and carries each key once.
pub fn reads_back(case: &Case, under: Under) -> Result<(), TestCaseError> {
    let merged = merged(case, under)?;
    let problem = unsettled(case.fixture.path, &merged.bytes);
    prop_assert!(
        problem.is_none(),
        "{}\n{case:?}",
        problem.unwrap_or_default()
    );
    let twice = flat(case, &merged.bytes).twice;
    prop_assert!(twice.is_empty(), "{twice:?}\n{case:?}");
    Ok(())
}

/// A merge counts the conflict records it writes.
pub fn counts(case: &Case, under: Under) -> Result<(), TestCaseError> {
    let merged = merged(case, under)?;
    prop_assert_eq!(merged.conflicts, carried(case, &merged.bytes));
    Ok(())
}

/// A merge keeps and drops keys as `keys_conserved` says.
pub fn conserves(case: &Case, under: Under) -> Result<(), TestCaseError> {
    if under.legacy(case) {
        let versions = case.versions();
        prop_assume!(!a_side_deletes_beneath_a_record_the_other_side_deleted(
            &versions
        ));
        prop_assume!(!a_side_renames_an_entry_and_the_other_deletes_one(
            &versions
        ));
    }
    let merged = merged(case, under)?;
    keys_conserved(case, &merged)
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn a_merge_reads_back_as_a_formatter_fixed_point_carrying_each_key_once(case in cases()) {
        for under in [FILE, LEGACY] {
            reads_back(&case, under)?;
        }
    }

    #[test]
    fn a_merge_counts_the_conflict_records_it_writes(case in cases()) {
        for under in [FILE, LEGACY] {
            counts(&case, under)?;
        }
    }

    #[test]
    fn a_merge_keeps_the_keys_both_sides_hold_or_one_adds_and_drops_those_one_deletes_and_the_other_leaves_untouched(case in cases()) {
        for under in [FILE, LEGACY] {
            conserves(&case, under)?;
        }
    }

    #[test]
    fn swapping_the_sides_keeps_the_conflicts_the_keys_and_each_record_s_parent_and_fields(case in cases()) {
        for under in [FILE, LEGACY] {
            symmetric(&case, under)?;
        }
    }
}
