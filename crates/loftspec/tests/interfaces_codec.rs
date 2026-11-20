//! Interfaces in the text form: each in its own file under `spec/interfaces/`, read back as
//! written, and no such directory for a document without one.

use crate::common::fields::{assert_required, fields};
use crate::common::full::full_document;
use crate::common::interfaces::{INTERFACE, interface, with_interface};
use crate::common::small_document;
use crate::common::text::{refused_in, replaced, text};
use loftspec::codec::{self, Files, Problem, ReadError};
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
    assert_eq!(
        fields(&files, FILE, &[]),
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
}

const FILE: &str = "interfaces/document-lookup.yaml";

/// The files of [`with_interface`] with `from` replaced by `to` in the file at `at`, read: one
/// error, at that file.
fn refused(at: &str, from: &str, to: &str) -> ReadError {
    refused_in(
        &replaced(&codec::write(&with_interface()), at, from, to),
        at,
    )
}

#[test]
fn an_interface_under_another_node_is_refused_where_it_stands() {
    let nested = "interfaces:\n  - key: 00000000-0000-7000-8000-0000000001e0\n    slug: nested\n    name: \"Nested\"\n";
    let error = refused(
        "components/document-model.yaml",
        "constraints:\n",
        &format!("{nested}constraints:\n"),
    );
    assert_eq!(
        error.problem,
        Problem::UnknownField("interfaces".to_owned())
    );
}

#[test]
fn a_product_feature_group_feature_or_component_under_an_interface_is_refused_where_it_stands() {
    for field in ["products", "featureGroups", "features", "components"] {
        let child = format!(
            "{field}:\n  - key: 00000000-0000-7000-8000-0000000001e1\n    name: \"Inner\"\n"
        );
        let error = refused(FILE, "requirements:\n", &format!("{child}requirements:\n"));
        assert_eq!(
            error.problem,
            Problem::UnknownField(field.to_owned()),
            "{field}"
        );
    }
}

#[test]
fn an_interface_record_without_its_key_slug_or_name_is_refused() {
    let files = codec::write(&with_interface());
    for field in ["key", "slug", "name"] {
        assert_required(&files, FILE, &[], field);
    }
}

#[test]
fn an_interface_file_holds_everything_beneath_its_interface() {
    let document = full_document();
    let files = codec::write(&document);
    let written = text(&files, FILE);
    for field in [
        "requirements:",
        "criteria:",
        "constraints:",
        "qualityAttributes:",
        "sections:",
    ] {
        assert!(written.contains(field), "{field} in\n{written}");
    }
    let beneath = files
        .iter()
        .filter(|(path, _)| path.as_str() != FILE)
        .any(|(_, bytes)| {
            String::from_utf8_lossy(bytes).contains("00000000-0000-7000-8000-0000000001d4")
        });
    assert!(
        !beneath,
        "the criterion beneath the Interface is in its file alone"
    );
    assert_eq!(read_back(&files).interfaces, document.interfaces);
}
