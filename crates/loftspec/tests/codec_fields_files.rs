//! The fields of the manifest, the bindings and the registry entries: written in the Format's
//! order, and refused by the reader when a required one is missing. Each expected order is the
//! published list, written out here rather than taken from the record schema the writer
//! follows.

use crate::common::fields::{assert_required, fields, scalar, scalars};
use crate::common::full::full_files;
use crate::common::text::{path, text};
use crate::common::{key, product, small_document};
use loftspec::codec::{self, Problem};
use loftspec::document::{CategoryEntry, TagEntry};
use loftspec::ids::{CategoryId, ProductId, TagId};
use loftspec::names::{CategoryName, TagName};
use loftspec::yaml::{Location, Position};

const MANIFEST: &str = "loftspec.yaml";
const LABELS: &str = "labels.yaml";
const DERIVED_FROM: &[&str] = &["derivedFrom"];
const BINDING: &[&str] = &["bindings", "0"];
const ENTRY: &[&str] = &["0"];

#[test]
fn the_manifests_fields_are_written_in_the_published_order() {
    let published = [
        "format",
        "algorithm",
        "documentKey",
        "publishBranch",
        "connected",
        "products",
        "derivedFrom",
    ];
    assert_eq!(fields(&full_files(), MANIFEST, &[]), published);
}

#[test]
fn a_manifest_without_its_format_algorithm_or_document_key_is_refused() {
    for field in ["format", "algorithm", "documentKey"] {
        assert_required(&full_files(), MANIFEST, &[], field);
    }
}

#[test]
fn derived_from_holds_its_three_fields_in_the_published_order_and_requires_each() {
    let published = ["authoredStateId", "contentAddress", "publishSequence"];
    assert_eq!(fields(&full_files(), MANIFEST, DERIVED_FROM), published);
    for field in published {
        assert_required(&full_files(), MANIFEST, DERIVED_FROM, field);
    }
}

#[test]
fn the_counters_are_required_and_written_for_the_nine_prefixes_in_the_published_order() {
    let published = ["P", "FG", "F", "RQ", "AC", "CMP", "CON", "QA", "ADR"];
    assert_eq!(fields(&full_files(), LABELS, &["counters"]), published);
    assert_required(&full_files(), LABELS, &[], "counters");
}

#[test]
fn a_binding_without_its_key_or_label_is_refused() {
    for field in ["key", "label"] {
        assert_required(&full_files(), LABELS, BINDING, field);
    }
}

#[test]
fn a_registry_entrys_fields_are_written_in_the_published_order_and_all_are_required() {
    let published = ["key", "name", "definition"];
    for registry in ["tags.yaml", "categories.yaml"] {
        assert_eq!(fields(&full_files(), registry, ENTRY), published);
        for field in published {
            assert_required(&full_files(), registry, ENTRY, field);
        }
    }
}

#[test]
fn registry_entries_with_the_same_name_are_written_in_key_order() {
    let mut document = small_document();
    let tag = |n: u32| TagEntry {
        id: TagId::new(key(n)),
        name: TagName::parse("portable").unwrap(),
        definition: "Twice.".to_owned(),
    };
    document.tags = vec![tag(62), tag(61)];
    let category = |n: u32| CategoryEntry {
        id: CategoryId::new(key(n)),
        name: CategoryName::parse("runtime.language").unwrap(),
        definition: "Twice.".to_owned(),
    };
    document.categories = vec![category(65), category(64)];
    let files = codec::write(&document);
    for (registry, first, second) in [("tags.yaml", 61, 62), ("categories.yaml", 64, 65)] {
        let keys = [
            scalar(&files, registry, &["0", "key"]),
            scalar(&files, registry, &["1", "key"]),
        ];
        assert_eq!(keys, [key(first).to_string(), key(second).to_string()]);
    }
}

#[test]
fn the_labels_file_holds_the_counters_then_the_bindings() {
    assert_eq!(fields(&full_files(), LABELS, &[]), ["counters", "bindings"]);
}

/// The position in the bindings list of the binding whose live label is `label`.
fn binding_of(files: &codec::Files, label: &str) -> String {
    let count = codec::read(files).unwrap().bindings.len();
    (0..count)
        .map(|index| index.to_string())
        .find(|index| scalar(files, LABELS, &["bindings", index, "label"]) == label)
        .unwrap()
}

#[test]
fn a_bindings_fields_are_written_in_the_published_order() {
    // The departed decision's binding, the one that fills every field of a binding.
    let files = full_files();
    let at = binding_of(&files, "ADR-5");
    let published = [
        "key",
        "label",
        "departed",
        "aliases",
        "withdrawn",
        "publishedState",
        "publishedHistory",
    ];
    assert_eq!(fields(&files, LABELS, &["bindings", &at]), published);
}

#[test]
fn a_binding_that_is_not_departed_and_records_no_state_omits_those_fields() {
    let files = full_files();
    let at = binding_of(&files, "RQ-2");
    let written = fields(&files, LABELS, &["bindings", &at]);
    assert_eq!(written, ["key", "label", "aliases", "withdrawn"]);
}

#[test]
fn a_departed_binding_is_written_with_departed_true_and_read_back() {
    let files = full_files();
    let at = binding_of(&files, "ADR-5");
    assert_eq!(
        scalar(&files, LABELS, &["bindings", &at, "departed"]),
        "true"
    );
    assert!(text(&files, LABELS).contains("\n    departed: true\n"));
    let read = codec::read(&files).unwrap();
    assert_eq!(codec::write(&read), files);
}

#[test]
fn a_published_state_is_one_of_the_five_kinds_of_a_decisions_state() {
    for kind in ["proposed", "accepted", "rejected", "superseded", "retired"] {
        let mut files = full_files();
        let labels = text(&files, LABELS).replace(
            "publishedState: accepted",
            &format!("publishedState: {kind}"),
        );
        files.insert(path(LABELS), labels.into_bytes());
        assert!(codec::read(&files).is_ok(), "{kind}");
    }
    let mut files = full_files();
    let labels = text(&files, LABELS).replace("publishedState: accepted", "publishedState: open");
    files.insert(path(LABELS), labels.into_bytes());
    let errors = codec::read(&files).unwrap_err();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(matches!(errors[0].problem, Problem::Invalid { .. }));
}

#[test]
fn the_manifest_lists_the_product_keys_in_document_order() {
    let mut document = small_document();
    document
        .products
        .insert(0, product(key(50), "Second Product"));
    let files = codec::write(&document);
    let listed = scalars(&files, MANIFEST, &["products"]);
    assert_eq!(listed, [key(50).to_string(), key(1).to_string()]);
    let read: Vec<_> = codec::read(&files)
        .unwrap()
        .products
        .iter()
        .map(|product| product.id)
        .collect();
    assert_eq!(
        read,
        [Some(ProductId::new(key(50))), Some(ProductId::new(key(1)))]
    );
}

#[test]
fn the_manifest_names_a_publish_branch_only_when_the_document_has_one() {
    assert_eq!(
        scalar(&full_files(), MANIFEST, &["publishBranch"]),
        "release"
    );
    let read = codec::read(&full_files()).unwrap();
    assert_eq!(read.publish_branch.as_deref(), Some("release"));
    let small = codec::write(&small_document());
    let written = fields(&small, MANIFEST, &[]);
    assert!(
        !written.iter().any(|field| field == "publishBranch"),
        "{written:?}"
    );
    assert_eq!(codec::read(&small).unwrap().publish_branch, None);
}

#[test]
fn a_manifest_of_a_document_that_is_not_connected_omits_connected() {
    assert_eq!(scalar(&full_files(), MANIFEST, &["connected"]), "true");
    assert!(codec::read(&full_files()).unwrap().connected);
    let small = codec::write(&small_document());
    let written = fields(&small, MANIFEST, &[]);
    assert!(
        !written.iter().any(|field| field == "connected"),
        "{written:?}"
    );
    assert!(!codec::read(&small).unwrap().connected);
}

#[test]
fn a_manifest_whose_connected_is_not_true_is_refused_naming_the_file_line_and_column() {
    for value in ["false", "\"true\"", "yes", "1"] {
        let mut files = full_files();
        let manifest =
            text(&files, MANIFEST).replace("connected: true", &format!("connected: {value}"));
        files.insert(path(MANIFEST), manifest.into_bytes());
        let errors = codec::read(&files).expect_err(value);
        assert_eq!(errors.len(), 1, "{value}: {errors:?}");
        assert!(
            matches!(errors[0].problem, Problem::Invalid { .. }),
            "{value}"
        );
        assert_eq!(errors[0].sites[0].path, path(MANIFEST), "{value}");
        let at = Location::At(Position::new(5, 12));
        assert_eq!(errors[0].sites[0].location, Some(at), "{value}");
    }
}
