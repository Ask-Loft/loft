//! `loft interface status`: each copy compared with the state its pin names, confirmed or a
//! mismatch naming its first difference, and with its remote's head, current, behind or
//! withdrawn. It writes nothing, and only a mismatch exits 1.

use std::fs;

use serde_json::Value;
use software_plant::remote::Remote;

use crate::common::others::COPY;
use crate::common::{Scratch, stderr, stdout, tree};
use crate::interface_add::{conforming, interface, ready};

/// A tree holding a copy of each Interface `named`, as `loft interface add` takes it with the
/// further arguments `args`.
fn holding(named: &[&str], args: &[&str]) -> (Scratch, Remote) {
    let (scratch, _history, remote) = ready(true);
    for name in named {
        let mut all = vec!["add", name];
        all.extend(args);
        let output = interface(&scratch, &all);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    }
    (scratch, remote)
}

/// Runs `loft interface status`, which must write no file, and gives its exit code and JSON.
fn status(scratch: &Scratch) -> (i32, Value) {
    let before = tree(&scratch.0);
    let output = interface(scratch, &["status", "--format", "json"]);
    assert_eq!(tree(&scratch.0), before, "status wrote a file");
    assert!(!scratch.0.join(".loft/save").exists());
    (output.status.code().unwrap(), conforming(&output))
}

/// The copy of `name` in a status report.
fn copy<'a>(document: &'a Value, name: &str) -> &'a Value {
    (document["copies"].as_array().unwrap().iter())
        .find(|copy| copy["interface"] == name)
        .unwrap_or_else(|| panic!("no {name} in {document}"))
}

#[test]
fn a_copy_equal_to_its_pinned_state_at_the_head_is_confirmed_and_current() {
    let (scratch, _remote) = holding(&["shelves:IF-1"], &[]);
    let (code, document) = status(&scratch);
    assert_eq!(code, 0, "{document}");
    assert_eq!(document["outcome"], "confirmed");
    let lookup = copy(&document, "shelves:IF-1");
    assert_eq!(lookup["pin"], "confirmed");
    assert_eq!(lookup["freshness"], "current");
    assert_eq!(lookup["publishSequence"], 2);
    assert_eq!(lookup["head"], 2);
}

#[test]
fn a_copy_whose_interface_the_head_changed_is_behind_and_the_command_exits_0() {
    let (scratch, _remote) = holding(&["shelves:IF-1", "shelves:IF-2"], &["--at", "1"]);
    let (code, document) = status(&scratch);
    assert_eq!(code, 0, "{document}");
    let lookup = copy(&document, "shelves:IF-1");
    assert_eq!(lookup["pin"], "confirmed");
    assert_eq!(lookup["freshness"], "behind");
    assert_eq!(lookup["publishSequence"], 1);
    assert_eq!(lookup["head"], 2);
    assert_eq!(copy(&document, "shelves:IF-2")["freshness"], "withdrawn");
}

#[test]
fn each_copy_is_reported_by_its_handle_and_label_with_its_publish_sequence_and_the_heads() {
    let (scratch, _remote) = holding(&["shelves:IF-1"], &["--at", "1"]);
    let output = interface(&scratch, &["status"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.contains(
            "shelves:IF-1 at publish sequence 1, confirmed; behind at the head, publish sequence 2"
        ),
        "{text}"
    );
}

#[test]
fn a_copy_that_differs_from_its_pinned_state_is_a_mismatch_naming_the_first_difference() {
    let (scratch, _remote) = holding(&["shelves:IF-1"], &[]);
    let text = fs::read_to_string(scratch.file(COPY)).unwrap();
    let root = (text.lines())
        .find_map(|line| line.strip_prefix("stateRoot: "))
        .unwrap()
        .trim_matches('"')
        .to_owned();
    let forged = "ab".repeat(32);
    fs::write(scratch.file(COPY), text.replace(&root, &forged)).unwrap();
    let (code, document) = status(&scratch);
    assert_eq!(code, 1, "{document}");
    assert_eq!(document["outcome"], "mismatch");
    let lookup = copy(&document, "shelves:IF-1");
    assert_eq!(lookup["pin"], "mismatch");
    let difference = lookup["difference"].as_str().unwrap();
    assert!(difference.contains("state root"), "{difference}");
    assert!(difference.contains(&forged), "{difference}");
}

#[test]
fn a_copy_whose_labels_differ_from_its_pinned_state_is_a_mismatch_naming_the_label() {
    let (scratch, _remote) = holding(&["shelves:IF-1"], &[]);
    scratch.edit(COPY, "label: IF-1\n", "label: IF-7\n");
    let (code, document) = status(&scratch);
    assert_eq!(code, 1, "{document}");
    let difference = copy(&document, "shelves:IF-7")["difference"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        difference.contains("IF-7") && difference.contains("IF-1"),
        "{difference}"
    );
}

#[test]
fn status_without_a_kept_sign_in_says_that_login_makes_one_and_opens_no_browser() {
    let (scratch, _remote) = holding(&["shelves:IF-1"], &[]);
    crate::common::signed_in::keep(&scratch, &serde_json::json!({}));
    let before = tree(&scratch.0);
    let args = ["interface", "status", "--format", "json"];
    let (output, browser) = crate::common::signed_in::run(
        &scratch,
        &args,
        software_plant::BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let message = conforming(&output)["errors"][0]["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        message.contains("loft interface login shelves"),
        "{message}"
    );
    assert_eq!(browser.visits(), Vec::<String>::new());
    assert_eq!(tree(&scratch.0), before);
}
