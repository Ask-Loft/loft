//! A copy merges as one value, as `spec/lint.yaml` does: one side's move of the pin wins, equal
//! moves coalesce, and two sides that move one pin apart are refused.

use crate::common::copies::{COPY, HANDLE, copy_at, owner, requiring};
use crate::common::text::path;
use loftspec::codec::{self, Files};
use loftspec::merge::{MergeError, TreeMergeError, merge_file, merge_tree};

/// The requiring document's files with its copy pinned at `sequence`.
fn pinned(sequence: u64) -> Files {
    let mut document = requiring();
    document.copies = vec![copy_at(&owner(), HANDLE, sequence)];
    codec::write(&document)
}

fn copy_bytes(files: &Files) -> &[u8] {
    &files[&path(COPY)]
}

#[test]
fn a_pin_that_one_side_moved_merges_to_the_moved_copy() {
    let (base, moved) = (pinned(3), pinned(4));
    let merged = merge_tree(&base, &moved, &base).unwrap();
    assert_eq!(copy_bytes(&merged.files), copy_bytes(&moved));
    assert_eq!(merged.document.copies[0].publish_sequence, 4);
    let merged = merge_file(
        &path(COPY),
        copy_bytes(&base),
        copy_bytes(&base),
        copy_bytes(&moved),
    );
    assert_eq!(merged.unwrap().bytes, copy_bytes(&moved));
}

#[test]
fn a_pin_that_both_sides_moved_apart_is_refused() {
    let (base, ours, theirs) = (pinned(3), pinned(4), pinned(5));
    let refused = merge_tree(&base, &ours, &theirs).unwrap_err();
    assert_eq!(
        refused,
        TreeMergeError::File {
            path: path(COPY),
            error: MergeError::BothChanged
        }
    );
    let file = merge_file(
        &path(COPY),
        copy_bytes(&base),
        copy_bytes(&ours),
        copy_bytes(&theirs),
    );
    assert_eq!(file.unwrap_err(), MergeError::BothChanged);
    let same = merge_tree(&base, &ours, &ours).unwrap();
    assert_eq!(copy_bytes(&same.files), copy_bytes(&ours));
}
