//! The merge by key, which `merge_file` now is, compared with `merge_file_legacy`: the two must
//! give the same on every case but their known differences. The laws of the merge run in
//! `merge_props.rs`.

use loftspec::merge::{MergeError, Merged, merge_file, merge_file_legacy};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

use crate::common::merge_cases::{Case, cases};
use crate::common::merge_keys::Versions;
use crate::common::merge_known::known_difference;
use crate::merge_props::{config, refuses_nested_moves};

/// Whether both merges give the same for the case's base and the two versions `ours` and
/// `theirs`, unless it is a known difference.
fn agree(case: &Case, ours: &[u8], theirs: &[u8]) -> Result<(), TestCaseError> {
    let versions = Versions::of(case.fixture.top, [&case.base, ours, theirs]);
    if known_difference(&versions).is_some() {
        return Ok(());
    }
    let legacy = case.merge(merge_file_legacy, &case.base, ours, theirs);
    if refuses_nested_moves(&legacy) {
        return Ok(());
    }
    let by_key = case.merge(merge_file, &case.base, ours, theirs);
    let text = |merged: &Result<Merged, MergeError>| match merged {
        Ok(merged) => format!(
            "{} conflicts:\n{}",
            merged.conflicts,
            String::from_utf8_lossy(&merged.bytes)
        ),
        Err(error) => format!("{error:?}"),
    };
    prop_assert!(
        by_key == legacy,
        "the merge by key gives {}\nwhere the legacy merge gives {}\nof ours\n{}\ntheirs\n{}\n{:?}",
        text(&by_key),
        text(&legacy),
        String::from_utf8_lossy(ours),
        String::from_utf8_lossy(theirs),
        case
    );
    Ok(())
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn the_merge_by_key_gives_what_the_file_merge_gives_but_for_its_known_differences(case in cases()) {
        let (base, ours, theirs) = (&case.base, &case.ours, &case.theirs);
        let pairs = [(ours, theirs), (theirs, ours), (ours, ours), (theirs, theirs), (ours, base), (base, theirs)];
        for (one, other) in pairs {
            agree(&case, one, other)?;
        }
    }
}
