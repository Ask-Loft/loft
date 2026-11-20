//! `loft interface add` refusing: a document that is not connected, a handle `documents` does
//! not list, a `--remote` that is not the listed address, an Interface already copied or not
//! held, a remote that serves the document itself, and a save its journal cannot make. Each
//! exits 1 and writes nothing.

use crate::common::connected::Connected;
use crate::common::others::{listing, shelves, shelves_remote};
use crate::common::remote::history;
use crate::common::{Scratch, stderr, tree};
use crate::interface_add::{conforming, interface, ready};
use software_plant::remote::{Remote, RemoteScenario};

/// Runs `loft interface add` with `args` in `scratch`, expects it refused with nothing written,
/// and gives its error message.
fn refused(scratch: &Scratch, args: &[&str]) -> String {
    let before = tree(&scratch.0);
    let mut all = vec!["add"];
    all.extend(args);
    all.extend(["--format", "json"]);
    let output = interface(scratch, &all);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let result = conforming(&output);
    assert_eq!(result["outcome"], "error");
    assert_eq!(tree(&scratch.0), before, "{args:?}: a file was written");
    result["errors"][0]["message"].as_str().unwrap().to_owned()
}

#[test]
fn an_add_in_a_document_that_is_not_connected_says_that_remote_set_connects_it() {
    let history = shelves();
    let remote = shelves_remote(&history);
    let scratch = Scratch::with_reading_list();
    let address = remote.address();
    let message = refused(&scratch, &["shelves:IF-1", "--remote", &address]);
    assert!(message.contains("loft remote set <address>"), "{message}");
    assert!(remote.requests().is_empty());
}

#[test]
fn an_unlisted_handle_without_remote_says_that_remote_lists_the_other_document() {
    let (scratch, _history, _remote) = ready(false);
    let message = refused(&scratch, &["shelves:IF-1"]);
    assert!(message.contains("--remote <address>"), "{message}");
}

#[test]
fn a_remote_that_differs_from_the_listed_address_is_refused_naming_the_listed_one() {
    let (scratch, _history, remote) = ready(true);
    let message = refused(
        &scratch,
        &[
            "shelves:IF-1",
            "--remote",
            "http://127.0.0.1:9/documents/other",
        ],
    );
    assert!(message.contains(&remote.address()), "{message}");
}

#[test]
fn an_interface_already_copied_says_that_update_moves_its_pin() {
    let (scratch, _history, _remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let message = refused(&scratch, &["shelves:IF-1"]);
    assert!(message.contains("loft interface update"), "{message}");
}

#[test]
fn an_interface_the_state_does_not_hold_is_named_with_its_handle_and_publish_sequence() {
    let (scratch, _history, _remote) = ready(true);
    let message = refused(&scratch, &["shelves:IF-9"]);
    assert!(message.contains("shelves"), "{message}");
    assert!(message.contains("IF-9"), "{message}");
    assert!(message.contains("publish sequence 2"), "{message}");
    let message = refused(&scratch, &["shelves:IF-1", "--at", "7"]);
    assert!(message.contains("publish sequence 7"), "{message}");
}

#[test]
fn a_remote_whose_state_names_the_documents_own_key_is_refused_naming_the_key() {
    let own = history(1);
    let remote = Remote::start(
        RemoteScenario::accepting(&own.key).holding("Reading List", own.states.clone()),
    );
    let scratch = Scratch::with_reading_list();
    listing(&scratch, &remote, true);
    let message = refused(&scratch, &["shelves:IF-1"]);
    assert!(message.contains(&own.key), "{message}");
}

#[test]
fn an_add_whose_journal_cannot_save_writes_neither_the_copy_nor_the_manifest() {
    let (scratch, _history, _remote) = ready(true);
    scratch.block_journal();
    let before = tree(&scratch.0);
    let output = interface(&scratch, &["add", "shelves:IF-1", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let message = Connected::message(&output);
    assert!(message.contains(".loft"), "{message}");
    assert_eq!(tree(&scratch.0), before);
}

#[test]
fn another_documents_address_is_held_to_the_rule_of_a_remotes_address() {
    let (scratch, _history, _remote) = ready(false);
    let message = refused(
        &scratch,
        &[
            "shelves:IF-1",
            "--remote",
            "http://shelves.example/documents/one",
        ],
    );
    assert!(message.contains("loopback"), "{message}");
}

#[test]
fn a_document_another_handle_lists_is_refused_naming_that_handle() {
    let (scratch, _history, remote) = ready(true);
    let address = remote.address();
    let message = refused(&scratch, &["stock:IF-2", "--remote", &address]);
    assert!(message.contains("under shelves"), "{message}");
}

#[test]
fn a_remote_that_holds_no_such_document_is_named_by_its_handle() {
    let (scratch, _history, remote) = ready(false);
    let address = format!("{}/documents/missing", remote.origin());
    crate::common::signed_in::keep(
        &scratch,
        &serde_json::json!({ address.clone(): crate::common::signed_in::granted(&remote, 3600) }),
    );
    let message = refused(&scratch, &["shelves:IF-1", "--remote", &address]);
    assert!(message.contains("shelves: the remote at"), "{message}");
    assert!(message.contains("holds no such document"), "{message}");
    assert!(!message.contains("loft remote set"), "{message}");
}

#[test]
fn a_copy_whose_node_shares_a_key_with_an_element_of_the_document_is_refused() {
    let (scratch, _history, _remote) = ready(true);
    crate::interfaces::add_lookup(&scratch);
    let message = refused(&scratch, &["shelves:IF-1"]);
    assert!(
        message.contains("an element of this document carries"),
        "{message}"
    );
}
