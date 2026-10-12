//! What a publish writes through the file store: the files under `spec/` it gives, such as
//! `spec/labels.yaml` and the files whose citations it rewrote, and the lock beside `spec/`, in
//! one save (AC-463); and the lock read back only when it is a valid lock record.

mod common;
mod fixtures;

use std::fs;

use common::Scratch;
use fixtures::{LINT_CONFIG, converted, with_lint};
use loftspec::codec::{Files, LABELS, SpecPath};
use loftspec::publish::{LOCK, Lock};
use loftspec_store::journal::JOURNAL;
use loftspec_store::{DocumentStore, FileStore, StoreError};

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

/// The files a publish gives when only the bindings change.
fn labels_file(bytes: Vec<u8>) -> Files {
    Files::from([(SpecPath::new(LABELS), bytes)])
}

/// A lock record for the tests.
fn lock() -> Lock {
    let zero = "0".repeat(64);
    Lock::read(
        format!(
            r#"{{"format":"loftspec/1","algorithm":"sha256-jcs/1",
            "documentKey":"00000000-0000-4000-8000-000000000001",
            "stateRoot":"{zero}","contentRoot":"{zero}","bindingRoot":"{zero}",
            "publishSequence":1,"publishedAt":"2026-01-01T00:00:00Z"}}"#
        )
        .as_bytes(),
    )
    .unwrap()
}

#[test]
fn a_tree_without_a_lock_has_none() {
    let (_scratch, store) = saved();
    assert_eq!(store.lock().unwrap(), None);
}

#[test]
fn ac_945_a_publish_writes_the_bindings_and_the_lock_in_one_save() {
    let (scratch, store) = saved();
    let mut bindings = labels(&scratch);
    bindings.extend_from_slice(b"# changed\n");
    store
        .publish(labels_file(bindings.clone()), &lock())
        .unwrap();
    assert_eq!(labels(&scratch), bindings);
    assert_eq!(store.lock().unwrap(), Some(lock()));
    assert!(!scratch.0.join(JOURNAL).exists());
}

#[test]
fn ac_460_a_publish_without_new_bindings_changes_no_file_under_spec() {
    let (scratch, store) = saved();
    let before = scratch.snapshot();
    store.publish(Files::new(), &lock()).unwrap();
    let mut after = scratch.snapshot();
    assert_eq!(after.remove(LOCK), Some(lock().to_bytes()));
    assert_eq!(after, before);
    assert_eq!(
        fs::read_to_string(scratch.0.join("spec/lint.yaml")).unwrap(),
        LINT_CONFIG
    );
}

#[test]
fn ac_926_a_lock_that_is_not_a_valid_lock_record_is_refused_naming_its_file() {
    let (scratch, store) = saved();
    fs::write(scratch.0.join(LOCK), "{}\n").unwrap();
    let error = store.lock().unwrap_err();
    assert!(matches!(error, StoreError::Lock(..)), "{error}");
    assert!(
        error
            .to_string()
            .contains("loft.lock.json: not a valid lock record"),
        "{error}"
    );
}

#[test]
fn a_publish_writes_the_bindings_a_rewritten_file_and_the_lock_in_one_save() {
    let (scratch, store) = saved();
    let before = scratch.snapshot();
    let feature = "products/reading-list/shelf/add-a-book.yaml";
    let mut rewritten = fs::read(scratch.0.join("spec").join(feature)).unwrap();
    rewritten.extend_from_slice(b"# rewritten\n");
    let mut files = labels_file(b"bindings\n".to_vec());
    files.insert(SpecPath::new(feature), rewritten.clone());
    store.publish(files, &lock()).unwrap();
    let mut after = scratch.snapshot();
    assert_eq!(after.remove(LOCK), Some(lock().to_bytes()));
    assert_eq!(
        after.remove(&format!("spec/{LABELS}")),
        Some(b"bindings\n".to_vec())
    );
    assert_eq!(after.remove(&format!("spec/{feature}")), Some(rewritten));
    let mut unchanged = before;
    unchanged.remove(&format!("spec/{LABELS}"));
    unchanged.remove(&format!("spec/{feature}"));
    assert_eq!(after, unchanged);
}
