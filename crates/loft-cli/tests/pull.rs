//! `loft pull`: the remote's head merged into the tree three ways, a published state or a
//! branch's draft written over a tree at its lock, and every refusal, under the remote
//! simulator.

use std::collections::BTreeMap;
use std::fs;
use std::process::Output;

use loftspec::codec::{Files, SpecPath};
use loftspec::publish::{self, Made, Publication, PublishedAt, ReadTree};
use serde_json::Value;
use software_plant::BrowserScenario;
use software_plant::remote::{RemoteScenario, StoredState};

use crate::common::connected::{Connected, connected, serve};
use crate::common::remote::{History, history, spec, wire};
use crate::common::signed_in::{self, document, lock_bytes, name_remote};
use crate::common::{EPOCH, Scratch, tree};

/// The Product's file, whose body the tests change.
const PRODUCT: &str = "products/reading-list/product.yaml";
/// The Product's body as the Reading List holds it.
const BODY: &str = "Keeps the books a reader means to read.";
/// A decision file a draft may leave out.
const DECISION: &str = "decisions/keep-a-change-log.yaml";

/// Runs `loft pull` with `args` over `scratch`, as JSON.
fn pull(scratch: &Scratch, args: &[&str]) -> Output {
    let (mut command, _) = signed_in::command(scratch, BrowserScenario::Absent, None);
    command
        .arg("pull")
        .args(args)
        .args(["--format", "json"])
        .output()
        .unwrap()
}

/// `files` with the Product's body replaced by `body`.
fn with_body(files: &Files, body: &str) -> Files {
    let mut files = files.clone();
    let path = SpecPath::new(PRODUCT);
    let text = String::from_utf8(files[&path].clone()).unwrap();
    assert!(text.contains(BODY));
    files.insert(path, text.replacen(BODY, body, 1).into_bytes());
    files
}

/// The state published from `files` after `previous`, as a remote holds it.
fn published_after(previous: &StoredState, files: &Files) -> StoredState {
    let lock = loftspec::publish::Lock::read(previous.lock.to_string().as_bytes()).unwrap();
    let at = PublishedAt::from_unix_seconds(EPOCH + 3600).unwrap();
    let tree = ReadTree::read(files).unwrap();
    let Publication::Published {
        lock,
        files: written,
        ..
    } = publish::publish(tree, None, Some(&lock), at, Made::NotAsRemote).unwrap()
    else {
        panic!("the change publishes");
    };
    let mut files = files.clone();
    files.extend(written);
    StoredState {
        lock: serde_json::from_slice(&lock.to_bytes()).unwrap(),
        files: wire(&files),
    }
}

/// A Feature's file, whose body the conflict test changes too, and that body.
const FEATURE: &str = "products/reading-list/shelf/add-a-book.yaml";
const FEATURE_BODY: &str = "Books are stored by CMP-1, as ADR-1 decided; F-2 removes them.";

/// `files` with the Feature's body replaced by `body`.
fn with_feature_body(files: &Files, body: &str) -> Files {
    let mut files = files.clone();
    let path = SpecPath::new(FEATURE);
    let text = String::from_utf8(files[&path].clone()).unwrap();
    assert!(text.contains(FEATURE_BODY));
    files.insert(path, text.replacen(FEATURE_BODY, body, 1).into_bytes());
    files
}

/// The tree's Product body is `body`.
fn set_body(scratch: &Scratch, body: &str) {
    scratch.edit(PRODUCT, BODY, body);
}

/// The paths of the remote's requests, in order.
fn calls(run: &Connected) -> Vec<String> {
    run.remote
        .requests()
        .into_iter()
        .filter(|request| request.path.starts_with("/documents/"))
        .map(|request| {
            request
                .path
                .rsplit_once(&run.history.key)
                .unwrap()
                .1
                .to_owned()
        })
        .collect()
}

/// A repository at state 2 of the Reading List, connected to a remote whose head is state 3.
fn behind() -> (Connected, History) {
    let run = connected(2, 2, RemoteScenario::accepting);
    let ahead = history(3);
    let run = run.serving(
        RemoteScenario::accepting(&ahead.key).holding("Reading List", ahead.states.clone()),
    );
    (run, ahead)
}

#[test]
fn a_pull_merges_the_head_three_ways_and_takes_its_labels_and_lock() {
    let (run, ahead) = behind();
    set_body(&run.scratch, "Keeps the books a reader means to read next.");
    // Our bindings lose one: the head's bindings replace them all the same.
    let labels = run.scratch.file("labels.yaml");
    let text = fs::read_to_string(&labels).unwrap();
    fs::write(&labels, &text[..text.rfind("  - key:").unwrap()]).unwrap();
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result = document(&output);
    assert_eq!(result["outcome"], "pulled");
    let head = &ahead.states[2];
    assert_eq!(result["publishSequence"], 3);
    assert_eq!(result["stateRoot"], head.lock["stateRoot"]);
    assert_eq!(calls(&run), ["/head", "/states/2", "/states/3"]);
    let files = tree(&run.scratch.0);
    let product = String::from_utf8(files[&SpecPath::new(PRODUCT)].clone()).unwrap();
    assert!(product.contains("means to read next."), "ours is kept");
    let theirs = spec(&head.files);
    for (path, bytes) in &theirs {
        if path.as_str() != PRODUCT && path.as_str() != "loftspec.yaml" {
            assert_eq!(&files[path], bytes, "{path} is the head's");
        }
    }
    assert_eq!(run.lock(), head.lock, "the lock is the head's");
    let manifest = fs::read_to_string(run.scratch.file("loftspec.yaml")).unwrap();
    assert!(manifest.contains(&format!("remote: \"{}\"", run.remote.address())));
    assert!(manifest.contains("connected: true"));
    assert_eq!(run.scratch.loft(&["fmt", "--check"]).status.code(), Some(0));
}

#[test]
fn a_pull_at_the_head_writes_nothing_and_fetches_no_state() {
    let run = connected(2, 2, RemoteScenario::accepting);
    let before = tree(&run.scratch.0);
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(document(&output)["outcome"], "unchanged");
    assert_eq!(calls(&run), ["/head"]);
    assert_eq!(tree(&run.scratch.0), before);
}

#[test]
fn a_document_without_a_lock_is_merged_from_an_empty_base() {
    let published = history(1);
    let scratch = Scratch::with_reading_list();
    let remote = serve(
        &scratch,
        RemoteScenario::accepting(&published.key).holding("Reading List", published.states.clone()),
    );
    let output = pull(&scratch, &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let paths: Vec<String> = remote.requests().into_iter().map(|r| r.path).collect();
    assert!(
        paths.iter().all(|path| !path.ends_with("/states/0")),
        "{paths:?}"
    );
    let lock: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("loft.lock.json")).unwrap()).unwrap();
    assert_eq!(lock, published.states[0].lock);
}

#[test]
fn a_conflict_is_written_reported_in_document_order_and_exits_one() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let first = run.history.states[0].clone();
    let theirs = published_after(
        &first,
        &with_feature_body(&with_body(&spec(&first.files), "Theirs."), "Theirs too."),
    );
    let key = run.history.key.clone();
    let run = run.serving(
        RemoteScenario::accepting(&key)
            .holding("Reading List", vec![first.clone(), theirs.clone()]),
    );
    set_body(&run.scratch, "Ours.");
    run.scratch.edit(FEATURE, FEATURE_BODY, "Ours too.");
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let result = document(&output);
    assert_eq!(result["outcome"], "conflicted");
    let conflicts = result["conflicts"].as_array().unwrap();
    assert_ne!(conflicts.len(), 0);
    for conflict in conflicts {
        assert_eq!(
            conflict["id"].as_str().map(str::len),
            Some(64),
            "{conflict}"
        );
        assert!(conflict["holder"].as_str().is_some() && conflict["field"].as_str().is_some());
    }
    // The Product comes before its Feature in the document.
    let holders: Vec<&str> = conflicts
        .iter()
        .filter(|conflict| conflict["field"] == "body")
        .map(|conflict| conflict["holder"].as_str().unwrap())
        .collect();
    assert_eq!(
        holders,
        [
            "00000000-0000-4000-8000-00000000000a",
            "00000000-0000-4000-8000-00000000001e"
        ],
        "{conflicts:?}"
    );
    let product = fs::read_to_string(run.scratch.file(PRODUCT)).unwrap();
    assert!(
        product.contains("Ours.") && product.contains("Theirs."),
        "{product}"
    );
    assert_eq!(run.lock(), theirs.lock, "the lock is written too");
    let (mut command, _) = signed_in::command(&run.scratch, BrowserScenario::Absent, None);
    let check = command
        .args(["check", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(1), "the conflict record stands");
}

#[test]
fn a_sequence_is_written_over_a_tree_at_its_lock() {
    let run = connected(3, 3, RemoteScenario::accepting);
    let output = pull(&run.scratch, &["--sequence", "1"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result = document(&output);
    assert_eq!(result["outcome"], "pulled");
    assert_eq!(result["publishSequence"], 1);
    assert_eq!(calls(&run), ["/head", "/states/1"]);
    let first = run.history.states[0].clone();
    assert_eq!(run.lock(), first.lock);
    let files = tree(&run.scratch.0);
    for (path, bytes) in spec(&first.files) {
        if path.as_str() != "loftspec.yaml" {
            assert_eq!(files[&path], bytes, "{path}");
        }
    }
    let manifest = fs::read_to_string(run.scratch.file("loftspec.yaml")).unwrap();
    assert!(
        manifest.contains("connected: true"),
        "the tree's connection is kept"
    );
}

#[test]
fn a_draft_is_written_over_a_tree_at_its_lock_leaving_the_lock_alone() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let first = run.history.states[0].clone();
    // The draft drops a decision, and `loft fmt` marks its binding departed.
    let mut files = with_body(&spec(&first.files), "A draft's body.");
    files.remove(&SpecPath::new(DECISION));
    let formatted = loftspec::fmt::format(&files).unwrap().changed;
    files.extend(formatted);
    let draft = wire(&files);
    let key = run.history.key.clone();
    let run = run.serving(
        RemoteScenario::accepting(&key)
            .holding("Reading List", vec![first.clone()])
            .draft("feat/x", draft),
    );
    let lock = fs::read(run.scratch.0.join("loft.lock.json")).unwrap();
    let output = pull(&run.scratch, &["--branch", "feat/x"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result = document(&output);
    assert_eq!(result["branch"], "feat/x");
    assert_eq!(calls(&run), ["/head", "/drafts/feat%2Fx"]);
    assert!(
        fs::read_to_string(run.scratch.file(PRODUCT))
            .unwrap()
            .contains("A draft's body.")
    );
    assert!(
        !run.scratch.file(DECISION).exists(),
        "a file the draft does not hold is removed"
    );
    assert_eq!(
        fs::read(run.scratch.0.join("loft.lock.json")).unwrap(),
        lock
    );
}

#[test]
fn an_earlier_state_or_a_draft_is_refused_over_changes_and_writes_nothing() {
    let run = connected(2, 2, RemoteScenario::accepting);
    set_body(&run.scratch, "Changed.");
    let before = tree(&run.scratch.0);
    for args in [["--sequence", "1"], ["--branch", "main"]] {
        let output = pull(&run.scratch, &args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let message = document(&output)["errors"][0]["message"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(message.contains("would overwrite changes"), "{message}");
        assert_eq!(tree(&run.scratch.0), before);
    }
    let unlocked = Scratch::with_reading_list();
    name_remote(&unlocked, Some(&run.remote.address()), true);
    let output = pull(&unlocked, &["--sequence", "1"]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a tree without a lock is refused"
    );
}

#[test]
fn sequence_and_branch_together_are_a_usage_error_naming_both() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let output = pull(&run.scratch, &["--sequence", "1", "--branch", "main"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--sequence") && stderr.contains("--branch"),
        "{stderr}"
    );
    assert!(run.remote.requests().is_empty());
}

#[test]
fn a_document_that_is_not_connected_is_refused_without_a_connection() {
    let run = connected(1, 1, RemoteScenario::accepting);
    name_remote(&run.scratch, Some(&run.remote.address()), false);
    let before = tree(&run.scratch.0);
    let output = pull(&run.scratch, &[]);
    assert_eq!(output.status.code(), Some(1));
    let message = document(&output)["errors"][0]["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        message.contains("only a connected document is pulled"),
        "{message}"
    );
    assert!(run.remote.requests().is_empty());
    assert_eq!(tree(&run.scratch.0), before);
}

#[test]
fn a_sequence_or_draft_the_remote_does_not_hold_is_refused_naming_it() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let before = tree(&run.scratch.0);
    for (args, named) in [
        (["--sequence", "7"], "publish sequence 7"),
        (["--branch", "feat/none"], "`feat/none`"),
    ] {
        let output = pull(&run.scratch, &args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let message = document(&output)["errors"][0]["message"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(message.contains(named), "{message}");
        assert_eq!(tree(&run.scratch.0), before);
    }
}

#[test]
fn a_state_of_another_document_is_refused_naming_both_keys() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let first = run.history.states[0].clone();
    let other = "00000000-0000-4000-8000-0000000000ff";
    let draft: BTreeMap<String, String> = first
        .files
        .iter()
        .map(|(path, text)| (path.clone(), text.replace(&run.history.key, other)))
        .collect();
    let key = run.history.key.clone();
    let run = run.serving(
        RemoteScenario::accepting(&key)
            .holding("Reading List", vec![first.clone()])
            .draft("other", draft),
    );
    let before = tree(&run.scratch.0);
    let output = pull(&run.scratch, &["--branch", "other"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = document(&output)["errors"][0]["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        message.contains(other) && message.contains(&run.history.key),
        "{message}"
    );
    assert_eq!(tree(&run.scratch.0), before);
}

/// A decision the second state of [`with_added_file`] adds in a file of its own.
const ADDED_DECISION: &str = "decisions/shelve-by-genre.yaml";

/// The first state of the Reading List and a second that adds [`ADDED_DECISION`].
fn with_added_file() -> (String, StoredState, StoredState) {
    let published = history(1);
    let first = published.states[0].clone();
    let mut files = spec(&first.files);
    files.insert(
        SpecPath::new(ADDED_DECISION),
        b"key: 01a18e50-0000-7000-8000-000000000001\nslug: shelve-by-genre\ntitle: \"Shelve by \
          genre\"\ncontext: \"Readers browse by genre.\"\nstate:\n  kind: proposed\n"
            .to_vec(),
    );
    let second = published_after(&first, &files);
    (published.key, first, second)
}

/// Writes `state` and its lock into `scratch`.
fn place(scratch: &Scratch, state: &StoredState) {
    fs::remove_dir_all(scratch.0.join("spec")).unwrap();
    for (path, bytes) in spec(&state.files) {
        let target = scratch.file(path.as_str());
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
    fs::write(scratch.0.join("loft.lock.json"), lock_bytes(state)).unwrap();
}

#[test]
fn a_sequence_removes_a_file_its_state_does_not_hold() {
    let (key, first, second) = with_added_file();
    let scratch = Scratch::with_reading_list();
    place(&scratch, &second);
    let _remote = serve(
        &scratch,
        RemoteScenario::accepting(&key).holding("Reading List", vec![first.clone(), second]),
    );
    assert!(scratch.file(ADDED_DECISION).exists());
    let output = pull(&scratch, &["--sequence", "1"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(!scratch.file(ADDED_DECISION).exists());
    let lock: Value =
        serde_json::from_slice(&fs::read(scratch.0.join("loft.lock.json")).unwrap()).unwrap();
    assert_eq!(lock, first.lock);
}

#[test]
fn a_lock_state_the_remote_does_not_hold_is_refused_naming_it() {
    let (key, first, second) = with_added_file();
    // The remote holds the first state alone, or a second state of another root.
    let other = published_after(&first, &with_body(&spec(&first.files), "Another second."));
    let third = published_after(&other, &with_body(&spec(&first.files), "A third."));
    for held in [vec![first.clone()], vec![first.clone(), other, third]] {
        let scratch = Scratch::with_reading_list();
        place(&scratch, &second);
        let _remote = serve(
            &scratch,
            RemoteScenario::accepting(&key).holding("Reading List", held),
        );
        let before = tree(&scratch.0);
        let output = pull(&scratch, &[]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let message = document(&output)["errors"][0]["message"]
            .as_str()
            .unwrap()
            .to_owned();
        let root = second.lock["stateRoot"].as_str().unwrap();
        assert!(
            message.contains("publish sequence 2") && message.contains(root),
            "{message}"
        );
        assert!(!message.contains("loft pull"), "{message}");
        assert_eq!(tree(&scratch.0), before);
    }
}
