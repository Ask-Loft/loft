//! The containment profile (F-17) as the text form holds it: a child of each kind is read only
//! in a record whose kind the profile lets hold it, while a Section stands under a node of every
//! kind and a `mentions` edge joins any two elements.

use std::collections::BTreeSet;

use crate::common::full::{full_document, full_files};
use crate::common::key;
use crate::common::text::{COMPONENT, DECISION, FEATURE, PRODUCT, path, refused_in, text};
use loftspec::codec::{self, Files, Problem};
use loftspec::document::{Document, Edge};
use loftspec::hash::hash_document;
use loftspec::ids::{DecisionId, ElementId};
use loftspec::kind::NodeKind;
use loftspec::validate::validate;

/// The field that holds the children of each structural kind.
const CHILD_FIELDS: [&str; 7] = [
    "featureGroups",
    "features",
    "requirements",
    "criteria",
    "components",
    "constraints",
    "qualityAttributes",
];

const COMPONENT_CHILDREN: &[&str] = &["components", "constraints", "qualityAttributes"];

/// Each keyed record of the full document's files: what it is, its file, its key, and the child
/// fields the profile lets its kind hold.
const RECORDS: [(&str, &str, u32, &[&str]); 13] = [
    ("product", PRODUCT, 1, &["featureGroups", "constraints"]),
    ("feature group", PRODUCT, 2, &["features", "constraints"]),
    ("constraint of a product", PRODUCT, 42, &[]),
    ("section", PRODUCT, 40, &[]),
    (
        "feature",
        FEATURE,
        3,
        &["requirements", "qualityAttributes"],
    ),
    ("requirement", FEATURE, 4, &["criteria"]),
    ("criterion", FEATURE, 5, &[]),
    ("quality attribute of a feature", FEATURE, 10, &[]),
    ("root component", COMPONENT, 6, COMPONENT_CHILDREN),
    ("nested component", COMPONENT, 61, COMPONENT_CHILDREN),
    ("constraint of a component", COMPONENT, 7, &[]),
    ("quality attribute of a component", COMPONENT, 8, &[]),
    ("decision", DECISION, 20, &[]),
];

/// The full document's files with `field`, holding one item, added to the record keyed `n` in
/// the file at `at`, after its key; with the site of the added field's name.
fn with_field(at: &str, n: u32, field: &str) -> (Files, String) {
    let mut files = full_files();
    let current = text(&files, at);
    let key_line = format!("key: {}", key(n));
    let (index, line) = current
        .lines()
        .enumerate()
        .find(|(_, line)| line.ends_with(&key_line))
        .unwrap_or_else(|| panic!("{key_line} in {at}"));
    let indent = " ".repeat(line.len() - key_line.len());
    let added = format!("{indent}{field}:\n{indent}  - {}\n", key(99));
    let mut lines: Vec<String> = current.lines().map(|line| format!("{line}\n")).collect();
    lines.insert(index + 1, added);
    files.insert(path(at), lines.concat().into_bytes());
    let site = format!("spec/{at}:{}:{}", index + 2, indent.len() + 1);
    (files, site)
}

#[test]
fn a_child_field_is_read_only_in_a_record_whose_kind_the_profile_lets_hold_it() {
    for (record, at, n, allowed) in RECORDS {
        for field in CHILD_FIELDS.iter().filter(|field| !allowed.contains(field)) {
            let (files, site) = with_field(at, n, field);
            let errors = codec::read(&files).expect_err(&format!("{field} under a {record}"));
            assert_eq!(errors.len(), 1, "{field} under a {record}: {errors:?}");
            assert_eq!(
                errors[0].problem,
                Problem::UnknownField((*field).to_owned()),
                "{field} under a {record}"
            );
            assert_eq!(
                errors[0].sites[0].to_string(),
                site,
                "{field} under a {record}"
            );
        }
    }
}

/// The small document's Feature file, in its Product's directory rather than its group's.
const MISPLACED: &str = "products/example-product/keys.yaml";

#[test]
fn a_feature_file_outside_a_feature_groups_directory_is_refused() {
    let mut files = full_files();
    let feature = files.remove(&path(FEATURE)).unwrap();
    files.insert(path(MISPLACED), feature);
    assert_eq!(refused_in(&files, MISPLACED).problem, Problem::UnknownFile);
}

/// The kinds of the nodes that hold at least one Section.
fn kinds_holding_sections(document: &Document) -> BTreeSet<NodeKind> {
    let mut kinds = BTreeSet::new();
    document.walk(&mut |node| {
        if !node.sections().is_empty() {
            kinds.insert(node.kind());
        }
    });
    kinds
}

#[test]
fn a_section_under_a_node_of_every_kind_and_under_a_section_is_accepted() {
    let document = full_document();
    assert_eq!(
        kinds_holding_sections(&document),
        NodeKind::ALL.into_iter().collect()
    );
    assert_eq!(validate(&document), Vec::new());
    let back = codec::read(&full_files()).unwrap();
    assert_eq!(
        hash_document(&back).unwrap().roots,
        hash_document(&document).unwrap().roots
    );
}

/// Every node of the document, then its decisions.
fn elements(document: &Document) -> Vec<ElementId> {
    let mut elements = Vec::new();
    document.walk(&mut |node| elements.push(node.id().unwrap().into()));
    elements.extend(
        document
            .decisions
            .iter()
            .map(|decision| ElementId::from(decision.id)),
    );
    elements
}

#[test]
fn a_mentions_edge_between_any_two_elements_is_accepted() {
    let mut document = full_document();
    let elements = elements(&document);
    assert!(elements.contains(&DecisionId::new(key(20)).into()));
    for &source in &elements {
        for &target in elements.iter().filter(|target| **target != source) {
            let edge = Edge::Mentions { source, target };
            if !document.edges.contains(&edge) {
                document.edges.push(edge);
            }
        }
    }
    let pairs = elements.len() * (elements.len() - 1);
    let mentions = |document: &Document| {
        let is_mention = |edge: &&Edge| matches!(edge, Edge::Mentions { .. });
        document.edges.iter().filter(is_mention).count()
    };
    assert_eq!(mentions(&document), pairs);
    assert_eq!(validate(&document), Vec::new());
    let back = codec::read(&codec::write(&document)).unwrap();
    assert_eq!(mentions(&back), pairs);
    assert_eq!(
        hash_document(&back).unwrap().roots,
        hash_document(&document).unwrap().roots
    );
}
