//! The file store saves only what changes: a file whose bytes would not change is left alone, a
//! save of an unchanged document writes nothing, and `spec/lint.yaml` is never removed.

mod common;

use std::fs;

use common::Scratch;
use loftspec::codec::SpecPath;
use loftspec::convert::v1::convert;
use loftspec::convert::v1::export::Export;
use loftspec::document::Document;
use loftspec_store::journal::JOURNAL;
use loftspec_store::{DocumentStore, FileStore, MemoryStore, StoreError};

const READING_LIST: &str =
    include_str!("../../../testdata/v1/reading-list/loft-authored-state.json");

const LINT: &str = "rules:\n  - rule: SH-002\n    enabled: false\n";

fn reading_list() -> Document {
    convert(&Export::from_json(READING_LIST).unwrap())
        .unwrap()
        .document
}

/// A file store holding the Reading List, with a lint configuration beside the document.
fn saved() -> (Scratch, FileStore) {
    let scratch = Scratch::new();
    scratch.put("spec/lint.yaml", LINT);
    let mut store = FileStore::open(&scratch.0).unwrap();
    store.save(&reading_list()).unwrap();
    (scratch, store)
}

#[test]
fn a_store_that_holds_no_document_says_so() {
    let scratch = Scratch::new();
    assert!(matches!(
        MemoryStore::new().load(),
        Err(StoreError::NoDocument)
    ));
    assert!(matches!(
        FileStore::open(&scratch.0).unwrap().load(),
        Err(StoreError::NoTree(_))
    ));
    assert_eq!(scratch.snapshot().len(), 0);
}

#[test]
fn a_save_of_an_unchanged_document_writes_nothing() {
    let (scratch, mut store) = saved();
    let before = scratch.snapshot();
    let resaved = store.save(&store.load().unwrap()).unwrap();
    assert!(resaved.is_empty(), "{resaved:?}");
    assert_eq!(scratch.snapshot(), before);
    assert!(!scratch.0.join(JOURNAL).exists());
}

#[test]
fn a_document_save_leaves_lint_yaml_unchanged() {
    let (scratch, mut store) = saved();
    let mut document = store.load().unwrap();
    document.products.clear();
    let resaved = store.save(&document).unwrap();
    assert!(!resaved.removed.is_empty());
    assert!(!resaved.removed.contains(&SpecPath::new("lint.yaml")));
    assert_eq!(
        fs::read(scratch.0.join("spec/lint.yaml")).unwrap(),
        LINT.as_bytes()
    );
}

#[cfg(unix)]
#[test]
fn a_file_whose_bytes_would_not_change_is_not_rewritten() {
    use std::os::unix::fs::MetadataExt;

    let (scratch, mut store) = saved();
    let tags = scratch.0.join("spec/tags.yaml");
    let before = fs::metadata(&tags).unwrap();
    let mut document = store.load().unwrap();
    document.products[0].name.push_str(" again");
    let resaved = store.save(&document).unwrap();
    assert_eq!(resaved.written.len(), 1, "{resaved:?}");
    let after = fs::metadata(&tags).unwrap();
    assert_eq!(
        (after.ino(), after.mtime_nsec(), after.mtime()),
        (before.ino(), before.mtime_nsec(), before.mtime())
    );
}
