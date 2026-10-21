//! Generated documents: the full document with one of its files edited at random, as the merge
//! properties edit it, then formatted and read; or a rich document edited at random in memory.
//! Each reads, so the properties that hold of a read document hold of each.

use std::collections::BTreeMap;

use loftspec::codec::{self, SpecPath};
use loftspec::document::Document;
use loftspec::ext::ExtValue;
use loftspec::ids::RequirementId;
use loftspec::names::Namespace;
use proptest::prelude::*;

use super::doc_edits::{doc_edit, doc_edited};
use super::edits::{Side, edit, edited, growth};
use super::full::full_nested_document;
use super::merge_cases::{FIXTURES, fixture_files};
use super::{key, label, term};

/// The full nested document with a term list, `ext` on its Feature and a departed binding.
pub fn rich_document() -> Document {
    let mut document = full_nested_document();
    document.terms = vec![term(70, "record", &["row", "entry"])];
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.details.ext =
        BTreeMap::from([(Namespace::parse("tool").unwrap(), ExtValue::Integer(3))]);
    let departed = RequirementId::new(key(99));
    document.bindings.bind(departed, label("RQ-40")).unwrap();
    document.bindings.depart(departed).unwrap();
    document
}

/// The full document with the file of fixture `file` grown and edited, formatted, and read;
/// none where it does not read.
fn file_edited(
    file: usize,
    grown: &[super::edits::Edit],
    edits: &[super::edits::Edit],
) -> Option<Document> {
    let fixture = FIXTURES[file];
    let path = SpecPath::new(fixture.path);
    let mut files = fixture_files().clone();
    let bytes = edited(fixture.top, &files[&path], Side::Base, grown);
    let bytes = edited(fixture.top, &bytes, Side::Ours, edits);
    files.insert(path, bytes);
    if let Ok(formatted) = loftspec::fmt::format(&files) {
        files.extend(formatted.changed);
    }
    codec::read(&files).ok()
}

/// A generated document; none where an edit left one that does not read.
pub fn generated() -> impl Strategy<Value = Option<Document>> {
    let files = (
        0..FIXTURES.len(),
        proptest::collection::vec(growth(), 0..=12),
        proptest::collection::vec(edit(), 0..=6),
    )
        .prop_map(|(file, grown, edits)| file_edited(file, &grown, &edits));
    let documents = proptest::collection::vec(doc_edit(), 0..=8).prop_map(|edits| {
        let document = doc_edited(&rich_document(), &edits);
        codec::read(&codec::write(&document)).ok()
    });
    prop_oneof![files, documents]
}
