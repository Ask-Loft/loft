//! What a publish writes through the file store: `spec/labels.yaml` and the lock beside `spec/`,
//! in one save (AC-463), the bindings only when their bytes change.

mod common;
mod fixtures;

use std::fs;

use common::Scratch;
use fixtures::{LINT_CONFIG, converted, with_lint};
use loftspec::codec::{self, LABELS, SpecPath};
use loftspec_store::fs::LOCK;
use loftspec_store::journal::JOURNAL;
use loftspec_store::{DocumentStore, FileStore};

const READING_LIST: &str =
    include_str!("../../../testdata/v1/reading-list/loft-authored-state.json");

/// A file store holding the Reading List, with a lint configuration beside the document.
fn saved() -> (Scratch, FileStore) {
    let scratch = with_lint();
    let mut store = FileStore::open(&scratch.0).unwrap();
    store.save(&converted(READING_LIST)).unwrap();
    (scratch, store)
}

fn labels(scratch: &Scratch) -> Vec<u8> {
    fs::read(scratch.0.join("spec").join(LABELS)).unwrap()
}

#[test]
fn a_tree_without_a_lock_has_none() {
    let (_scratch, store) = saved();
    assert_eq!(store.lock().unwrap(), None);
}

#[test]
fn a_publish_writes_the_bindings_and_the_lock_in_one_save() {
    let (scratch, store) = saved();
    let mut bindings = labels(&scratch);
    bindings.extend_from_slice(b"# changed\n");
    store.publish(bindings.clone(), b"{}\n".to_vec()).unwrap();
    assert_eq!(labels(&scratch), bindings);
    assert_eq!(store.lock().unwrap(), Some(b"{}\n".to_vec()));
    assert!(!scratch.0.join(JOURNAL).exists());
}

#[test]
fn a_publish_changes_no_other_file_under_spec() {
    let (scratch, store) = saved();
    let before = scratch.snapshot();
    store.publish(labels(&scratch), b"{}\n".to_vec()).unwrap();
    let mut after = scratch.snapshot();
    assert_eq!(after.remove(LOCK), Some(b"{}\n".to_vec()));
    assert_eq!(after, before);
    assert_eq!(
        fs::read_to_string(scratch.0.join("spec/lint.yaml")).unwrap(),
        LINT_CONFIG
    );
}

#[cfg(unix)]
#[test]
fn bindings_whose_bytes_would_not_change_are_not_rewritten() {
    use std::os::unix::fs::MetadataExt;

    let (scratch, store) = saved();
    let path = scratch.0.join("spec").join(LABELS);
    let before = fs::metadata(&path).unwrap();
    let document = store.load().unwrap();
    let unchanged = codec::write(&document)
        .remove(&SpecPath::new(LABELS))
        .unwrap();
    store.publish(unchanged, b"{}\n".to_vec()).unwrap();
    let after = fs::metadata(&path).unwrap();
    assert_eq!(
        (after.ino(), after.mtime_nsec(), after.mtime()),
        (before.ino(), before.mtime_nsec(), before.mtime())
    );
}
