//! Which file holds each record, the order in which a file lists the records and keys it holds,
//! and the encoding of every file (the Directory layout Feature; the reader and writer's UTF-8
//! Constraint).

use crate::common::fields::{fields, scalar, scalars};
use crate::common::full::full_nested_document;
use crate::common::text::{COMPONENT, FEATURE, PRODUCT, files, path, refused_in, text};
use crate::common::{component, feature, key, product, root, slug, small_document};
use loftspec::codec::{self, Files, Problem, SpecPath};
use loftspec::document::{Document, Edge};
use loftspec::ids::{ComponentId, CriterionId, FeatureGroupId, FeatureId, ProductId};
use loftspec::tree::{Details, FeatureGroup};
use loftspec::yaml::{self, ErrorKind, Node, Value};

const LATER: &str = "products/example-product/identity/later.yaml";

/// The small document's files with the file at `at` holding its record twice, as a list.
fn holding_twice(at: &str) -> Files {
    let record = text(&files(), at);
    let entry = format!("- {}", record.replace('\n', "\n  ").trim_end());
    let mut files = files();
    files.insert(path(at), format!("{entry}\n{entry}\n").into_bytes());
    files
}

#[test]
fn the_product_file_holds_its_feature_groups_in_document_order() {
    let mut document = small_document();
    let group = FeatureGroup {
        id: Some(FeatureGroupId::new(key(51))),
        slug: slug("second"),
        name: "Second".to_owned(),
        details: Details::default(),
        features: Vec::new(),
        constraints: Vec::new(),
        quality_attributes: Vec::new(),
    };
    document.products[0].feature_groups.insert(0, group);
    let files = codec::write(&document);
    let written = ["0", "1"].map(|index| scalar(&files, PRODUCT, &["featureGroups", index, "key"]));
    assert_eq!(written, [key(51).to_string(), key(2).to_string()]);
    let read = codec::read(&files).unwrap();
    let groups: Vec<_> = read.products[0]
        .feature_groups
        .iter()
        .map(|group| group.slug.as_str().to_owned())
        .collect();
    assert_eq!(groups, ["second", "identity"]);
}

#[test]
fn a_feature_group_record_lists_its_features_keys_in_document_order() {
    let mut document = small_document();
    let group = &mut document.products[0].feature_groups[0];
    group
        .features
        .insert(0, feature(key(52), "Later", Vec::new()));
    let files = codec::write(&document);
    let listed = scalars(&files, PRODUCT, &["featureGroups", "0", "features"]);
    assert_eq!(listed, [key(52).to_string(), key(3).to_string()]);
    assert_eq!(scalar(&files, LATER, &["key"]), key(52).to_string());
    let read = codec::read(&files).unwrap();
    let features: Vec<_> = read.products[0].feature_groups[0]
        .features
        .iter()
        .map(|feature| feature.id)
        .collect();
    assert_eq!(
        features,
        [Some(FeatureId::new(key(52))), Some(FeatureId::new(key(3)))]
    );
}

/// Asserts that the file at `at` holds one record: one is read, and two or none are refused.
fn assert_holds_one_record(at: &str) {
    assert_eq!(fields(&files(), at, &[])[..3], ["key", "slug", "name"]);
    refused_in(&holding_twice(at), at);
    let mut empty = files();
    empty.insert(path(at), Vec::new());
    refused_in(&empty, at);
}

#[test]
fn a_feature_file_holds_exactly_one_feature() {
    assert_holds_one_record(FEATURE);
}

#[test]
fn a_component_file_holds_one_root_component() {
    assert_holds_one_record(COMPONENT);
}

#[test]
fn a_component_file_holds_every_component_constraint_and_quality_attribute_beneath_its_root() {
    let files = codec::write(&full_nested_document());
    let nested = ["components", "0"];
    let at = |route: &[&str]| scalar(&files, COMPONENT, &[&nested[..], route].concat());
    assert_eq!(at(&["key"]), key(61).to_string());
    assert_eq!(at(&["components", "0", "key"]), key(66).to_string());
    assert_eq!(at(&["constraints", "0", "key"]), key(64).to_string());
    assert_eq!(at(&["qualityAttributes", "0", "key"]), key(65).to_string());
    let components: Vec<&str> = files
        .keys()
        .map(SpecPath::as_str)
        .filter(|path| path.starts_with("components/"))
        .collect();
    assert_eq!(
        components,
        [COMPONENT],
        "no Component beneath a root has a file"
    );
    for n in [61, 64, 65, 66] {
        let holding: Vec<&str> = files
            .iter()
            .filter(|(_, bytes)| String::from_utf8_lossy(bytes).contains(&key(n).to_string()))
            .map(|(path, _)| path.as_str())
            .filter(|path| *path != "labels.yaml")
            .collect();
        assert_eq!(holding, [COMPONENT], "{n}");
    }
}

/// The small document with a second target, ahead of the first in the document though after
/// it by key, on a `realizes`, a `scopedTo` and a `mentions` edge.
fn with_targets_out_of_key_order() -> Document {
    let mut document = small_document();
    document.products.push(product(key(50), "Second Product"));
    document.components.push(root(component(key(53), "Part")));
    document.edges.splice(
        0..0,
        [
            Edge::Realizes {
                feature: FeatureId::new(key(3)),
                component: ComponentId::new(key(53)),
            },
            Edge::ScopedTo {
                component: ComponentId::new(key(6)),
                product: ProductId::new(key(50)),
            },
            Edge::Mentions {
                source: CriterionId::new(key(5)).into(),
                target: ComponentId::new(key(6)).into(),
            },
        ],
    );
    document
}

#[test]
fn the_targets_of_realizes_scoped_to_and_mentions_are_written_sorted_by_key() {
    let files = codec::write(&with_targets_out_of_key_order());
    let criterion = ["requirements", "0", "criteria", "0", "mentions"];
    let lists = [
        (FEATURE, &["realizes"][..], [6, 53]),
        (COMPONENT, &["scopedTo"], [1, 50]),
        (FEATURE, &criterion, [4, 6]),
    ];
    for (at, route, sorted) in lists {
        let written = scalars(&files, at, route);
        assert_eq!(written, sorted.map(|n| key(n).to_string()), "{route:?}");
    }
}

/// How many empty lists `node` holds: empty sequences, and keys with nothing after them.
fn empty_lists(node: &Node) -> usize {
    match &node.value {
        Value::Mapping(entries) => entries.iter().map(|entry| empty_lists(&entry.value)).sum(),
        Value::Sequence(items) => {
            usize::from(items.is_empty()) + items.iter().map(empty_lists).sum::<usize>()
        }
        Value::Empty => 1,
        Value::Scalar(_) => 0,
    }
}

#[test]
fn an_empty_list_is_not_written() {
    let mut sparse = small_document();
    sparse.tags.clear();
    let sparse = codec::write(&sparse);
    assert_eq!(
        sparse[&path("tags.yaml")],
        b"",
        "an empty registry is an empty file"
    );
    for files in [sparse, codec::write(&full_nested_document())] {
        for (at, bytes) in &files {
            if let Some(node) = yaml::read(bytes).unwrap() {
                assert_eq!(empty_lists(&node), 0, "{}", at.as_str());
            }
        }
    }
}

#[test]
fn the_writer_writes_utf8_and_the_reader_refuses_any_other_encoding() {
    let mut document = small_document();
    document.products[0].name = "Zürich".to_owned();
    let files = codec::write(&document);
    for (at, bytes) in &files {
        assert!(std::str::from_utf8(bytes).is_ok(), "{}", at.as_str());
        assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]), "{}", at.as_str());
    }
    let written = &files[&path(PRODUCT)];
    assert!(written.windows(7).any(|bytes| bytes == b"Z\xC3\xBCrich"));
    assert_eq!(codec::read(&files).unwrap().products[0].name, "Zürich");
    let mut latin1 = files.clone();
    let mut bytes = written.clone();
    let at = bytes
        .windows(2)
        .position(|pair| pair == [0xC3, 0xBC])
        .unwrap();
    bytes.splice(at..at + 2, [0xFC]);
    latin1.insert(path(PRODUCT), bytes);
    let errors = codec::read(&latin1).expect_err("refused");
    assert_eq!(errors[0].problem, Problem::Syntax(ErrorKind::InvalidUtf8));
    assert_eq!(errors[0].sites[0].path, path(PRODUCT));
}
