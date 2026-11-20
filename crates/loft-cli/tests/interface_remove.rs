//! `loft interface remove`, which deletes a copy nothing references, and the handle's entry once
//! no copy is left under it, without reaching a remote; and `loft interface login`, a sign-in to
//! the remote that a handle's entry names.

use std::fs;

use crate::common::others::COPY;
use crate::common::signed_in::{self, keep, sign_ins};
use crate::common::{stderr, stdout, tree};
use crate::interface_add::{conforming, interface, ready};
use crate::interfaces::LOOKUP;
use serde_json::json;
use software_plant::BrowserScenario;

const STOCK: &str = "documents/shelves/shelf-stock.yaml";
const STORE: &str = "components/shelf-store.yaml";
const SCOPE: &str = "scopedTo:\n  - 00000000-0000-4000-8000-00000000000a\n";

fn manifest(scratch: &crate::common::Scratch) -> String {
    fs::read_to_string(scratch.file("loftspec.yaml")).unwrap()
}

#[test]
fn a_removed_copy_is_deleted_and_its_entry_kept_while_a_copy_is_left_under_its_handle() {
    let (scratch, _history, remote) = ready(true);
    for name in ["shelves:IF-1", "shelves:IF-2"] {
        let output = interface(&scratch, &["add", name, "--at", "1"]);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    }
    let asked = remote.requests().len();
    let output = interface(&scratch, &["remove", "shelves:IF-2", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document = conforming(&output);
    assert_eq!(document["outcome"], "removed");
    assert_eq!(document["unlisted"], false);
    assert!(!scratch.file(STOCK).exists());
    assert!(manifest(&scratch).contains("handle: shelves"));
    let output = interface(&scratch, &["remove", "shelves:IF-1", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(conforming(&output)["unlisted"], true);
    assert!(!scratch.file(COPY).exists());
    assert!(
        !manifest(&scratch).contains("documents:"),
        "{}",
        manifest(&scratch)
    );
    assert_eq!(remote.requests().len(), asked, "remove reached the remote");
    let check = scratch.loft(&["check"]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));
}

#[test]
fn a_remove_opens_no_connection_even_with_no_remote_listening() {
    let (scratch, _history, remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    drop(remote);
    keep(&scratch, &json!({}));
    let output = interface(&scratch, &["remove", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("removed the copy of shelves:IF-1"), "{text}");
    assert!(
        text.contains("Commit spec/: it no longer holds a copy of shelves:IF-1."),
        "{text}"
    );
}

#[test]
fn a_remove_whose_journal_cannot_save_deletes_no_copy_and_keeps_the_manifest() {
    let (scratch, _history, _remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    // The journal lives under `.loft/`, which a file of that name keeps from being created.
    fs::write(scratch.0.join(".loft"), "not a directory\n").unwrap();
    let before = tree(&scratch.0);
    let output = interface(&scratch, &["remove", "shelves:IF-1", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_eq!(tree(&scratch.0), before);
}

#[test]
fn a_copy_a_reference_names_is_not_removed_and_each_reference_is_named() {
    let (scratch, _history, _remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    scratch.edit(
        STORE,
        SCOPE,
        &format!("{SCOPE}requires:\n  - document: shelves\n    key: {LOOKUP}\n"),
    );
    let formatted = scratch.loft(&["fmt"]);
    assert_eq!(formatted.status.code(), Some(0), "{}", stderr(&formatted));
    let before = tree(&scratch.0);
    let output = interface(&scratch, &["remove", "shelves:IF-1", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let message = conforming(&output)["errors"][0]["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(message.contains("CMP-"), "{message}");
    assert!(message.contains("`requires`"), "{message}");
    assert_eq!(tree(&scratch.0), before);
}

#[test]
fn login_keeps_a_sign_in_under_the_entrys_address_writes_no_tree_file_and_prints_no_token() {
    let (scratch, _history, remote) = ready(true);
    keep(&scratch, &json!({}));
    let before = tree(&scratch.0);
    let args = ["interface", "login", "shelves", "--format", "json"];
    let (output, browser) = signed_in::run(&scratch, &args, BrowserScenario::Visiting, None);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(browser.visits().len(), 1);
    assert_eq!(conforming(&output)["address"], remote.address());
    let kept = sign_ins(&scratch);
    let token = kept[remote.address()]["accessToken"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(!stdout(&output).contains(&token));
    assert!(!stderr(&output).contains(&token));
    assert_eq!(tree(&scratch.0), before);
}

#[test]
fn login_for_a_handle_documents_does_not_list_is_refused() {
    let (scratch, _history, _remote) = ready(true);
    let output = interface(&scratch, &["login", "elsewhere", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let message = conforming(&output)["errors"][0]["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(message.contains("elsewhere"), "{message}");
}
