//! A Component's boundary in the text form and the content view (D-11 Component boundary, ADR-15).

mod common;

use common::small_document;
use common::text::{edited, files, text};
use loftspec::attributes::Boundary;
use loftspec::canonical::canonical_bytes;
use loftspec::codec::{self, Problem};
use loftspec::hash::ContentHash;

const COMPONENT: &str = "components/document-model.yaml";

fn external_document() -> loftspec::document::Document {
    let mut document = small_document();
    document.components[0].boundary = Some(Boundary::External);
    document
}

#[test]
fn boundary_is_written_after_the_body_and_tags_and_before_scoped_to() {
    let written = codec::write(&external_document());
    let component = String::from_utf8(written[&common::text::path(COMPONENT)].clone()).unwrap();
    let boundary = component.find("\nboundary: external\n").expect("written");
    assert!(boundary > component.find("\nbody: ").unwrap());
    assert!(boundary < component.find("\nscopedTo:").unwrap());
}

#[test]
fn boundary_reads_back_as_written() {
    let document = external_document();
    let read = codec::read(&codec::write(&document)).unwrap();
    assert_eq!(read.components[0].boundary, Some(Boundary::External));
    assert_eq!(codec::write(&read), codec::write(&document));
}

#[test]
fn a_component_without_boundary_reads_as_none() {
    assert_eq!(codec::read(&files()).unwrap().components[0].boundary, None);
    assert!(!text(&files(), COMPONENT).contains("boundary"));
}

#[test]
fn another_boundary_value_is_an_error_at_its_position() {
    let files = edited(COMPONENT, "scopedTo:", "boundary: outside\nscopedTo:");
    let errors = codec::read(&files).expect_err("refused");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        matches!(&errors[0].problem, Problem::Invalid { value, .. } if value == "outside"),
        "{errors:?}"
    );
    assert_eq!(
        errors[0].sites[0].to_string(),
        "spec/components/document-model.yaml:7:11"
    );
}

#[test]
fn the_boundary_enters_the_content_view_as_written() {
    let mut component = small_document().components[0].component.clone();
    let view = |component: &loftspec::tree::Component| {
        String::from_utf8(canonical_bytes(&component.content_view()[9])).unwrap()
    };
    assert_eq!(view(&component), "null");
    component.boundary = Some(Boundary::External);
    assert_eq!(view(&component), "\"external\"");
    component.boundary = Some(Boundary::Private);
    assert_eq!(view(&component), "\"private\"");
}

#[test]
fn an_explicit_private_and_an_absent_boundary_give_different_views() {
    let mut component = small_document().components[0].component.clone();
    let absent = component.content_hash();
    component.boundary = Some(Boundary::Private);
    assert_ne!(component.content_hash(), absent);
}

#[test]
fn a_node_that_is_not_a_component_has_a_null_boundary_slot() {
    let feature = &small_document().products[0].feature_groups[0].features[0];
    assert_eq!(feature.content_view()[9], serde_json::Value::Null);
}
