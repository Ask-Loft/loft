//! Writing the tree: a save leaves either every file as it was or every file as written, and the
//! next command finishes a committed save or discards an uncommitted one.

mod common;

use std::collections::BTreeMap;
use std::fs;

use common::Scratch;
use loftspec::codec::SpecPath;
use loftspec_store::StoreError;
use loftspec_store::journal::{self, Changes, JOURNAL, RootPath};

/// A tree of three files, one in a directory of its own.
fn tree() -> Scratch {
    let scratch = Scratch::new();
    scratch.put("spec/loftspec.yaml", "manifest\n");
    scratch.put("spec/tags.yaml", "tags\n");
    scratch.put("spec/products/a/product.yaml", "product a\n");
    scratch
}

fn spec(path: &str) -> RootPath {
    RootPath::in_spec(&SpecPath::new(path))
}

/// Rewrites the tags, adds a Product, removes the other Product and writes a file beside `spec/`.
fn changes() -> Changes {
    let mut changes = Changes::default();
    changes
        .write
        .insert(spec("tags.yaml"), b"tags 2\n".to_vec());
    changes
        .write
        .insert(spec("products/b/product.yaml"), b"product b\n".to_vec());
    changes
        .write
        .insert(RootPath::beside("v1-sidecar.json"), b"{}\n".to_vec());
    changes.remove.insert(spec("products/a/product.yaml"));
    changes
}

/// The tree as [`changes`] leaves it.
fn written() -> BTreeMap<String, Vec<u8>> {
    [
        ("spec/", ""),
        ("spec/loftspec.yaml", "manifest\n"),
        ("spec/tags.yaml", "tags 2\n"),
        ("spec/products/", ""),
        ("spec/products/b/", ""),
        ("spec/products/b/product.yaml", "product b\n"),
        ("v1-sidecar.json", "{}\n"),
    ]
    .into_iter()
    .map(|(path, text)| (path.to_owned(), text.as_bytes().to_vec()))
    .collect()
}

/// Leaves [`changes`] committed and not finished, as a save interrupted after its commit does.
fn committed(scratch: &Scratch) {
    drop(
        journal::prepare(&scratch.0, &changes())
            .unwrap()
            .commit()
            .unwrap(),
    );
}

#[test]
fn a_save_writes_and_removes_every_named_file_and_leaves_no_journal() {
    let scratch = tree();
    journal::save(&scratch.0, &changes()).unwrap();
    assert_eq!(scratch.snapshot(), written());
}

#[test]
fn an_empty_save_creates_no_journal() {
    let scratch = tree();
    let before = scratch.snapshot();
    journal::save(&scratch.0, &Changes::default()).unwrap();
    assert_eq!(scratch.snapshot(), before);
}

#[test]
fn every_file_of_a_save_is_staged_before_any_file_under_spec_changes() {
    let scratch = tree();
    let before = scratch.snapshot();
    let _prepared = journal::prepare(&scratch.0, &changes()).unwrap();
    let after: BTreeMap<_, _> = scratch
        .snapshot()
        .into_iter()
        .filter(|(path, _)| !path.starts_with(".loft/"))
        .collect();
    assert_eq!(after, before);
    let staged = fs::read_dir(scratch.0.join(JOURNAL).join("files")).unwrap();
    assert_eq!(staged.count(), 3);
}

#[test]
fn a_save_interrupted_before_its_journal_is_complete_leaves_every_file_as_it_was() {
    let scratch = tree();
    let before = scratch.snapshot();
    drop(journal::prepare(&scratch.0, &changes()).unwrap());
    journal::recover(&scratch.0).unwrap();
    assert_eq!(scratch.snapshot(), before);
}

#[test]
fn a_save_interrupted_after_its_journal_is_complete_is_finished_by_recovery() {
    let scratch = tree();
    committed(&scratch);
    assert_eq!(
        fs::read(scratch.0.join("spec/tags.yaml")).unwrap(),
        b"tags\n"
    );
    journal::recover(&scratch.0).unwrap();
    assert_eq!(scratch.snapshot(), written());
}

#[test]
fn a_save_interrupted_part_way_through_finishing_is_finished_by_recovery() {
    let scratch = tree();
    committed(&scratch);
    // The first staged file, the new Product, reached its place before the interruption.
    fs::create_dir_all(scratch.0.join("spec/products/b")).unwrap();
    fs::rename(
        scratch.0.join(JOURNAL).join("files/0"),
        scratch.0.join("spec/products/b/product.yaml"),
    )
    .unwrap();
    journal::recover(&scratch.0).unwrap();
    assert_eq!(scratch.snapshot(), written());
}

#[test]
fn recovering_twice_changes_nothing_more() {
    let scratch = tree();
    committed(&scratch);
    journal::recover(&scratch.0).unwrap();
    journal::recover(&scratch.0).unwrap();
    assert_eq!(scratch.snapshot(), written());
}

#[test]
fn a_save_is_refused_while_a_committed_journal_is_there() {
    let scratch = tree();
    committed(&scratch);
    let before = scratch.snapshot();
    let mut more = Changes::default();
    more.write.insert(spec("tags.yaml"), b"tags 3\n".to_vec());
    let error = journal::save(&scratch.0, &more).unwrap_err();
    assert!(matches!(error, StoreError::Pending(_)), "{error}");
    assert_eq!(scratch.snapshot(), before);
}

#[test]
fn a_journal_of_an_unknown_format_is_refused_and_left_alone() {
    let scratch = tree();
    scratch.put(
        ".loft/save/plan.json",
        r#"{"format":"loftspec-save/99","write":[],"remove":[]}"#,
    );
    let before = scratch.snapshot();
    let error = journal::recover(&scratch.0).unwrap_err();
    assert!(matches!(error, StoreError::Journal(..)), "{error}");
    assert_eq!(scratch.snapshot(), before);
}

#[test]
fn a_second_save_is_refused_while_a_journal_is_there() {
    let scratch = tree();
    let _prepared = journal::prepare(&scratch.0, &changes()).unwrap();
    let error = journal::prepare(&scratch.0, &changes()).unwrap_err();
    assert!(matches!(error, StoreError::Pending(_)), "{error}");
}

#[test]
fn a_journal_directory_shares_loft_with_other_files() {
    let scratch = tree();
    scratch.put(".loft/other", "kept\n");
    journal::save(&scratch.0, &changes()).unwrap();
    assert_eq!(fs::read(scratch.0.join(".loft/other")).unwrap(), b"kept\n");
    assert!(!scratch.0.join(JOURNAL).exists());
}
