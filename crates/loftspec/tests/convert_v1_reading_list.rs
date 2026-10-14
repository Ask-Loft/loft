//! The Reading List export: a small v1 export whose every hash and address the v1 hashing
//! produced, converted end to end (F-13..F-15). It covers every node kind, all four Component
//! scopes, a Component under an external one, Facet Tags, a tag whose v1 name is not a tag name,
//! a constraint category, a quality target, decisions in each of the five states, and prose with
//! a resolvable reference, an unresolvable one, a heading and a tag name in a code span.

use std::collections::BTreeMap;

use crate::common::v1::key;

use loftspec::codec;
use loftspec::convert::v1::export::Export;
use loftspec::convert::v1::{Conversion, Finding, convert};
use loftspec::document::Edge;
use loftspec::hash::hash_document;
use loftspec::identity::LabelPrefix;
use loftspec::ids::{FeatureGroupId, NodeId, RequirementId};

const READING_LIST: &str =
    include_str!("../../../testdata/v1/reading-list/loft-authored-state.json");

/// The state root of the converted document, pinned from this crate's own hashing (the hash
/// views have independent golden vectors under `testdata/golden/`). The same value is asserted
/// where the converter is built with other `serde_json` features, so a canonical form that
/// depends on the build shows there. A change to it is a deliberate format change, never a
/// refresh to make a failure disappear.
const STATE_ROOT: &str = "48b598fb480c28de10fe7b24351754f6aa855c71e82fc8ad7a7f77a077d9caf3";

fn converted() -> Conversion {
    convert(&Export::from_json(READING_LIST).unwrap()).unwrap()
}

#[test]
fn every_node_but_the_facet_tags_and_every_decision_is_bound() {
    let document = converted().document;
    let nodes = document.bindings.len() - document.decisions.len();
    assert_eq!(nodes, 28, "32 v1 nodes less 4 Facet Tags");
    assert_eq!(document.decisions.len(), 5);
    assert_eq!(document.products.len(), 1);
    assert_eq!(document.components.len(), 3);
}

#[test]
fn the_edges_are_the_links_the_scope_the_subjects_and_the_references() {
    let edges = converted().document.edges;
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for edge in &edges {
        *kinds.entry(edge.kind().as_str()).or_default() += 1;
    }
    let expected = BTreeMap::from([
        ("mentions", 3),
        ("realizes", 3),
        ("scoped_to", 1),
        ("subject", 4),
    ]);
    assert_eq!(kinds, expected);
    assert!(
        edges
            .iter()
            .all(|edge| !matches!(edge, Edge::ScopedTo { .. }) || edge.source().key() == key(0x3c))
    );
}

#[test]
/// The criterion that cites `RQ-99` names a label above every Requirement label of the export,
/// which is no reference (AC-891).
fn the_findings_are_one_text_statement_and_one_heading_in_export_order() {
    assert_eq!(
        converted().findings,
        [
            Finding::StatementAsText {
                requirement: RequirementId::new(key(0x30))
            },
            Finding::HeadingInBody {
                node: NodeId::FeatureGroup(FeatureGroupId::new(key(0x15))),
                line: 3,
            },
        ]
    );
}

#[test]
fn each_counter_is_one_above_the_highest_label_and_facet_tags_count_in_the_sidecar() {
    let conversion = converted();
    let next = |prefix| conversion.document.counters.next(prefix).get();
    assert_eq!(next(LabelPrefix::Requirement), 7);
    assert_eq!(next(LabelPrefix::Criterion), 8);
    assert_eq!(next(LabelPrefix::Component), 6);
    assert_eq!(next(LabelPrefix::Decision), 6);
    assert_eq!(conversion.sidecar.highest["FT"].get(), 4);
}

#[test]
fn the_sidecar_keeps_the_v1_name_behind_the_one_renamed_tag() {
    let tag_names = converted().sidecar.tag_names;
    let renamed: Vec<(&str, &str)> = tag_names
        .iter()
        .map(|(tag, v1_name)| (tag.as_str(), v1_name.as_str()))
        .collect();
    assert_eq!(renamed, [("shelf-life", "Shelf life")]);
}

#[test]
fn two_conversions_write_identical_sidecars() {
    assert_eq!(converted().sidecar.to_json(), converted().sidecar.to_json());
}

#[test]
fn two_conversions_write_byte_identical_spec_trees() {
    let first = codec::write(&converted().document);
    assert!(first.len() > 10, "the tree holds {} files", first.len());
    assert_eq!(first, codec::write(&converted().document));
}

#[test]
fn the_converted_document_hashes_to_its_pinned_state_root() {
    let hashes = hash_document(&converted().document).unwrap();
    assert_eq!(hashes.roots.state.to_string(), STATE_ROOT);
}
