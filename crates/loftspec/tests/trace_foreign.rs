//! Answering a foreign Interface: the criteria beneath a copied Interface that a Component of the
//! document provides are obligations of the document, judged against their content hash in the
//! copy and written after the links to the document's own nodes.

use crate::common::copies::{HANDLE, SHELF, copy_at, owner, requiring, requiring_two};
use crate::common::interfaces::{INTERFACE, MODEL, provides, requires};
use crate::common::key;
use crate::common::trace::{SOURCE, all_reviewed, files, found, hashes, run, tested};
use loftspec::document::Document;
use loftspec::identity::Key;
use loftspec::ids::{CriterionId, NodeId};
use loftspec::trace::{Link, Trace, TraceRule, write};

/// The copied criterion.
const ANSWER: u32 = INTERFACE + 2;

/// [`requiring`] whose root Component provides the copied Interface.
fn answering() -> Document {
    let mut document = requiring();
    document.edges.push(provides(MODEL, INTERFACE));
    document
}

/// The tests of `tests/a.rs`, with one for the copied criterion.
fn source() -> String {
    format!("{SOURCE}fn answer() {{}}\n")
}

/// The links of the small document's three nodes in scope, and one to the copied criterion,
/// each reviewed against its hash in `document`.
fn links(document: &Document) -> Vec<Link> {
    let mut links = all_reviewed(document);
    let id = NodeId::from(CriterionId::new(key(ANSWER)));
    links.push(Link {
        reviewed: Some(hashes(document).foreign[&id]),
        ..tested(ANSWER, "answer")
    });
    links
}

/// [`answering`] with its copy pinned at `sequence` of `owner`.
fn pinned(owner: &Document, sequence: u64) -> Document {
    let mut document = answering();
    document.copies = vec![copy_at(owner, HANDLE, sequence)];
    document
}

#[test]
fn each_criterion_beneath_a_provided_copied_interface_is_in_scope() {
    let document = answering();
    let reconciled = run(&document, all_reviewed(&document), &files(&source()));
    assert_eq!(found(&reconciled), [(TraceRule::Tr001, key(ANSWER))]);
    assert_eq!(reconciled.in_scope, 4);
    let reconciled = run(&document, links(&document), &files(&source()));
    assert_eq!(found(&reconciled), []);
}

#[test]
fn a_criterion_beneath_a_copied_interface_no_component_provides_is_not_in_scope() {
    let mut document = requiring();
    document.edges.push(requires(MODEL, INTERFACE));
    let reconciled = run(&document, all_reviewed(&document), &files(&source()));
    assert_eq!(found(&reconciled), []);
    assert_eq!(reconciled.in_scope, 3);
}

#[test]
fn a_link_reviewed_before_its_pin_moved_to_a_changed_criterion_is_stale() {
    let before = pinned(&owner(), 3);
    let reviewed = links(&before);
    let mut changed = owner();
    let answer = &mut changed.interfaces[0].requirements[0].criteria[0].name;
    "The answer names the document and its owner.".clone_into(answer);
    let after = pinned(&changed, 5);
    let reconciled = run(&after, reviewed.clone(), &files(&source()));
    assert_eq!(found(&reconciled), [(TraceRule::Tr003, key(ANSWER))]);
    let unchanged = pinned(&owner(), 4);
    assert_eq!(found(&run(&unchanged, reviewed, &files(&source()))), []);
}

/// The nodes of the links the trace file is written with, in order.
fn written_order(document: &Document, links: Vec<Link>) -> Vec<Key> {
    let text = write(&Trace { links }, document);
    (text.lines())
        .filter_map(|line| line.trim_start().strip_prefix("- node: "))
        .map(|key| Key::parse(key).unwrap())
        .collect()
}

#[test]
fn links_to_foreign_criteria_follow_the_documents_own_by_handle_then_copy_order() {
    let mut document = requiring_two();
    document
        .edges
        .extend([provides(MODEL, INTERFACE), provides(MODEL, SHELF)]);
    let gone = 0x999;
    let links = vec![
        tested(ANSWER, "answer"),
        tested(gone, "gone"),
        tested(SHELF + 2, "shelf"),
        tested(5, "five"),
    ];
    let order = written_order(&document, links);
    assert_eq!(order, [key(5), key(SHELF + 2), key(ANSWER), key(gone)]);
}
