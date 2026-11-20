//! A reference into another document as the projections show it, `<handle>:<label>` and the
//! node's name, and every reader of a document's edges taking one whole: rendering, comparing,
//! reviewing, linting and validating a document that references other documents.

use crate::common::comparing::version;
use crate::common::copies::{HANDLE, requiring};
use crate::common::interfaces::{INTERFACE, MODEL, provides, requires};
use crate::common::key;
use crate::common::lint::findings;
use loftspec::decision::NodeRef;
use loftspec::diff::compare;
use loftspec::diff::{content_changes, node_changes};
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::ids::{CriterionId, DecisionId, ElementId, RequirementId};
use loftspec::render::openspec::openspec;
use loftspec::render::{render, render_tree};
use loftspec::review::pack;
use loftspec::validate::validate;

/// [`requiring`] with a foreign edge of each kind: its root Component provides the copied
/// Interface, its criterion mentions the copied criterion, and its decision names the copied
/// Requirement as affected.
fn crossing() -> Document {
    let mut document = requiring();
    document.edges.extend([
        provides(MODEL, INTERFACE),
        Edge::Mentions {
            source: ElementId::Node(CriterionId::new(key(5)).into()),
            target: ElementId::Node(CriterionId::new(key(INTERFACE + 2)).into()),
        },
        Edge::Subject {
            decision: DecisionId::new(key(0x14)),
            subject: NodeRef::Held(RequirementId::new(key(INTERFACE + 1)).into()),
            role: SubjectRole::Affected,
        },
    ]);
    document
}

#[test]
fn a_foreign_node_with_a_label_renders_as_its_handle_and_label_then_its_name() {
    let markdown = render(&crossing()).unwrap().markdown;
    assert!(
        markdown.contains(&format!("Provides: {HANDLE}:IF-1 · Document lookup")),
        "{markdown}"
    );
    let label = crossing().copies[0].labels[&key(INTERFACE + 1)];
    let subject = format!("`{HANDLE}:{label}` · The lookup shall answer a document by its key.");
    assert!(markdown.contains(&subject), "{markdown}");
}

#[test]
fn a_foreign_node_without_a_label_renders_as_its_handle_then_its_name() {
    let mut document = crossing();
    document.copies[0].labels.remove(&key(INTERFACE));
    let markdown = render(&document).unwrap().markdown;
    assert!(
        markdown.contains(&format!("Provides: {HANDLE} · Document lookup")),
        "{markdown}"
    );
}

#[test]
fn every_reader_of_edges_takes_a_document_that_references_another() {
    let document = crossing();
    let mut also = crossing();
    also.edges.push(requires(MODEL, INTERFACE));
    assert_eq!(validate(&document), vec![]);
    let projections = render_tree(&document, None).unwrap();
    let json = String::from_utf8(projections.json).unwrap();
    assert!(json.contains(&key(INTERFACE).to_string()));
    openspec(&document).unwrap();
    let base = requiring();
    compare(version(&base), version(&document)).unwrap();
    compare(version(&document), version(&base)).unwrap();
    assert!(!node_changes(&base, &document).is_empty());
    assert!(!content_changes(&base, &document).is_empty());
    pack(&document, Some(&base), None).unwrap();
    pack(&document, None, None).unwrap();
    assert!(findings(&document).len() < 100);
    assert!(findings(&also).len() < 100);
}
