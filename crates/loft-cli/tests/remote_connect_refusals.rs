//! `loft remote set` refusing: a manifest its journal cannot save, a name the remote refuses, an
//! import answered with another head, and what the command never takes: a token as an argument
//! or on standard input. Each runs under the remote, browser and git simulators.

use std::fs;
use std::io::Write as _;
use std::process::Stdio;
use std::time::{Duration, Instant};

use loftspec::codec;
use serde_json::json;
use software_plant::BrowserScenario;
use software_plant::remote::{Remote, RemoteScenario};

use crate::common::remote::history;
use crate::common::signed_in::{self, granted, keep, name_remote};
use crate::common::tree;
use crate::remote_connect::{head_of, published, set};

#[test]
fn a_set_whose_manifest_its_journal_cannot_save_changes_no_file_of_the_tree() {
    let history = history(1);
    for connected in [false, true] {
        let (scratch, git) = published(&history);
        let remote = Remote::start(RemoteScenario::accepting(&history.key));
        if connected {
            name_remote(&scratch, Some("https://old.example/documents/one"), true);
            keep(
                &scratch,
                &json!({ remote.address(): granted(&remote, 3600) }),
            );
        }
        // The journal lives under `.loft/`, which a file of that name keeps from being created.
        fs::write(scratch.0.join(".loft"), "not a directory\n").unwrap();
        let before = tree(&scratch.0);
        let (output, result) = set(&scratch, &remote, Some(&git));
        assert_eq!(output.status.code(), Some(1), "{connected}: {output:?}");
        let message = result["errors"][0]["message"].as_str().unwrap();
        assert!(message.contains(".loft"), "{connected}: {message}");
        assert_eq!(tree(&scratch.0), before, "{connected}");
    }
}

#[test]
fn a_create_refused_for_its_name_is_reported_with_the_remote_s_detail() {
    let history = history(1);
    let (scratch, git) = published(&history);
    let long = "R".repeat(201);
    scratch.edit(
        "products/reading-list/product.yaml",
        "name: \"Reading List\"",
        &format!("name: \"{long}\""),
    );
    let remote = Remote::start(RemoteScenario::accepting(&history.key));
    let (output, result) = set(&scratch, &remote, Some(&git));
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = result["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("`create`")
            && message.contains("invalid_request")
            && message.contains("`name` is blank or longer than 200 characters"),
        "{message}"
    );
}

#[test]
fn an_import_answered_with_another_head_is_refused_naming_both_heads_and_writes_nothing() {
    let history = history(2);
    let (scratch, git) = published(&history);
    let before = tree(&scratch.0);
    let remote =
        Remote::start(RemoteScenario::accepting(&history.key).misstating_its_import_head());
    let (output, result) = set(&scratch, &remote, Some(&git));
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = result["errors"][0]["message"].as_str().unwrap();
    let lock = head_of(&history);
    let root = lock["stateRoot"].as_str().unwrap();
    let last = if root.ends_with('0') { '1' } else { '0' };
    let answered = format!("{}{last}", &root[..root.len() - 1]);
    assert!(
        message.contains(root) && message.contains(&answered),
        "{message}"
    );
    assert_eq!(
        message.matches("publish sequence 2").count(),
        2,
        "{message}"
    );
    assert_eq!(tree(&scratch.0), before);
}

#[test]
fn set_takes_no_token_as_an_argument_and_never_reads_standard_input() {
    let history = history(1);
    let (scratch, git) = published(&history);
    let remote = Remote::start(RemoteScenario::accepting(&history.key));
    let (mut command, _) = signed_in::command(&scratch, BrowserScenario::Visiting, Some(&git));
    let output = command
        .args(["remote", "set", &remote.address(), "--token", "a-token"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");

    // Standard input stays open and holds a token: a command that read it would wait for its end.
    let (mut command, _) = signed_in::command(&scratch, BrowserScenario::Visiting, Some(&git));
    let mut child = command
        .args(["remote", "set", &remote.address()])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"a-token\n").unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            panic!("`loft remote set` waited on standard input");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    drop(stdin);
    assert_eq!(status.code(), Some(0));
    assert!(codec::read(&tree(&scratch.0)).unwrap().connected);
}
