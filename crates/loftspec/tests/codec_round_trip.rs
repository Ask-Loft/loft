//! A document written as a `spec/` tree reads back to the same roots (D-11 FG-2).

mod common;

use common::small_document;
use common::v1::{convert_value, full_export};
use loftspec::codec::{self, Files};
use loftspec::document::Document;
use loftspec::hash::hash_document;

/// The document a tree reads as, or a panic listing the first read errors.
fn read_back(files: &Files) -> Document {
    codec::read(files).unwrap_or_else(|errors| {
        let messages: Vec<String> = errors.iter().take(20).map(ToString::to_string).collect();
        panic!("{}", messages.join("\n"))
    })
}

fn round_trip(document: &Document) -> (Files, Document) {
    let files = codec::write(document);
    let back = read_back(&files);
    (files, back)
}

/// Asserts two documents have the same content, binding and state roots, returning the state
/// root.
fn assert_roots_match(before: &Document, after: &Document) -> loftspec::canonical::Digest {
    let before = hash_document(before).unwrap().roots;
    let after = hash_document(after).unwrap().roots;
    assert_eq!(after.content, before.content, "content root");
    assert_eq!(after.bindings, before.bindings, "binding root");
    assert_eq!(after.state, before.state, "state root");
    after.state
}

fn assert_same_roots(document: &Document) -> Files {
    let (files, back) = round_trip(document);
    assert_roots_match(document, &back);
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
/// With `LOFTSPEC_TEXT_OUT` set, the written tree is left under that directory, for `loft fmt`.
fn a_live_v1_export_round_trips_through_text() {
    let document = loftspec::convert::v1::convert(&common::v1::live_export())
        .unwrap()
        .document;
    let started = std::time::Instant::now();
    let files = codec::write(&document);
    let written = started.elapsed();
    let started = std::time::Instant::now();
    let back = read_back(&files);
    let read = started.elapsed();
    let state_root = assert_roots_match(&document, &back);
    assert_eq!(codec::write(&back), files, "a second write changes no byte");
    let invalid = loftspec::validate::validate(&back);
    assert!(invalid.is_empty(), "{invalid:#?}");
    let started = std::time::Instant::now();
    let formatted = loftspec::fmt::format(&files).unwrap();
    let format = started.elapsed();
    assert!(
        formatted.changed.is_empty(),
        "{:?}",
        formatted.changed.keys()
    );
    if let Ok(out) = std::env::var("LOFTSPEC_TEXT_OUT") {
        for (path, bytes) in &files {
            let target = std::path::Path::new(&out).join("spec").join(path.as_str());
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, bytes).unwrap();
        }
    }
    let bytes: usize = files.values().map(Vec::len).sum();
    println!(
        "{} files, {bytes} bytes; write {written:?}, read {read:?}, format {format:?}; state \
         root {}",
        files.len(),
        state_root
    );
}
