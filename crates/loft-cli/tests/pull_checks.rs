//! `loft pull`: what it checks in what the remote gives before writing it, how it reports a pull
//! with nothing to write, and a node the tree deleted that the head still holds.

use std::fs;

use serde_json::json;
use software_plant::BrowserScenario;
use software_plant::http::{Answer, Request, Responder, Response, Server};
use software_plant::remote::{RemoteScenario, TOKEN};

use crate::common::connected::{Connected, connected};
use crate::common::remote::{publish_at, spec, stored, wire, with_criterion};
use crate::common::signed_in::{self, document};
use crate::common::tree;
use crate::pull::{BODY, DECISION, PRODUCT, behind, pull, with_body};

#[test]
fn a_head_of_another_document_is_refused_naming_both_keys() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let first = run.history.states[0].clone();
    let other = "00000000-0000-4000-8000-0000000000ff";
    // Another document, published twice: its second state stands as the remote's head.
    let files = spec(&first.files)
        .into_iter()
        .map(|(path, bytes)| {
            let text = String::from_utf8(bytes).unwrap();
            (path, text.replace(&run.history.key, other).into_bytes())
        })
        .collect();
    let (lock, files) = publish_at(&files, None, 1);
    let (lock, files) = publish_at(
        &with_criterion(&files, "01a18e50-0000-7000-8000-000000000002"),
        Some(&lock),
        2,
    );
    let key = run.history.key.clone();
    let run = run.serving(
        RemoteScenario::accepting(&key).holding("Reading List", vec![first, stored(&lock, &files)]),
    );
    let before = tree(&run.scratch.0);
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = Connected::message(&output);
    assert!(
        message.contains(other) && message.contains(&key),
        "{message}"
    );
    assert_eq!(tree(&run.scratch.0), before);
}

#[test]
fn a_state_whose_files_do_not_agree_with_its_lock_is_refused() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let published = crate::common::remote::history(2);
    let mut tampered = published.states[1].clone();
    tampered.files = wire(&with_body(&spec(&tampered.files), "Tampered."));
    let key = run.history.key.clone();
    let run = run.serving(
        RemoteScenario::accepting(&key)
            .holding("Reading List", vec![published.states[0].clone(), tampered]),
    );
    let before = tree(&run.scratch.0);
    for args in [&[][..], &["--sequence", "2"][..]] {
        let output = pull(&run.scratch, args);
        assert_eq!(output.status.code(), Some(1), "{args:?}: {output:?}");
        let message = Connected::message(&output);
        assert!(
            message.contains("publish sequence 2 does not agree with its lock"),
            "{message}"
        );
        assert_eq!(tree(&run.scratch.0), before);
    }
}

#[test]
fn a_pull_with_nothing_to_write_reports_unchanged_naming_what_it_pulled() {
    let run = connected(2, 2, RemoteScenario::accepting);
    let second = run.history.states[1].clone();
    let key = run.history.key.clone();
    let states = run.history.states.clone();
    let run = run.serving(
        RemoteScenario::accepting(&key)
            .holding("Reading List", states)
            .draft("feat/same", second.files.clone()),
    );
    let before = tree(&run.scratch.0);
    let at_sequence = document(&pull(&run.scratch, &["--sequence", "2"]));
    assert_eq!(at_sequence["outcome"], "unchanged");
    assert_eq!(at_sequence["publishSequence"], 2);
    assert_eq!(at_sequence["stateRoot"], second.lock["stateRoot"]);
    let at_draft = document(&pull(&run.scratch, &["--branch", "feat/same"]));
    assert_eq!(at_draft["outcome"], "unchanged");
    assert_eq!(at_draft["branch"], "feat/same");
    assert_eq!(tree(&run.scratch.0), before);
    let (mut command, _) = crate::common::signed_in::command(
        &run.scratch,
        software_plant::BrowserScenario::Absent,
        None,
    );
    let output = command.args(["pull", "--sequence", "2"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("unchanged: the tree already holds publish sequence 2"),
        "{stdout}"
    );
}

#[test]
fn a_node_the_tree_deleted_that_the_head_still_holds_stays_deleted_its_binding_departed() {
    let (run, _) = behind();
    fs::remove_file(run.scratch.file(DECISION)).unwrap();
    assert_eq!(run.scratch.loft(&["fmt"]).status.code(), Some(0));
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(!run.scratch.file(DECISION).exists());
    let labels = fs::read_to_string(run.scratch.file("labels.yaml")).unwrap();
    let binding = &labels[labels.find("00000000-0000-4000-8000-00000000008c").unwrap()..];
    let binding = &binding[..binding.find("  - key:").unwrap_or(binding.len())];
    assert!(binding.contains("departed: true"), "{binding}");
    let product = fs::read_to_string(run.scratch.file(PRODUCT)).unwrap();
    assert!(product.contains(BODY));
    assert_eq!(run.scratch.loft(&["check"]).status.code(), Some(0));
}

/// A remote whose head does not read: its publish sequence is not a number.
struct Garbled;

impl Responder for Garbled {
    fn answer(&mut self, _: &Request) -> Answer {
        Response::json(
            200,
            &json!({ "head": { "publishSequence": "two", "stateRoot": null } }),
        )
        .into()
    }
}

#[test]
fn a_head_that_does_not_read_is_refused_naming_the_remote_s_address() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let garbled = Server::start(Garbled);
    let address = format!("{}/documents/{}", garbled.url(), run.history.key);
    let env = [
        ("LOFT_REMOTE_ENDPOINT", address.as_str()),
        ("LOFT_REMOTE_TOKEN", TOKEN),
    ];
    let refused = |output: &std::process::Output| {
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let message = Connected::message(output);
        assert!(
            message.contains(&format!("{address}: the remote's head does not read")),
            "{message}"
        );
    };
    let before = tree(&run.scratch.0);
    let (mut command, _) = signed_in::command(&run.scratch, BrowserScenario::Absent, None);
    refused(
        &command
            .args(["pull", "--format", "json"])
            .envs(env)
            .output()
            .unwrap(),
    );
    assert_eq!(tree(&run.scratch.0), before);
    // A publish reads the same head, and refuses it the same way.
    run.change();
    let changed = tree(&run.scratch.0);
    refused(&run.publish_with(&env));
    assert_eq!(tree(&run.scratch.0), changed);
}
