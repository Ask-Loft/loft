//! What a `spec-change` view records of the other documents a connected document relies on: an
//! Interface that a Component gained drawn so that the edge ends at its box, the document's own
//! or a copy's; a copy's Interface drawn as the copy holds it, labeled with its handle; a copy
//! added or removed; and a pin moved, with each node of the promise that the move changed.

use std::fs;

use loftspec::codec::copy_of;
use loftspec::identity::Key;
use loftspec::slug::Slug;
use serde_json::Value;

use crate::common::copies::{HANDLE, OTHER_REMOTE, copy_lookup};
use crate::common::others::{ADDED, COPY, DROPPED, shelves};
use crate::common::remote::spec;
use crate::interfaces::{LOOKUP, SHELF_STORE, add_lookup};
use crate::yard::Yard;
use crate::yard_view_constraints::diagram;
use crate::yard_view_reports::node;

/// The Shelf store's file, under `spec/`.
const STORE: &str = "components/shelf-store.yaml";
/// The Shelf store's `scopedTo`, after which its other fields follow.
const SCOPE: &str = "scopedTo:\n  - 00000000-0000-4000-8000-00000000000a\n";
/// The criterion of "Shelf lookup" that the second state rewords.
const ANSWER: &str = "00000000-0000-4000-8000-0000000000b2";

/// The edges of `graph` of `kind`, as (from, to).
fn edges(graph: &Value, kind: &str) -> Vec<(String, String)> {
    graph["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|edge| edge["kind"] == kind)
        .map(|edge| {
            let end = |field: &str| edge[field].as_str().unwrap().to_owned();
            (end("from"), end("to"))
        })
        .collect()
}

/// The box whose key is `key`.
fn keyed<'a>(graph: &'a Value, key: &str) -> &'a Value {
    graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["key"] == key)
        .unwrap_or_else(|| panic!("no box {key}: {graph}"))
}

/// Writes the copy of "Shelf lookup" as the other document published it in the state of
/// publish sequence `sequence`, 1 or 2.
fn pin_at(yard: &Yard, sequence: u64) {
    let history = shelves();
    let state = &history.states[usize::try_from(sequence).unwrap() - 1];
    let root = state.lock["stateRoot"].as_str().unwrap().parse().unwrap();
    let handle = Slug::parse(HANDLE).unwrap();
    let lookup = Key::parse(LOOKUP).unwrap();
    let copy = copy_of(&handle, &spec(&state.files), lookup, sequence, root).unwrap();
    fs::write(yard.scratch.file(COPY), &copy.bytes).unwrap();
}

#[test]
fn a_component_that_gained_a_copied_interface_draws_it_labeled_with_its_handle() {
    let mut yard = Yard::with_spec();
    copy_lookup(&yard.scratch);
    yard.rebased();
    yard.scratch.edit(
        STORE,
        SCOPE,
        &format!("{SCOPE}requires:\n  - document: {HANDLE}\n    key: {LOOKUP}\n"),
    );
    let graph = diagram(&yard);
    assert_eq!(node(&graph, "Shelf store")["change"], "changed", "{graph}");
    let lookup = keyed(&graph, LOOKUP);
    assert_eq!(lookup["kind"], "interface", "{lookup}");
    assert_eq!(lookup["label"], format!("{HANDLE}:IF-1"), "{lookup}");
    assert_eq!(lookup["document"], HANDLE, "{lookup}");
    assert_eq!(lookup["change"], Value::Null, "drawn for context: {lookup}");
    assert_eq!(
        edges(&graph, "requires"),
        [(SHELF_STORE.to_owned(), LOOKUP.to_owned())]
    );
}

#[test]
fn a_component_that_gained_its_own_interface_draws_it_so_that_the_edge_ends_at_its_box() {
    let mut yard = Yard::with_spec();
    add_lookup(&yard.scratch);
    let provides = format!("{SCOPE}provides:\n  - {LOOKUP}\n");
    yard.scratch.edit(STORE, &provides, SCOPE);
    yard.rebased();
    yard.scratch.edit(STORE, SCOPE, &provides);
    let graph = diagram(&yard);
    let lookup = keyed(&graph, LOOKUP);
    assert_eq!(lookup["change"], Value::Null, "{lookup}");
    assert!(lookup.get("document").is_none(), "{lookup}");
    assert_eq!(
        edges(&graph, "provides"),
        [(SHELF_STORE.to_owned(), LOOKUP.to_owned())]
    );
}

#[test]
fn a_copy_added_or_removed_draws_its_interface_added_or_removed() {
    for added in [true, false] {
        let mut yard = Yard::with_spec();
        copy_lookup(&yard.scratch);
        let bytes = fs::read(yard.scratch.file(COPY)).unwrap();
        if added {
            fs::remove_file(yard.scratch.file(COPY)).unwrap();
            yard.rebased();
            fs::write(yard.scratch.file(COPY), &bytes).unwrap();
        } else {
            yard.rebased();
            fs::remove_file(yard.scratch.file(COPY)).unwrap();
        }
        let graph = diagram(&yard);
        let lookup = keyed(&graph, LOOKUP);
        let change = if added { "added" } else { "removed" };
        assert_eq!(lookup["change"], change, "{lookup}");
        assert_eq!(lookup["document"], HANDLE, "{lookup}");
    }
}

#[test]
fn a_moved_pin_draws_its_interface_changed_with_each_node_its_promise_changed() {
    let mut yard = Yard::with_spec();
    copy_lookup(&yard.scratch);
    pin_at(&yard, 1);
    yard.rebased();
    pin_at(&yard, 2);
    let graph = diagram(&yard);
    assert_eq!(keyed(&graph, LOOKUP)["change"], "changed", "{graph}");
    for (key, change) in [(ANSWER, "changed"), (DROPPED, "removed"), (ADDED, "added")] {
        let drawn = keyed(&graph, key);
        assert_eq!(drawn["change"], change, "{drawn}");
        assert_eq!(drawn["document"], HANDLE, "{drawn}");
    }
}

/// The entry of the diff whose key is `key`.
fn entry<'a>(graph: &'a Value, key: &str) -> &'a Value {
    graph["diff"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["key"] == key)
        .unwrap_or_else(|| panic!("no diff of {key}: {graph}"))
}

#[test]
fn the_diff_lists_a_moved_pin_with_its_publish_sequences_and_each_node_its_promise_changed() {
    let mut yard = Yard::with_spec();
    copy_lookup(&yard.scratch);
    pin_at(&yard, 1);
    yard.rebased();
    pin_at(&yard, 2);
    let graph = diagram(&yard);
    let lookup = entry(&graph, LOOKUP);
    assert_eq!(lookup["change"], "changed", "{lookup}");
    assert_eq!(lookup["label"], format!("{HANDLE}:IF-1"), "{lookup}");
    assert_eq!(lookup["name"], "Shelf lookup", "{lookup}");
    assert_eq!(lookup["document"], HANDLE, "{lookup}");
    assert_eq!(
        lookup["pin"],
        serde_json::json!({ "before": 1, "after": 2 })
    );
    let answer = entry(&graph, ANSWER);
    assert_eq!(answer["change"], "changed", "{answer}");
    assert_eq!(
        answer["words"]["after"],
        "The returned book has the title and the author asked for."
    );
    assert_eq!(entry(&graph, DROPPED)["change"], "removed");
    assert_eq!(entry(&graph, ADDED)["change"], "added");
}

#[test]
fn the_diff_lists_a_copy_added_with_the_publish_sequence_of_its_pin() {
    let mut yard = Yard::with_spec();
    copy_lookup(&yard.scratch);
    let bytes = fs::read(yard.scratch.file(COPY)).unwrap();
    fs::remove_file(yard.scratch.file(COPY)).unwrap();
    yard.rebased();
    fs::write(yard.scratch.file(COPY), &bytes).unwrap();
    let graph = diagram(&yard);
    let lookup = entry(&graph, LOOKUP);
    assert_eq!(lookup["change"], "added", "{lookup}");
    assert_eq!(
        lookup["pin"],
        serde_json::json!({ "before": null, "after": 2 })
    );
}

#[test]
fn the_diff_lists_an_entry_of_documents_added_with_its_address() {
    let yard = Yard::with_spec();
    copy_lookup(&yard.scratch);
    let graph = diagram(&yard);
    let listed = entry(&graph, &format!("document:{HANDLE}"));
    assert_eq!(listed["kind"], "document", "{listed}");
    assert_eq!(listed["name"], HANDLE, "{listed}");
    assert_eq!(listed["change"], "added", "{listed}");
    assert_eq!(listed["words"]["after"], OTHER_REMOTE, "{listed}");
    assert!(listed["words"].get("before").is_none(), "{listed}");
}

#[test]
fn the_diff_lists_an_entry_of_documents_readdressed_with_its_addresses_before_and_after() {
    let mut yard = Yard::with_spec();
    copy_lookup(&yard.scratch);
    yard.rebased();
    let moved = "https://shelves.example/loft/t/plant/api/v2/documents/shelves/remote";
    yard.scratch.edit("loftspec.yaml", OTHER_REMOTE, moved);
    let graph = diagram(&yard);
    let listed = entry(&graph, &format!("document:{HANDLE}"));
    assert_eq!(listed["change"], "changed", "{listed}");
    assert_eq!(listed["words"]["before"], OTHER_REMOTE, "{listed}");
    assert_eq!(listed["words"]["after"], moved, "{listed}");
}

/// The keys of the diff's entries.
fn listed(graph: &Value) -> Vec<String> {
    (graph["diff"].as_array().unwrap().iter())
        .map(|entry| entry["key"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn a_copy_that_moved_to_another_handle_at_the_same_pin_is_neither_added_nor_removed() {
    let mut yard = Yard::with_spec();
    copy_lookup(&yard.scratch);
    yard.rebased();
    let documents = yard.scratch.file("documents");
    fs::rename(documents.join(HANDLE), documents.join("stock")).unwrap();
    let handle = format!("handle: {HANDLE}\n");
    yard.scratch
        .edit("loftspec.yaml", &handle, "handle: stock\n");
    let graph = diagram(&yard);
    let keys = listed(&graph);
    assert!(!keys.contains(&LOOKUP.to_owned()), "{keys:?}");
    assert_eq!(
        entry(&graph, &format!("document:{HANDLE}"))["change"],
        "removed"
    );
    assert_eq!(entry(&graph, "document:stock")["change"], "added");
}

#[test]
fn a_copy_whose_bytes_changed_at_the_same_pin_is_changed_with_no_pin_in_the_diff() {
    let mut yard = Yard::with_spec();
    copy_lookup(&yard.scratch);
    pin_at(&yard, 1);
    yard.rebased();
    let text = fs::read_to_string(yard.scratch.file(COPY)).unwrap();
    let root = (text.lines())
        .find_map(|line| line.strip_prefix("stateRoot: "))
        .unwrap();
    yard.scratch.edit(COPY, root, &"cd".repeat(32));
    let graph = diagram(&yard);
    let lookup = entry(&graph, LOOKUP);
    assert_eq!(lookup["change"], "changed", "{lookup}");
    assert!(lookup.get("pin").is_none(), "{lookup}");
}
