//! The merge properties under `merge_file_by_key`, which is to replace `merge_file`: every law
//! of `merge_props.rs`, leaving out only the asymmetry both merges keep by design; and the two
//! merges compared, which must give the same on every case but their known differences.

use loftspec::merge::{MergeError, Merged, merge_file, merge_file_by_key};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

use crate::common::merge_cases::{Case, cases};
use crate::common::merge_keys::Versions;
use crate::common::merge_known::known_difference;
use crate::merge_props::{
    BY_KEY, config, conserves, counts, idempotent, reads_back, refuses_nested_moves, symmetric,
};

/// Whether both merges give the same for the case's base and the two versions `ours` and
/// `theirs`, unless it is a known difference.
fn agree(case: &Case, ours: &[u8], theirs: &[u8]) -> Result<(), TestCaseError> {
    let versions = Versions::of(case.fixture.top, [&case.base, ours, theirs]);
    if known_difference(&versions).is_some() {
        return Ok(());
    }
    let file_merge = case.merge(merge_file, &case.base, ours, theirs);
    if refuses_nested_moves(&file_merge) {
        return Ok(());
    }
    let by_key = case.merge(merge_file_by_key, &case.base, ours, theirs);
    let text = |merged: &Result<Merged, MergeError>| match merged {
        Ok(merged) => format!(
            "{} conflicts:\n{}",
            merged.conflicts,
            String::from_utf8_lossy(&merged.bytes)
        ),
        Err(error) => format!("{error:?}"),
    };
    prop_assert!(
        by_key == file_merge,
        "the merge by key gives {}\nwhere the file merge gives {}\nof ours\n{}\ntheirs\n{}\n{:?}",
        text(&by_key),
        text(&file_merge),
        String::from_utf8_lossy(ours),
        String::from_utf8_lossy(theirs),
        case
    );
    Ok(())
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn a_side_merged_with_an_unchanged_side_or_with_itself_is_that_side(case in cases()) {
        idempotent(&case, BY_KEY)?;
    }

    #[test]
    fn a_merge_reads_back_as_a_formatter_fixed_point_carrying_each_key_once(case in cases()) {
        reads_back(&case, BY_KEY)?;
    }

    #[test]
    fn a_merge_counts_the_conflict_records_it_writes(case in cases()) {
        counts(&case, BY_KEY)?;
    }

    #[test]
    fn a_merge_keeps_the_keys_both_sides_hold_or_one_adds_and_drops_those_one_deletes_and_the_other_leaves_untouched(case in cases()) {
        conserves(&case, BY_KEY)?;
    }

    #[test]
    fn swapping_the_sides_keeps_the_conflicts_the_keys_and_each_record_s_parent_and_fields(case in cases()) {
        symmetric(&case, BY_KEY)?;
    }

    #[test]
    fn the_merge_by_key_gives_what_the_file_merge_gives_but_for_its_known_differences(case in cases()) {
        let (base, ours, theirs) = (&case.base, &case.ours, &case.theirs);
        let pairs = [(ours, theirs), (theirs, ours), (ours, ours), (theirs, theirs), (ours, base), (base, theirs)];
        for (one, other) in pairs {
            agree(&case, one, other)?;
        }
    }
}
