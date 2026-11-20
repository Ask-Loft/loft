//! `loft interface update`: moves the pins of the copies it names to their remote's head, or one
//! copy to the publish sequence `--at` names, and reports each node of each promise the move
//! added, changed and removed; a withdrawn Interface stays at its pin, and a reference left
//! naming a node no copy holds is named.

use std::fs;

use serde_json::Value;
use software_plant::remote::Remote;

use crate::common::others::COPY;
use crate::common::{Scratch, stderr, stdout};
use crate::interface_add::{conforming, interface, ready};

/// The copy of "Shelf stock" under `spec/`.
const STOCK: &str = "documents/shelves/shelf-stock.yaml";
const STORE: &str = "components/shelf-store.yaml";
const SCOPE: &str = "scopedTo:\n  - 00000000-0000-4000-8000-00000000000a\n";

/// A tree holding a copy of each Interface `named` at publish sequence 1, one state behind the
/// other document's head, and the other document's remote, which serves while it is held.
fn pinned_at_first(named: &[&str]) -> (Scratch, Remote) {
    let (scratch, _history, remote) = ready(true);
    for name in named {
        let output = interface(&scratch, &["add", name, "--at", "1"]);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    }
    (scratch, remote)
}

fn update(scratch: &Scratch, args: &[&str]) -> (i32, Value) {
    let mut all = vec!["update"];
    all.extend(args);
    all.extend(["--format", "json"]);
    let output = interface(scratch, &all);
    (output.status.code().unwrap(), conforming(&output))
}

fn names(nodes: &Value) -> Vec<&str> {
    (nodes.as_array().unwrap().iter())
        .map(|node| node["name"].as_str().unwrap())
        .collect()
}

#[test]
fn a_named_copy_moves_to_the_head_and_each_node_added_changed_and_removed_is_reported() {
    let (scratch, _remote) = pinned_at_first(&["shelves:IF-1", "shelves:IF-2"]);
    let stock = fs::read(scratch.file(STOCK)).unwrap();
    let (code, document) = update(&scratch, &["shelves:IF-1"]);
    assert_eq!(code, 0, "{document}");
    assert_eq!(document["outcome"], "updated");
    let [copy] = document["copies"].as_array().unwrap().as_slice() else {
        panic!("one copy: {document}");
    };
    assert_eq!(copy["interface"], "shelves:IF-1");
    assert_eq!(copy["publishSequence"], 2);
    assert_eq!(
        names(&copy["changed"]),
        ["The returned book has the title and the author asked for."]
    );
    assert_eq!(
        names(&copy["added"]),
        ["The returned book is on the shelf."]
    );
    assert_eq!(
        names(&copy["removed"]),
        ["The shelf names the place of the book."]
    );
    for part in ["added", "changed", "removed"] {
        let label = copy[part][0]["label"].as_str().unwrap();
        assert!(label.starts_with("shelves:AC-"), "{label}");
    }
    let moved = fs::read_to_string(scratch.file(COPY)).unwrap();
    assert!(moved.contains("publishSequence: 2\n"), "{moved}");
    assert_eq!(fs::read(scratch.file(STOCK)).unwrap(), stock);
}

#[test]
fn every_copy_or_a_handles_copies_move_and_a_withdrawn_interface_stays_at_its_pin() {
    for args in [&[][..], &["shelves"][..]] {
        let (scratch, _remote) = pinned_at_first(&["shelves:IF-1", "shelves:IF-2"]);
        let stock = fs::read(scratch.file(STOCK)).unwrap();
        let (code, document) = update(&scratch, args);
        assert_eq!(code, 1, "{args:?}: {document}");
        assert_eq!(document["outcome"], "withdrawn", "{args:?}");
        let copies = document["copies"].as_array().unwrap();
        let state = |name: &str| {
            let copy = copies
                .iter()
                .find(|copy| copy["interface"] == name)
                .unwrap();
            (
                copy["state"].clone(),
                copy["publishSequence"].clone(),
                copy["head"].clone(),
            )
        };
        assert_eq!(
            state("shelves:IF-1"),
            ("updated".into(), 2.into(), 2.into())
        );
        assert_eq!(
            state("shelves:IF-2"),
            ("withdrawn".into(), 1.into(), 2.into())
        );
        assert_eq!(fs::read(scratch.file(STOCK)).unwrap(), stock, "{args:?}");
        let moved = fs::read_to_string(scratch.file(COPY)).unwrap();
        assert!(moved.contains("publishSequence: 2\n"), "{args:?}");
    }
}

#[test]
fn a_withdrawn_interface_is_named_with_the_heads_publish_sequence() {
    let (scratch, _remote) = pinned_at_first(&["shelves:IF-2"]);
    let output = interface(&scratch, &["update"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.contains("shelves:IF-2 is withdrawn: publish sequence 2 no longer holds it"),
        "{text}"
    );
}

#[test]
fn a_copy_already_at_the_head_is_reported_current_and_left_as_it_was() {
    let (scratch, _history, _remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let before = fs::read(scratch.file(COPY)).unwrap();
    let (code, document) = update(&scratch, &[]);
    assert_eq!(code, 0, "{document}");
    assert_eq!(document["outcome"], "current");
    assert_eq!(document["copies"][0]["state"], "current");
    assert_eq!(fs::read(scratch.file(COPY)).unwrap(), before);
}

#[test]
fn at_moves_the_one_named_copy_to_that_publish_sequence_and_names_one_copy_only() {
    let (scratch, _history, _remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let (code, document) = update(&scratch, &["shelves:IF-1", "--at", "1"]);
    assert_eq!(code, 0, "{document}");
    assert_eq!(document["copies"][0]["publishSequence"], 1);
    let moved = fs::read_to_string(scratch.file(COPY)).unwrap();
    assert!(moved.contains("publishSequence: 1\n"), "{moved}");
    let (code, document) = update(&scratch, &["shelves", "--at", "2"]);
    assert_eq!(code, 1, "{document}");
    assert_eq!(document["outcome"], "error");
}

#[test]
fn a_copy_the_document_does_not_hold_is_refused() {
    let (scratch, _remote) = pinned_at_first(&["shelves:IF-1"]);
    let (code, document) = update(&scratch, &["shelves:IF-9"]);
    assert_eq!(code, 1, "{document}");
    let message = document["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("loft interface add"), "{message}");
}

/// The tree pinned at the first state with the Shelf store mentioning the criterion the second
/// state drops.
fn mentioning_the_dropped_criterion() -> (Scratch, Remote) {
    let (scratch, remote) = pinned_at_first(&["shelves:IF-1"]);
    let dropped = crate::common::others::DROPPED;
    scratch.edit(
        STORE,
        SCOPE,
        &format!("{SCOPE}mentions:\n  - document: shelves\n    key: {dropped}\n"),
    );
    let formatted = scratch.loft(&["fmt"]);
    assert_eq!(formatted.status.code(), Some(0), "{}", stderr(&formatted));
    (scratch, remote)
}

#[test]
fn a_reference_left_naming_a_node_no_copy_holds_is_named_and_the_copies_are_written() {
    let (scratch, _remote) = mentioning_the_dropped_criterion();
    let (code, document) = update(&scratch, &[]);
    assert_eq!(code, 1, "{document}");
    assert_eq!(document["outcome"], "dangling");
    let [reference] = document["dangling"].as_array().unwrap().as_slice() else {
        panic!("one reference: {document}");
    };
    assert!(reference["element"].as_str().unwrap().starts_with("CMP-"));
    assert_eq!(reference["field"], "mentions");
    let removed = &document["copies"][0]["removed"][0]["label"];
    assert_eq!(&reference["label"], removed);
    let (plain, _plain_remote) = pinned_at_first(&["shelves:IF-1"]);
    let (code, _) = update(&plain, &[]);
    assert_eq!(code, 0);
    assert_eq!(
        fs::read(scratch.file(COPY)).unwrap(),
        fs::read(plain.file(COPY)).unwrap()
    );
}

#[test]
fn an_update_whose_journal_cannot_save_moves_no_copy() {
    let (scratch, _remote) = pinned_at_first(&["shelves:IF-1"]);
    scratch.block_journal();
    let before = crate::common::tree(&scratch.0);
    let (code, document) = update(&scratch, &[]);
    assert_eq!(code, 1, "{document}");
    assert_eq!(document["outcome"], "error");
    assert_eq!(crate::common::tree(&scratch.0), before);
}

#[test]
fn without_a_kept_sign_in_an_update_signs_the_person_in_to_the_other_remote_first() {
    let (scratch, remote) = pinned_at_first(&["shelves:IF-1"]);
    crate::common::signed_in::keep(&scratch, &serde_json::json!({}));
    let args = ["interface", "update"];
    let (output, browser) = crate::common::signed_in::run(
        &scratch,
        &args,
        software_plant::BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(browser.visits().len(), 1);
    let kept = crate::common::signed_in::sign_ins(&scratch);
    assert!(kept[remote.address()]["accessToken"].is_string());
}

#[test]
fn a_copy_at_the_head_that_is_not_the_copy_its_state_gives_is_taken_again() {
    let (scratch, _history, _remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let taken = fs::read(scratch.file(COPY)).unwrap();
    scratch.edit(COPY, "label: IF-1\n", "label: IF-7\n");
    let (code, document) = update(&scratch, &[]);
    assert_eq!(code, 0, "{document}");
    assert_eq!(document["outcome"], "updated");
    assert_eq!(document["copies"][0]["publishSequence"], 2);
    assert_eq!(fs::read(scratch.file(COPY)).unwrap(), taken);
}
