//! Properties of the tree merger over random edits: every tree it returns reads into a document.
//! Each case is either a file of the full document edited at random, as the merge properties
//! edit it, set into the full document's tree on each side; or the whole document edited in
//! memory, with deletions that take every reference to what they delete with them and edges
//! that each side adds.

use loftspec::codec::{self, Files, SpecPath};
use loftspec::merge::{TreeMergeError, merge_tree};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

use crate::common::doc_edits::{doc_edit, doc_edited};
use crate::common::full::full_nested_document;
use crate::common::merge_cases::{cases, fixture_files};

/// The full document's tree with the file at `path` replaced by `bytes`.
fn with_file(path: &str, bytes: &[u8]) -> Files {
    let mut files = fixture_files().clone();
    files.insert(SpecPath::new(path), bytes.to_vec());
    files
}

/// Asserts that a merge of three trees that each read gives a tree that reads, and never refuses
/// its result as one that does not read. `refusals` says whether the merge may refuse the trees
/// otherwise: a random edit of a file may change a record's kind or slug, so that no place takes
/// a record or two records fall in one file; an edit of a document in memory does neither.
fn reads(base: &Files, ours: &Files, theirs: &Files, refusals: bool) -> Result<(), TestCaseError> {
    for side in [base, ours, theirs] {
        prop_assume!(codec::read(side).is_ok());
    }
    let merged = match merge_tree(base, ours, theirs) {
        Ok(merged) => merged,
        Err(TreeMergeError::Unwritable(errors)) => {
            return Err(TestCaseError::fail(format!(
                "the result does not read: {errors:?}"
            )));
        }
        Err(refusal) => {
            prop_assert!(refusals, "{refusal}");
            return Ok(());
        }
    };
    let read = codec::read(&merged.files);
    prop_assert!(read.is_ok(), "{:?}", read.err());
    Ok(())
}

proptest! {
    #[test]
    fn every_tree_the_merger_gives_for_edited_files_reads(case in cases()) {
        let path = case.fixture.path;
        reads(
            &with_file(path, &case.base),
            &with_file(path, &case.ours),
            &with_file(path, &case.theirs),
            true,
        )?;
    }

    #[test]
    fn every_tree_the_merger_gives_for_edited_documents_reads(
        grown in proptest::collection::vec(doc_edit(), 0..=4),
        ours in proptest::collection::vec(doc_edit(), 0..=6),
        theirs in proptest::collection::vec(doc_edit(), 0..=6),
    ) {
        let base = doc_edited(&full_nested_document(), &grown);
        let [ours, theirs] = [ours, theirs].map(|edits| doc_edited(&base, &edits));
        reads(&codec::write(&base), &codec::write(&ours), &codec::write(&theirs), false)?;
    }
}
