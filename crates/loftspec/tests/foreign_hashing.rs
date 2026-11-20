//! A reference into another document in the hashes: an edge to a node of a copy enters the
//! edges root by that node's content hash in its copy, and the pins of the copies enter the
//! state view, so that moving a pin changes the state root and a changed promise changes the
//! content root, while a handle is never hashed.

use crate::common::copies::{HANDLE, OWNER, copy_at, owner, requiring, requiring_two, state_root};
use crate::common::interfaces::{INTERFACE, MODEL, provides};
use crate::common::{key, slug, small_document};
use loftspec::canonical::{Digest, Domain, digest};
use loftspec::decision::NodeRef;
use loftspec::document::{Document, Edge, SubjectRole};
use loftspec::hash::{ContentHash, StateView, hash_document, hash_indexed_with_views, state_view};
use loftspec::ids::{DecisionId, RequirementId};
use serde_json::{Value, json};

/// [`requiring`] whose root Component provides the copied Interface and whose decision names
/// the copied Requirement as affected.
fn crossing() -> Document {
    let mut document = requiring();
    document.edges.extend([
        provides(MODEL, INTERFACE),
        Edge::Subject {
            decision: DecisionId::new(key(0x14)),
            subject: NodeRef::Held(RequirementId::new(key(INTERFACE + 1)).into()),
            role: SubjectRole::Affected,
        },
    ]);
    document
}

/// The edges view's tuples of the type `kind`, each as its target's hash.
fn targets(document: &Document, kind: &str) -> Vec<Value> {
    let (_, views) = hash_indexed_with_views(&document.index().unwrap());
    let (_, _, edges) = (views.roots.into_iter())
        .find(|(name, _, _)| *name == "edges")
        .unwrap();
    let tuples = edges.as_array().unwrap().clone();
    let typed = tuples.into_iter().filter(|tuple| tuple[1] == json!(kind));
    typed.map(|tuple| tuple[2].clone()).collect()
}

/// The other document with its Requirement's criterion reworded.
fn changed_owner() -> Document {
    let mut document = owner();
    let answer = &mut document.interfaces[0].requirements[0].criteria[0].name;
    "The answer names the document and its owner.".clone_into(answer);
    document
}

/// [`crossing`] with its copy pinned at `sequence` of `owner`.
fn pinned(owner: &Document, sequence: u64) -> Document {
    let mut document = crossing();
    document.copies = vec![copy_at(owner, HANDLE, sequence)];
    document
}

#[test]
fn an_edge_to_a_foreign_node_enters_the_edges_root_with_the_nodes_hash_in_its_copy() {
    let hash = owner().interfaces[0].content_hash();
    assert_eq!(targets(&crossing(), "provides"), vec![json!(hash)]);
}

#[test]
fn a_foreign_subject_is_named_by_its_content_hash_in_its_copy() {
    let hash = owner().interfaces[0].requirements[0].content_hash();
    assert!(targets(&crossing(), "subject").contains(&json!(hash)));
}

#[test]
fn moving_a_pin_to_a_state_with_the_referenced_node_unchanged_leaves_the_content_root() {
    let (at_three, at_four) = (pinned(&owner(), 3), pinned(&owner(), 4));
    let (three, four) = (
        hash_document(&at_three).unwrap().roots,
        hash_document(&at_four).unwrap().roots,
    );
    assert_eq!(three.content, four.content);
    assert_ne!(
        three.state, four.state,
        "moving a pin changes the state root"
    );
}

#[test]
fn moving_a_pin_to_a_state_with_the_referenced_node_changed_changes_the_content_root() {
    let before = hash_document(&pinned(&owner(), 3)).unwrap().roots;
    let after = hash_document(&pinned(&changed_owner(), 5)).unwrap().roots;
    assert_ne!(before.content, after.content);
}

#[test]
fn renaming_a_handle_leaves_the_content_root_and_the_state_root() {
    let before = hash_document(&crossing()).unwrap().roots;
    let mut renamed = crossing();
    renamed.documents[0].handle = slug("formats");
    renamed.copies[0].handle = slug("formats");
    let after = hash_document(&renamed).unwrap().roots;
    assert_eq!(before.content, after.content);
    assert_eq!(before.state, after.state);
    assert_eq!(before.pins, after.pins);
}

#[test]
fn the_state_view_of_a_tree_with_a_copy_ends_with_its_pins_after_the_lint_digest() {
    let document = crossing();
    let roots = hash_document(&document).unwrap().roots;
    let view = state_view(&document, &roots, StateView::Two, Some("rules: []\n"));
    let members = view.as_array().unwrap();
    let pins = roots.pins.unwrap();
    assert_eq!(members.last(), Some(&json!({ "pins": pins })));
    assert!(
        members[members.len() - 2]
            .get("lintConfiguration")
            .is_some()
    );
    let one = state_view(&document, &roots, StateView::One, None);
    assert_eq!(
        one.as_array().unwrap().last(),
        Some(&json!({ "pins": pins }))
    );
}

#[test]
fn the_pins_digest_covers_each_pins_document_interface_sequence_state_root_and_hash() {
    let document = crossing();
    let pin = json!([
        key(OWNER).to_string(),
        key(INTERFACE).to_string(),
        3,
        state_root(3),
        owner().interfaces[0].content_hash(),
    ]);
    let expected: Digest = digest(Domain::Pins, &json!([pin]));
    assert_eq!(hash_document(&document).unwrap().roots.pins, Some(expected));
}

#[test]
fn the_pins_are_sorted_by_document_key_then_interface_key() {
    let document = requiring_two();
    let (_, views) = hash_indexed_with_views(&document.index().unwrap());
    let (_, domain, pins) = (views.roots.into_iter())
        .find(|(name, _, _)| *name == "pins")
        .unwrap();
    assert_eq!(domain, Domain::Pins);
    let documents: Vec<Value> = pins
        .as_array()
        .unwrap()
        .iter()
        .map(|pin| pin[0].clone())
        .collect();
    assert_eq!(documents, [json!(key(OWNER)), json!(key(OWNER + 1))]);
    assert_eq!(document.documents[1].handle.as_str(), "catalog");
}

#[test]
fn a_tree_without_a_copy_has_the_state_root_it_had_before_copies() {
    let roots = hash_document(&small_document()).unwrap().roots;
    assert_eq!(roots.pins, None);
    let mut connected = small_document();
    connected.connected = true;
    assert_eq!(hash_document(&connected).unwrap().roots.state, roots.state);
}
