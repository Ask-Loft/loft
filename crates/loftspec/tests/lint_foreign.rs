//! A citation of another document's label in prose, `<handle>:<label>`: `V2-003` accepts it
//! where a `mentions` edge names the node the copies under that handle bind to the label, reports
//! it otherwise, and never matches it against the document's own labels.

use crate::common::copies::{HANDLE, requiring};
use crate::common::interfaces::INTERFACE;
use crate::common::key;
use crate::common::lint::findings;
use loftspec::document::{Document, Edge};
use loftspec::ids::{CriterionId, ElementId, NodeId, RequirementId};
use loftspec::lint::RuleId;
use loftspec::prose::label_spans;

/// The small document's criterion, keyed 5.
fn criterion() -> ElementId {
    ElementId::Node(CriterionId::new(key(5)).into())
}

/// [`requiring`] whose criterion cites `citation` in its name, mentioning each of `mentioned`.
fn citing(citation: &str, mentioned: &[NodeId]) -> Document {
    let mut document = requiring();
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.requirements[0].criteria[0].name =
        format!("Every node record carries a `key` field, as {citation} says.");
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Mentions { .. }));
    let mentions = mentioned.iter().map(|target| Edge::Mentions {
        source: criterion(),
        target: ElementId::Node(*target),
    });
    document.edges.extend(mentions);
    document
}

/// The `V2-003` findings, each as its term.
fn unmatched(document: &Document) -> Vec<String> {
    (findings(document).into_iter())
        .filter(|found| found.rule == RuleId::V2003)
        .filter_map(|found| found.term)
        .collect()
}

/// The copied Interface's label, `IF-1`.
fn copied() -> NodeId {
    loftspec::ids::InterfaceId::new(key(INTERFACE)).into()
}

#[test]
fn a_foreign_citation_with_a_mentions_edge_to_its_node_yields_no_finding() {
    let citation = format!("{HANDLE}:IF-1");
    assert_eq!(
        unmatched(&citing(&citation, &[copied()])),
        Vec::<String>::new()
    );
}

#[test]
fn a_foreign_citation_without_such_an_edge_yields_one_finding_naming_it() {
    let citation = format!("{HANDLE}:IF-1");
    assert_eq!(unmatched(&citing(&citation, &[])), [citation]);
}

#[test]
fn a_foreign_citation_is_never_matched_against_the_documents_own_labels() {
    let own = NodeId::from(RequirementId::new(key(4)));
    let document = requiring();
    assert_eq!(
        document.bindings.label_of_key(key(4)).unwrap().to_string(),
        "RQ-2"
    );
    let citation = format!("{HANDLE}:RQ-2");
    assert_eq!(unmatched(&citing(&citation, &[own])), [citation]);
    assert_eq!(unmatched(&citing("RQ-2", &[own])), Vec::<String>::new());
}

#[test]
fn a_citation_under_a_handle_is_scanned_with_its_handle() {
    let spans = label_spans("as format:IF-1 says, and RQ-2 and Format:AC-1 and x_y:AC-2");
    let handles: Vec<Option<&str>> = spans.iter().map(|span| span.handle.as_deref()).collect();
    assert_eq!(handles, [Some("format"), None, None, None]);
    assert_eq!(spans[0].label.to_string(), "IF-1");
}
