//! The file store saves only what changes: a file whose bytes would not change is left alone, a
//! save of an unchanged document writes nothing, and `spec/lint.yaml` is never removed.

mod common;
mod fixtures;

use std::fs;

use common::Scratch;
use fixtures::{LINT_CONFIG, converted, with_lint};
use loftspec::codec::{self, SpecPath};
use loftspec::document::Document;
use loftspec_store::journal::{self, Changes, JOURNAL, RootPath};
use loftspec_store::{DocumentStore, FileStore, MemoryStore, StoreError};

const READING_LIST: &str =
    include_str!("../../../testdata/v1/reading-list/loft-authored-state.json");

fn reading_list() -> Document {
    converted(READING_LIST)
}

/// A file store holding the Reading List, with a lint configuration beside the document.
fn saved() -> (Scratch, FileStore) {
    let scratch = with_lint();
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
    assert_ne!(resaved.removed, Vec::new());
    assert!(!resaved.removed.contains(&SpecPath::new("lint.yaml")));
    assert_eq!(
        fs::read(scratch.0.join("spec/lint.yaml")).unwrap(),
        LINT_CONFIG.as_bytes()
    );
}

#[test]
fn a_save_finishes_an_interrupted_save_before_it_reads_the_tree() {
    let (scratch, mut store) = saved();
    let tags = SpecPath::new("tags.yaml");
    let mut planted = Changes::default();
    planted
        .write
        .insert(RootPath::in_spec(&tags), b"planted\n".to_vec());
    drop(
        journal::prepare(&scratch.0, &planted)
            .unwrap()
            .commit()
            .unwrap(),
    );
    let resaved = store.save(&reading_list()).unwrap();
    assert_eq!(resaved.written, vec![tags.clone()]);
    assert_eq!(
        fs::read(scratch.0.join("spec/tags.yaml")).unwrap(),
        codec::write(&reading_list())[&tags]
    );
    assert!(!scratch.0.join(JOURNAL).exists());
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
