//! A document written as a `spec/` tree reads back to the same roots (FG-2).

use crate::common;
use crate::common::fields::scalar;
use crate::common::small_document;
use crate::common::text::{DECISION, FEATURE, PRODUCT, text};
use crate::common::v1::{convert_value, full_export};
use loftspec::codec::{self, Files};
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::hash::hash_document;
use loftspec::ids::{ComponentId, DecisionId};

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

#[test]
fn a_products_constraints_are_written_in_its_record_and_read_back() {
    let mut document = small_document();
    let mut constraint = document.components[0].constraints[0].clone();
    constraint.id = Some(loftspec::ids::ConstraintId::new(common::key(90)));
    document.products[0].constraints = vec![constraint.clone()];
    let files = assert_same_roots(&document);
    let back = read_back(&files);
    assert_eq!(back.products[0].constraints, vec![constraint]);
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
fn writing_what_was_read_changes_no_byte() {
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

#[test]
fn a_decisions_subjects_are_written_by_role_then_by_target() {
    let mut document = small_document();
    let decision = DecisionId::new(common::key(20));
    let subject = |n: u32, role: SubjectRole| Edge::Subject {
        decision,
        subject: ComponentId::new(common::key(n)).into(),
        role,
    };
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Subject { .. }));
    for n in [50, 51, 52, 53] {
        document.components.push(common::root(common::component(
            common::key(n),
            &format!("Part {n}"),
        )));
    }
    document.edges.extend([
        subject(53, SubjectRole::Outcome),
        subject(51, SubjectRole::Affected),
        subject(50, SubjectRole::Outcome),
        subject(52, SubjectRole::Affected),
    ]);
    let files = codec::write(&document);
    let written: Vec<(String, String)> = (0..4)
        .map(|index| {
            let at = |field| scalar(&files, DECISION, &["subjects", &index.to_string(), field]);
            (at("role"), at("node"))
        })
        .collect();
    let expected = [
        ("affected", 51),
        ("affected", 52),
        ("outcome", 50),
        ("outcome", 53),
    ]
    .map(|(role, n)| (role.to_owned(), common::key(n).to_string()));
    assert_eq!(written, expected);
}

#[test]
fn an_absent_body_is_not_written() {
    let mut document = small_document();
    document.products[0].details.body = None;
    let files = codec::write(&document);
    let product = text(&files, PRODUCT);
    assert!(!product.contains("body"), "{product}");
    let feature = text(&files, FEATURE);
    assert_eq!(
        feature.matches("body:").count(),
        1,
        "the Feature's alone: {feature}"
    );
    assert_eq!(read_back(&files).products[0].details.body, None);
}

#[test]
fn a_section_is_written_under_sections_in_its_owning_nodes_record() {
    let files = common::full::full_files();
    let at = |file, route: &[&str]| scalar(&files, file, route);
    assert_eq!(
        at(PRODUCT, &["sections", "0", "key"]),
        common::key(40).to_string()
    );
    assert_eq!(at(PRODUCT, &["sections", "0", "name"]), "Notes");
    assert_eq!(
        at(PRODUCT, &["sections", "0", "sections", "0", "key"]),
        common::key(41).to_string()
    );
    assert_eq!(
        at(FEATURE, &["requirements", "0", "sections", "0", "key"]),
        common::key(51).to_string()
    );
    assert!(
        files.keys().all(|path| !path.as_str().contains("section")),
        "no Section has a file of its own"
    );
}
