//! Editing a document held in memory (Library entry points): finding a node or a decision by
//! its key, and detaching a node with every node beneath it.

use crate::common::editing::{
    GROUP, KEYS, LABELS, NESTED, SLUGS, beneath, changes, editable, feature_id, features, number,
};
use crate::common::key;
use loftspec::document::{Document, Edge};
use loftspec::edit::{self, Found};
use loftspec::ids::{ComponentId, NodeId};
use loftspec::node::Node;

/// The node `n` finds, which must be a node.
fn found_node(document: &Document, n: u32) -> Node<'_> {
    match edit::find(document, key(n)) {
        Some(Found::Node(node)) => node,
        other => panic!("key {n} found {other:?}"),
    }
}

/// The edges of `document` whose source is one of `numbers`.
fn edges_from(document: &Document, numbers: &[u32]) -> Vec<Edge> {
    let from = |edge: &Edge| numbers.contains(&number(edge.source().key()));
    document.edges.iter().copied().filter(from).collect()
}

#[test]
fn a_nodes_key_finds_that_node_at_any_depth() {
    let document = editable();
    for n in [1, GROUP, KEYS, 4, 5, 10, 6, NESTED, 66, 64, 41] {
        let node = found_node(&document, n);
        assert_eq!(node.id().map(NodeId::key), Some(key(n)), "key {n}");
    }
    assert_eq!(found_node(&document, KEYS).name(), "Keys");
}

#[test]
fn a_decisions_key_finds_that_decision() {
    let document = editable();
    let Some(Found::Decision(decision)) = edit::find(&document, key(20)) else {
        panic!("the decision is found");
    };
    assert_eq!(decision, &document.decisions[0]);
}

#[test]
fn a_key_the_document_does_not_hold_finds_nothing() {
    let document = editable();
    assert_eq!(edit::find(&document, key(0x1ff)), None);
    let mut detached = editable();
    edit::detach(&mut detached, feature_id(SLUGS)).unwrap();
    assert_eq!(edit::find(&detached, key(SLUGS)), None);
}

#[test]
fn finding_changes_nothing_in_the_document() {
    let document = editable();
    let before = document.clone();
    for n in [KEYS, 20, 0x1ff] {
        let _ = edit::find(&document, key(n));
    }
    assert_eq!(document, before);
}

#[test]
fn the_detached_node_is_returned_with_every_node_beneath_it_and_the_edges_they_write() {
    let mut document = editable();
    let before = document.clone();
    let numbers = beneath(found_node(&before, KEYS));
    assert!(numbers.len() > 4, "{numbers:?}");
    let subtree = edit::detach(&mut document, feature_id(KEYS)).unwrap();
    assert_eq!(subtree.node(), found_node(&before, KEYS));
    assert_eq!(beneath(subtree.node()), numbers);
    assert_eq!(subtree.edges(), edges_from(&before, &numbers));
    assert_ne!(subtree.edges(), []);
}

#[test]
fn the_document_holds_no_detached_node_after_the_detach() {
    let mut document = editable();
    let numbers = beneath(found_node(&document, KEYS));
    edit::detach(&mut document, feature_id(KEYS)).unwrap();
    for n in numbers {
        assert_eq!(edit::find(&document, key(n)), None, "key {n}");
    }
    assert_eq!(
        edges_from(&document, &beneath(found_node(&editable(), KEYS))),
        []
    );
}

#[test]
fn every_detached_node_keeps_its_key() {
    let mut document = editable();
    let nested = ComponentId::new(key(NESTED)).into();
    let numbers = beneath(found_node(&document, NESTED));
    let subtree = edit::detach(&mut document, nested).unwrap();
    let kept: Vec<u32> = subtree
        .ids()
        .into_iter()
        .map(|id| number(id.key()))
        .collect();
    assert_eq!(kept, numbers);
    assert_eq!(subtree.id(), Some(nested));
}

#[test]
fn the_detached_nodes_former_siblings_keep_their_order() {
    let mut document = editable();
    assert_eq!(features(&document, 0), [KEYS, LABELS, SLUGS]);
    edit::detach(&mut document, feature_id(LABELS)).unwrap();
    assert_eq!(features(&document, 0), [KEYS, SLUGS]);
}

#[test]
fn a_detach_changes_no_other_node_and_no_decision() {
    let before = editable();
    let mut document = before.clone();
    edit::detach(&mut document, feature_id(KEYS)).unwrap();
    let changes = changes(&before, &document);
    let mut removed = beneath(found_node(&before, KEYS));
    removed.sort_unstable();
    assert_eq!(changes.removed, removed);
    assert!(changes.added.is_empty() && changes.changed.is_empty() && changes.moved.is_empty());
    assert_eq!(document.decisions, before.decisions);
}
