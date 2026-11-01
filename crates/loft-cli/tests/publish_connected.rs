//! `loft publish` of a connected document, through its remote: the delta from the base its lock
//! names, the retry key, the answer applied in one save, and the refusals of the publish itself,
//! run under the remote and git simulators.

use std::fs;

use serde_json::{Value, json};
use software_plant::remote::RemoteScenario;
use software_plant::{BrowserScenario, GitScenario};

use crate::common::connected::{ADDED, Connected, connected, paths, serve};
use crate::common::remote::history;
use crate::common::signed_in::{self, document, sign_ins};
use crate::common::{Scratch, tree};

#[test]
fn a_connected_publish_sends_its_delta_and_writes_the_labels_and_lock_the_remote_returns() {
    let run = connected(2, 2, RemoteScenario::accepting);
    run.change();
    let output = run.publish();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result = document(&output);
    assert_eq!(result["outcome"], "published");
    assert_eq!(result["publishSequence"], 3);
    let allocated = result["allocated"].as_array().unwrap();
    assert!(
        allocated.iter().any(|given| given["key"] == ADDED),
        "{allocated:?}"
    );
    let key = &run.history.key;
    assert_eq!(
        run.calls(),
        [
            format!("GET /documents/{key}/head"),
            format!("POST /documents/{key}/publish")
        ]
    );
    let sent = run.sent();
    let base = &run.history.states[1].lock;
    assert_eq!(
        sent["base"],
        json!({ "publishSequence": 2, "stateRoot": base["stateRoot"] })
    );
    let changed = paths(&sent, "changed");
    assert!(
        changed.contains(&"spec/loftspec.yaml".to_owned()),
        "{changed:?}"
    );
    assert_eq!(
        changed.len(),
        2,
        "only the files that differ are sent: {changed:?}"
    );
    assert!(!changed.iter().any(|path| path == "spec/labels.yaml"));
    assert_eq!(paths(&sent, "added"), Vec::<String>::new());
    assert_eq!(paths(&sent, "removed"), Vec::<String>::new());
    assert!(sent["retryKey"].as_str().is_some_and(|key| key.len() == 64));

    let head = run.remote.states().last().unwrap().clone();
    assert_eq!(run.lock(), head.lock, "the returned lock is written");
    let labels = fs::read_to_string(run.scratch.file("labels.yaml")).unwrap();
    assert_eq!(labels, head.files["spec/labels.yaml"]);
    let stdout_text = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout_text
            .contains(&sign_ins(&run.scratch)[run.remote.address()]["accessToken"].to_string())
    );
}

#[test]
fn a_publish_whose_answer_was_lost_is_sent_again_with_its_retry_key_and_replayed() {
    let run = connected(1, 1, RemoteScenario::accepting);
    run.change();
    let before = tree(&run.scratch.0);
    let lock_before = fs::read(run.scratch.0.join("loft.lock.json")).unwrap();
    assert_eq!(run.publish().status.code(), Some(0));
    let first = run.sent()["retryKey"].clone();
    let published = run.lock();
    // The answer is lost: the tree is as it was before the publish.
    fs::remove_dir_all(run.scratch.0.join("spec")).unwrap();
    for (path, bytes) in &before {
        let target = run.scratch.file(path.as_str());
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
    fs::write(run.scratch.0.join("loft.lock.json"), &lock_before).unwrap();
    let output = run.publish();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let requests: Vec<Value> = run
        .publish_requests()
        .into_iter()
        .map(|request| serde_json::from_slice(&request.body).unwrap())
        .collect();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1]["retryKey"], first,
        "the same tree sends the same key"
    );
    assert_eq!(
        run.remote.states().len(),
        2,
        "the replay publishes nothing new"
    );
    assert_eq!(
        run.lock(),
        published,
        "the replay moves the lock past the head it named"
    );
}

#[test]
fn a_tree_at_the_head_its_lock_names_sends_nothing_and_says_so() {
    let run = connected(2, 2, RemoteScenario::accepting);
    let before = tree(&run.scratch.0);
    let output = run.publish();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(document(&output)["outcome"], "unchanged");
    assert_eq!(
        run.calls(),
        [format!("GET /documents/{}/head", run.history.key)]
    );
    assert_eq!(tree(&run.scratch.0), before);
    let (mut command, _) =
        signed_in::command(&run.scratch, BrowserScenario::Absent, Some(&run.git));
    let output = command.arg("publish").output().unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("nothing to publish"));
}

#[test]
fn a_head_past_the_lock_takes_the_base_from_history_and_the_conflict_says_to_pull() {
    let run = connected(2, 2, RemoteScenario::accepting);
    // The remote moved on: it holds a third state the tree does not.
    let moved = history(3);
    let scenario =
        RemoteScenario::accepting(&run.history.key).holding("Reading List", moved.states.clone());
    let run = run.serving(scenario);
    run.change();
    let before = tree(&run.scratch.0);
    let output = run.publish();
    assert_eq!(output.status.code(), Some(1));
    let message = Connected::message(&output);
    let head = moved.states[2].lock["stateRoot"].as_str().unwrap();
    assert!(
        message.contains("publish sequence 3") && message.contains(head),
        "{message}"
    );
    assert!(message.contains("`loft pull`"), "{message}");
    let sent = run.sent();
    assert_eq!(
        paths(&sent, "changed").len(),
        2,
        "the base is the lock's state, from history"
    );
    assert!(!run.calls().iter().any(|call| call.contains("/states/")));
    assert_eq!(tree(&run.scratch.0), before);
}

#[test]
fn without_the_lock_in_history_the_base_is_fetched_from_the_remote() {
    let mut run = connected(2, 2, RemoteScenario::accepting);
    run.git = GitScenario::clean_on("main");
    let moved = history(3);
    let scenario =
        RemoteScenario::accepting(&run.history.key).holding("Reading List", moved.states.clone());
    let run = run.serving(scenario);
    run.change();
    let output = run.publish();
    assert_eq!(output.status.code(), Some(1), "the head moved on");
    let key = &run.history.key;
    assert!(
        run.calls()
            .contains(&format!("GET /documents/{key}/states/2")),
        "{:?}",
        run.calls()
    );
    assert_eq!(paths(&run.sent(), "changed").len(), 2);
}

#[test]
fn a_lock_the_remote_does_not_hold_is_refused_naming_it_and_saying_to_pull() {
    let mut run = connected(2, 1, RemoteScenario::accepting);
    run.git = GitScenario::clean_on("main");
    run.change();
    let before = tree(&run.scratch.0);
    let output = run.publish();
    assert_eq!(output.status.code(), Some(1));
    let message = Connected::message(&output);
    let root = run.history.states[1].lock["stateRoot"].as_str().unwrap();
    assert!(
        message.contains("publish sequence 2") && message.contains(root),
        "{message}"
    );
    assert!(message.contains("`loft pull`"), "{message}");
    assert_eq!(tree(&run.scratch.0), before);
}

#[test]
fn a_remote_without_the_document_is_refused_naming_its_key_and_remote_set() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let scenario = RemoteScenario::accepting(&run.history.key);
    let run = run.serving(scenario);
    let output = run.publish();
    assert_eq!(output.status.code(), Some(1));
    let message = Connected::message(&output);
    assert!(
        message.contains(&run.history.key) && message.contains("loft remote set"),
        "{message}"
    );
}

#[test]
fn a_conflict_at_the_base_is_reported_with_its_detail_and_no_pull() {
    let run = connected(1, 1, |key| {
        RemoteScenario::accepting(key).closed("the document is archived")
    });
    run.change();
    let output = run.publish();
    assert_eq!(output.status.code(), Some(1));
    let message = Connected::message(&output);
    assert!(message.contains("the document is archived"), "{message}");
    assert!(!message.contains("loft pull"), "{message}");
}

#[test]
fn a_document_without_a_lock_sends_every_file_but_its_labels_from_an_empty_base() {
    let scratch = Scratch::with_reading_list();
    assert!(scratch.file("labels.yaml").exists());
    let key = loftspec::codec::read(&tree(&scratch.0))
        .unwrap()
        .id
        .key()
        .to_string();
    let remote = serve(
        &scratch,
        RemoteScenario::accepting(&key).holding("Reading List", Vec::new()),
    );
    let git = GitScenario::clean_on("main");
    let (mut command, _) = signed_in::command(&scratch, BrowserScenario::Absent, Some(&git));
    let output = command
        .args(["publish", "--format", "json"])
        .output()
        .unwrap();
    let request = remote
        .requests()
        .into_iter()
        .find(|request| request.path.ends_with("/publish"))
        .unwrap_or_else(|| panic!("a publish request: {output:?}"));
    let sent: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(
        sent["base"],
        json!({ "publishSequence": 0, "stateRoot": null })
    );
    let added = paths(&sent, "added");
    assert_eq!(added.len(), tree(&scratch.0).len() - 1, "{added:?}");
    assert!(!added.contains(&"spec/labels.yaml".to_owned()));
    assert!(added.iter().all(|path| path.starts_with("spec/")));
}

#[test]
fn a_change_the_remote_answers_as_unchanged_writes_nothing_and_its_replay_says_so_too() {
    let run = connected(1, 1, RemoteScenario::accepting);
    // The tree loses one binding: its state root differs from the head's, but no file the
    // delta carries changes, so the remote has nothing to publish.
    let labels = run.scratch.file("labels.yaml");
    let text = fs::read_to_string(&labels).unwrap();
    let last = text.rfind("  - key:").unwrap();
    fs::write(&labels, &text[..last]).unwrap();
    let before = tree(&run.scratch.0);
    let lock = fs::read(run.scratch.0.join("loft.lock.json")).unwrap();
    for attempt in 0..2 {
        let output = run.publish();
        assert_eq!(output.status.code(), Some(0), "{attempt}: {output:?}");
        let result = document(&output);
        assert_eq!(result["outcome"], "unchanged", "{attempt}");
        assert_eq!(result["allocated"], json!([]), "{attempt}");
        assert_eq!(tree(&run.scratch.0), before, "{attempt}");
        assert_eq!(
            fs::read(run.scratch.0.join("loft.lock.json")).unwrap(),
            lock
        );
    }
    // Only the manifest, which now names the remote, differs, and `remote` enters no root.
    let sent = run.sent();
    assert_eq!(paths(&sent, "changed"), ["spec/loftspec.yaml"]);
    for name in ["added", "removed"] {
        assert_eq!(paths(&sent, name), Vec::<String>::new(), "{name}");
    }
    assert_eq!(
        run.publish_requests().len(),
        2,
        "the second is answered as a replay"
    );
    let (mut command, _) =
        signed_in::command(&run.scratch, BrowserScenario::Absent, Some(&run.git));
    let output = command.arg("publish").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nothing to publish"), "{stdout}");
}
