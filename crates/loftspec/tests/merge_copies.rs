//! A copy merges as one value, as `spec/lint.yaml` does: one side's move of the pin wins, equal
//! moves coalesce, and two sides that move one pin apart are refused.

use crate::common::copies::{COPY, HANDLE, copy_at, owner, requiring};
use crate::common::merge::merged_at;
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

/// The requiring document's manifest, with the address of its entry of `documents` written as
/// `address`.
fn listing(address: &str) -> String {
    let files = codec::write(&requiring());
    let manifest = String::from_utf8(files[&path(MANIFEST)].clone()).unwrap();
    let base = format!("https://remote.example/documents/{HANDLE}");
    assert!(manifest.contains(&base), "{manifest}");
    manifest.replace(&base, address)
}

const MANIFEST: &str = "loftspec.yaml";

#[test]
fn an_entry_of_documents_whose_address_both_sides_changed_apart_is_a_conflict() {
    let base = listing(&format!("https://remote.example/documents/{HANDLE}"));
    let ours = listing("https://ours.example/documents/format");
    let theirs = listing("https://theirs.example/documents/format");
    let (text, conflicts) = merged_at(MANIFEST, &base, &ours, &theirs);
    assert_eq!(conflicts, 1, "{text}");
    assert!(
        text.contains("ours.example") && text.contains("theirs.example"),
        "{text}"
    );
    let kept = text.split("conflicts:").next().unwrap();
    assert_eq!(kept.matches("  - handle: ").count(), 1, "{text}");
    assert!(kept.contains("remote.example/documents/format"), "{text}");
    let (text, conflicts) = merged_at(MANIFEST, &base, &ours, &base);
    assert_eq!((text.as_str(), conflicts), (ours.as_str(), 0));
    let (text, conflicts) = merged_at(MANIFEST, &base, &ours, &ours);
    assert_eq!((text.as_str(), conflicts), (ours.as_str(), 0));
}
