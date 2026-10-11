//! A document written as a `spec/` tree reads back to the same roots (D-11 FG-2).

mod common;

use common::small_document;
use common::v1::{convert_value, full_export};
use loftspec::codec::{self, Files};
use loftspec::document::Document;
use loftspec::hash::hash_document;

fn round_trip(document: &Document) -> (Files, Document) {
    let files = codec::write(document);
    let back = codec::read(&files).unwrap_or_else(|errors| {
        let messages: Vec<String> = errors.iter().map(ToString::to_string).collect();
        panic!("{}", messages.join("\n"))
    });
    (files, back)
}

fn assert_same_roots(document: &Document) -> Files {
    let (files, back) = round_trip(document);
    let before = hash_document(document).unwrap().roots;
    let after = hash_document(&back).unwrap().roots;
    assert_eq!(after.content, before.content, "content root");
    assert_eq!(after.bindings, before.bindings, "binding root");
    assert_eq!(after.state, before.state, "state root");
    files
}

#[test]
fn the_small_document_reads_back_to_the_same_roots() {
    let files = assert_same_roots(&small_document());
    for path in [
        "loftspec.yaml",
        "labels.yaml",
        "tags.yaml",
        "categories.yaml",
    ] {
        assert!(files.keys().any(|key| key.as_str() == path), "{path}");
    }
}

#[test]
fn a_converted_export_reads_back_to_the_same_roots() {
    assert_same_roots(&convert_value(&full_export()).unwrap().document);
}

#[test]
fn the_reading_list_reads_back_to_the_same_roots() {
    let text = include_str!("../../../testdata/v1/reading-list/loft-authored-state.json");
    let export: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_same_roots(&convert_value(&export).unwrap().document);
}

#[test]
fn ac_119_writing_what_was_read_changes_no_byte() {
    let document = convert_value(&full_export()).unwrap().document;
    let (files, back) = round_trip(&document);
    assert_eq!(codec::write(&back), files);
}

#[test]
#[ignore = "reads a local v1 export named by LOFTSPEC_V1_EXPORT; the export is not in this repository"]
fn a_live_v1_export_round_trips_through_text() {
    let document = loftspec::convert::v1::convert(&common::v1::live_export())
        .unwrap()
        .document;
    let started = std::time::Instant::now();
    let files = codec::write(&document);
    let written = started.elapsed();
    let started = std::time::Instant::now();
    let back = codec::read(&files).unwrap_or_else(|errors| {
        let messages: Vec<String> = errors.iter().take(20).map(ToString::to_string).collect();
        panic!("{}", messages.join("\n"))
    });
    let read = started.elapsed();
    let before = hash_document(&document).unwrap().roots;
    let after = hash_document(&back).unwrap().roots;
    assert_eq!(after.content, before.content, "content root");
    assert_eq!(after.bindings, before.bindings, "binding root");
    assert_eq!(after.state, before.state, "state root");
    assert_eq!(codec::write(&back), files, "a second write changes no byte");
    let bytes: usize = files.values().map(Vec::len).sum();
    println!(
        "{} files, {bytes} bytes; write {written:?}, read {read:?}; state root {}",
        files.len(),
        after.state
    );
}
