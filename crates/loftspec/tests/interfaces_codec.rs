//! Interfaces in the text form: each in its own file under `spec/interfaces/`, read back as
//! written, and no such directory for a document without one.

use crate::common::interfaces::{INTERFACE, interface, with_interface};
use crate::common::small_document;
use crate::common::text::{path, text};
use loftspec::codec::{self, Files};
use loftspec::kind::NodeKind;
use loftspec::node::Node;

fn read_back(files: &Files) -> loftspec::document::Document {
    codec::read(files).unwrap_or_else(|errors| panic!("{errors:?}"))
}

#[test]
fn an_interface_is_written_to_its_own_file_under_its_slug() {
    let files = codec::write(&with_interface());
    let written = text(&files, "interfaces/document-lookup.yaml");
    assert!(written.starts_with("key: "), "{written}");
    assert!(written.contains("\nslug: document-lookup\n"), "{written}");
    assert!(
        written.contains("\nname: \"Document lookup\"\n"),
        "{written}"
    );
    assert!(written.contains("\nrequirements:\n"), "{written}");
}

#[test]
fn an_interface_record_holds_its_fields_in_the_formats_order() {
    let mut document = with_interface();
    let promise = &mut document.interfaces[0];
    promise.constraints = document.components[0].constraints.clone();
    promise.quality_attributes = document.components[0].quality_attributes.clone();
    let files = codec::write(&document);
    let written = text(&files, "interfaces/document-lookup.yaml");
    let fields: Vec<&str> = written
        .lines()
        .filter(|line| !line.starts_with(' ') && !line.starts_with('-'))
        .filter_map(|line| line.split_once(':').map(|(name, _)| name))
        .collect();
    assert_eq!(
        fields,
        [
            "key",
            "slug",
            "name",
            "body",
            "requirements",
            "constraints",
            "qualityAttributes"
        ]
    );
}

#[test]
fn an_interface_reads_back_as_it_was_written() {
    let document = with_interface();
    let files = codec::write(&document);
    let back = read_back(&files);
    assert_eq!(back.interfaces, document.interfaces);
    assert_eq!(codec::write(&back), files);
}

#[test]
fn interfaces_are_ordered_by_file_name_after_the_root_components() {
    let mut document = with_interface();
    let earlier = interface(
        crate::common::key(INTERFACE + 10),
        "Analytics feed",
        Vec::new(),
    );
    document.interfaces.push(earlier);
    let back = read_back(&codec::write(&document));
    let names: Vec<_> = back
        .interfaces
        .iter()
        .map(|each| each.name.as_str())
        .collect();
    assert_eq!(names, ["Analytics feed", "Document lookup"]);
    let kinds: Vec<NodeKind> = back.root_nodes().map(Node::kind).collect();
    let last_component = kinds.iter().rposition(|kind| *kind == NodeKind::Component);
    let first_interface = kinds.iter().position(|kind| *kind == NodeKind::Interface);
    assert!(last_component < first_interface, "{kinds:?}");
}

#[test]
fn a_document_without_an_interface_is_written_without_an_interfaces_directory() {
    let files = codec::write(&small_document());
    assert!(
        files
            .keys()
            .all(|at| !at.to_string().starts_with("interfaces/")),
        "{:?}",
        files.keys().collect::<Vec<_>>()
    );
    assert!(!files.contains_key(&path("interfaces/document-lookup.yaml")));
}
