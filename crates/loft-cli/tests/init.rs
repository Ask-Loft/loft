//! `loft init` (the `loft init` Feature): an empty document created in the current directory,
//! which reads, validates and lints clean; refused, with nothing written, where `spec/` exists.

use std::fs;

use crate::common::{Scratch, stderr, stdout};
use crate::schema;
use crate::snapshot::snapshot;
use loftspec::bindings::Counters;
use loftspec::document::Document;
use loftspec_store::{DocumentStore as _, FileStore};
use serde_json::Value;

/// A directory where `loft init` ran and succeeded.
fn initialised() -> Scratch {
    let scratch = Scratch::empty();
    let output = scratch.loft(&["init"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    scratch
}

/// The document the tree holds, read back by the reader.
fn document(scratch: &Scratch) -> Document {
    FileStore::open(&scratch.0).unwrap().load().unwrap()
}

/// The document key the manifest carries.
fn document_key(scratch: &Scratch) -> String {
    document(scratch).id.to_string()
}

#[test]
fn spec_is_created_with_a_manifest() {
    let scratch = initialised();
    assert!(scratch.file("loftspec.yaml").is_file());
}

#[test]
fn the_manifest_carries_a_fresh_document_key() {
    let (first, second) = (document_key(&initialised()), document_key(&initialised()));
    assert_ne!(first, second);
    // A minted key is a version 7 UUID.
    assert_eq!(first.as_bytes()[14], b'7', "{first}");
}

#[test]
fn the_labels_file_is_created_with_every_counter_at_its_first_value() {
    let document = document(&initialised());
    assert_eq!(document.counters, Counters::default());
    assert!(document.bindings.iter().next().is_none());
}

#[test]
fn the_registries_are_created_empty() {
    let scratch = initialised();
    for registry in ["tags.yaml", "categories.yaml"] {
        assert_eq!(fs::read(scratch.file(registry)).unwrap(), b"", "{registry}");
    }
}

#[test]
fn the_created_tree_reads_validates_and_lints_clean() {
    let scratch = initialised();
    assert_eq!(
        loftspec::validate::validate(&document(&scratch)),
        Vec::new()
    );
    let check = scratch.loft(&["check", "--fail-on", "hint"]);
    assert_eq!(check.status.code(), Some(0), "{}", stdout(&check));
    assert_eq!(stdout(&check), "");
}

#[test]
fn an_existing_spec_is_refused_naming_it_and_nothing_is_written() {
    let scratch = Scratch::empty();
    fs::create_dir(scratch.0.join("spec")).unwrap();
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&["init"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("spec"), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("already exists"),
        "{}",
        stderr(&output)
    );
    assert_eq!(snapshot(&scratch.0), before);
}

#[test]
fn a_second_init_is_refused_and_keeps_the_first_tree_with_its_edits() {
    let scratch = initialised();
    let key = format!("documentKey: {}\n", document_key(&scratch));
    scratch.edit(
        "loftspec.yaml",
        &key,
        &format!("{key}publishBranch: trunk\n"),
    );
    let before = snapshot(&scratch.0);
    assert_eq!(scratch.loft(&["init"]).status.code(), Some(1));
    assert_eq!(snapshot(&scratch.0), before);
}

#[test]
fn a_tree_already_there_is_refused_and_left_as_it_was() {
    let scratch = Scratch::with_reading_list();
    let before = snapshot(&scratch.0);
    assert_eq!(scratch.loft(&["init"]).status.code(), Some(1));
    assert_eq!(snapshot(&scratch.0), before);
}

/// The one JSON document a run printed, held to the `loft init` schema.
fn json_of(output: &std::process::Output) -> Value {
    schema::conforming(&output.stdout, include_str!("../../../schemas/init.json"))
}

#[test]
fn the_json_document_names_spec_as_created_and_carries_the_document_key() {
    let scratch = Scratch::empty();
    let output = scratch.loft(&["init", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document = json_of(&output);
    assert_eq!(document["outcome"], "created");
    assert_eq!(document["created"], "spec/");
    assert_eq!(document["documentKey"], document_key(&scratch));
}

#[test]
fn a_refused_init_is_listed_among_the_json_document_s_errors_naming_the_path() {
    let scratch = Scratch::empty();
    fs::create_dir(scratch.0.join("spec")).unwrap();
    let output = scratch.loft(&["init", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    let document = json_of(&output);
    assert_eq!(document["outcome"], "error");
    let message = document["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("spec") && message.contains("already exists"),
        "{message}"
    );
}
