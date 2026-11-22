//! What a `spec-change` view records of the other documents a connected document relies on: an
//! Interface that a Component gained drawn so that the edge ends at its box, the document's own
//! or a copy's; a copy's Interface drawn as the copy holds it, labeled with its handle; a copy
//! added or removed; and a pin moved, with each node of the promise that the move changed.

use std::fs;

use loftspec::codec::copy_of;
use loftspec::identity::Key;
use loftspec::slug::Slug;
use serde_json::Value;

use crate::common::copies::{HANDLE, copy_lookup};
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
