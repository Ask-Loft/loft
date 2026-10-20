//! A merge conflict in the model (F-34): the validator reports it at the file and key of
//! the record that holds it until it is removed (RQ-126), it survives a write and a read, no
//! hash reads it, and a tree that holds one does not verify.

use crate::common::text::{FEATURE, SHALL_CONFLICT, edited, files, on_requirement, path, text};
use loftspec::check::{Checked, check};
use loftspec::codec::{self, Files};
use loftspec::conflict::{ConflictOwner, ConflictValue, FieldPath};
use loftspec::hash::hash_document;
use loftspec::publish::{
    Made, Publication, PublishedAt, ReadTree, RepoState, Unverified, publish, verify,
};

fn conflicted() -> Files {
    on_requirement(SHALL_CONFLICT)
}

/// The validation errors a tree gives, as they print.
fn errors(files: &Files) -> Vec<String> {
    match check(files) {
        Checked::Read { invalid, .. } => invalid.iter().map(ToString::to_string).collect(),
        Checked::Unread(errors) => panic!("the tree reads: {errors:?}"),
    }
}

#[test]
fn a_conflict_record_is_an_error_naming_the_file_and_the_key() {
    let errors = errors(&conflicted());
    assert_eq!(
        errors,
        [format!(
            "spec/{FEATURE}:8:10: requirement 00000000-0000-7000-8000-000000000004 holds an \
             unresolved merge conflict over `statement.shall`"
        )]
    );
}

#[test]
fn removing_the_record_clears_the_error() {
    assert_eq!(errors(&files()), Vec::<String>::new());
    let resolved = codec::read(&conflicted()).map(|mut document| {
        document.conflicts.clear();
        codec::write(&document)
    });
    assert_eq!(errors(&resolved.unwrap()), Vec::<String>::new());
}

#[test]
fn a_conflict_is_read_into_the_model_with_its_owner_its_field_and_its_sides() {
    let document = codec::read(&conflicted()).unwrap();
    let [conflict] = document.conflicts.as_slice() else {
        panic!("one conflict: {:?}", document.conflicts);
    };
    assert_eq!(
        conflict.owner.key().unwrap().to_string(),
        "00000000-0000-7000-8000-000000000004"
    );
    assert!(matches!(conflict.owner, ConflictOwner::Element(_)));
    assert_eq!(conflict.field, FieldPath::new("statement.shall"));
    assert_eq!(
        conflict.ours,
        Some(ConflictValue::Scalar(
            "write a key on each node.".to_owned()
        ))
    );
}

#[test]
fn a_conflict_survives_a_read_and_a_write() {
    let files = conflicted();
    assert_eq!(codec::write(&codec::read(&files).unwrap()), files);
}

#[test]
fn no_hash_reads_a_conflict() {
    let clean = hash_document(&codec::read(&files()).unwrap()).unwrap();
    let held = hash_document(&codec::read(&conflicted()).unwrap()).unwrap();
    assert_eq!(held.roots, clean.roots);
}

#[test]
fn a_conflict_on_the_manifest_is_placed_at_its_file() {
    let mut files = edited(
        "loftspec.yaml",
        "products:\n",
        "publishBranch: main\nproducts:\n",
    );
    let manifest = text(&files, "loftspec.yaml");
    files.insert(
        path("loftspec.yaml"),
        format!("{manifest}conflicts:\n  - field: publishBranch\n    ours: release\n").into_bytes(),
    );
    assert_eq!(
        errors(&files),
        [
            "spec/loftspec.yaml: the manifest holds an unresolved merge conflict over `publishBranch`"
        ]
    );
    assert_eq!(codec::write(&codec::read(&files).unwrap()), files);
}

#[test]
fn a_tree_that_holds_a_conflict_compares_no_root_and_does_not_verify() {
    let repo = RepoState {
        branch: Some("main".to_owned()),
        modified: Vec::new(),
        stopped: None,
    };
    let at = PublishedAt::from_unix_seconds(0).unwrap();
    let published = publish(
        ReadTree::read(&files()).unwrap(),
        Some(&repo),
        None,
        at,
        Made::NotAsRemote,
    )
    .unwrap();
    let (Publication::Published { lock, .. } | Publication::Unchanged(lock)) = published else {
        panic!("a document that is not connected is published");
    };
    let files = conflicted();
    let refused = verify(&ReadTree::read(&files).unwrap(), &lock);
    let Err(Unverified::Invalid(errors)) = refused else {
        panic!("a conflicted tree is refused: {refused:?}");
    };
    assert!(
        errors[0]
            .to_string()
            .contains("holds an unresolved merge conflict"),
        "{}",
        errors[0]
    );
}
