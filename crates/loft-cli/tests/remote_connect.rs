//! `loft remote set <address>`: the connect of a document to its remote, its published history
//! read from the repository and imported, under the remote, browser and git simulators.

use serde_json::{Value, json};
use software_plant::remote::{Remote, RemoteScenario};
use software_plant::{BrowserScenario, GitScenario};

use crate::common::remote::{History, history};
use crate::common::signed_in::{
    document, granted, keep, lock_bytes, name_remote, repository, run, sign_ins,
};
use crate::common::{Scratch, tree};

/// The name the converted Reading List's first Product carries.
const PRODUCT_NAME: &str = "Reading List";

/// A repository that published `history`, every publish commit in its history.
fn published(history: &History) -> (Scratch, GitScenario) {
    let scratch = Scratch::with_reading_list();
    let sequences: Vec<usize> = (1..=history.states.len()).collect();
    let git = repository(&scratch, history, &sequences);
    (scratch, git)
}

/// Runs `loft remote set <remote's address> --format json` over `scratch`.
fn set(
    scratch: &Scratch,
    remote: &Remote,
    git: Option<&GitScenario>,
) -> (std::process::Output, Value) {
    let address = remote.address();
    let (output, _) = run(
        scratch,
        &["remote", "set", &address, "--format", "json"],
        BrowserScenario::Visiting,
        git,
    );
    let result = document(&output);
    (output, result)
}

/// The paths of the remote's requests that a sign-in does not make.
fn document_calls(remote: &Remote) -> Vec<String> {
    remote
        .requests()
        .into_iter()
        .filter(|request| request.header("authorization").is_some())
        .map(|request| format!("{} {}", request.method, request.path))
        .collect()
}

/// The lock's head as the output names it.
fn head_of(history: &History) -> Value {
    let last = &history.states.last().unwrap().lock;
    json!({ "publishSequence": last["publishSequence"], "stateRoot": last["stateRoot"] })
}

#[test]
fn set_creates_the_document_and_imports_every_published_state_before_naming_the_remote() {
    let history = history(3);
    let (scratch, git) = published(&history);
    let remote = Remote::start(RemoteScenario::accepting(&history.key));
    let (output, result) = set(&scratch, &remote, Some(&git));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(result["outcome"], "connected");
    assert_eq!(result["address"], remote.address());
    assert_eq!(result["documentKey"], history.key);
    assert_eq!(result["head"], head_of(&history));
    let key = &history.key;
    assert_eq!(
        document_calls(&remote),
        [
            format!("POST /documents/{key}/create"),
            format!("POST /documents/{key}/import"),
        ]
    );
    assert_eq!(remote.name().as_deref(), Some(PRODUCT_NAME));
    let held = remote.states();
    assert_eq!(held.len(), 3);
    for (held, published) in held.iter().zip(&history.states) {
        assert_eq!(held.lock, published.lock);
        assert_eq!(held.files, published.files);
    }
    let manifest = std::fs::read_to_string(scratch.file("loftspec.yaml")).unwrap();
    assert!(manifest.contains("connected: true"), "{manifest}");
    assert!(
        manifest.contains(&format!("remote: \"{}\"", remote.address())),
        "{manifest}"
    );
    let (output, _) = run(
        &scratch,
        &["remote", "set", &remote.address()],
        BrowserScenario::Absent,
        Some(&git),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Commit spec/loftspec.yaml: it now names"),
        "{stdout}"
    );
}

#[test]
fn a_connect_says_to_commit_the_manifest_and_holds_no_token() {
    let history = history(1);
    let (scratch, git) = published(&history);
    let remote = Remote::start(RemoteScenario::accepting(&history.key));
    let (output, _) = run(
        &scratch,
        &["remote", "set", &remote.address()],
        BrowserScenario::Visiting,
        Some(&git),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Commit spec/loftspec.yaml: it now names the document's remote."),
        "{stdout}"
    );
    let sign_in = &sign_ins(&scratch)[remote.address()];
    let printed = format!("{stdout}{}", String::from_utf8_lossy(&output.stderr));
    for token in ["accessToken", "refreshToken"] {
        assert!(
            !printed.contains(sign_in[token].as_str().unwrap()),
            "{token}"
        );
    }
}

#[test]
fn a_document_never_published_is_created_and_nothing_is_imported() {
    let scratch = Scratch::with_reading_list();
    let key = codec_key(&scratch);
    let remote = Remote::start(RemoteScenario::accepting(&key));
    let (output, result) = set(&scratch, &remote, None);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        result["head"],
        json!({ "publishSequence": 0, "stateRoot": null })
    );
    assert_eq!(
        document_calls(&remote),
        [format!("POST /documents/{key}/create")]
    );
    assert_eq!(remote.states().len(), 0);
}

#[test]
fn a_remote_that_holds_the_lock_s_head_already_is_connected_without_an_import() {
    let history = history(2);
    let (scratch, git) = published(&history);
    let scenario = RemoteScenario::accepting(&history.key).holding("Held", history.states.clone());
    let remote = Remote::start(scenario);
    let (output, result) = set(&scratch, &remote, Some(&git));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(result["outcome"], "connected");
    assert_eq!(result["head"], head_of(&history));
    assert_eq!(
        document_calls(&remote),
        [format!("POST /documents/{}/create", history.key)]
    );
}

#[test]
fn a_remote_that_holds_the_document_empty_is_imported_into() {
    let history = history(2);
    let (scratch, git) = published(&history);
    let remote = Remote::start(RemoteScenario::accepting(&history.key).holding("Held", Vec::new()));
    let (output, result) = set(&scratch, &remote, Some(&git));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(result["head"], head_of(&history));
    assert_eq!(remote.states().len(), 2);
}

#[test]
fn a_remote_at_another_head_is_refused_naming_both_heads_and_nothing_is_written() {
    let history = history(3);
    let (scratch, git) = published(&history);
    let before = tree(&scratch.0);
    let scenario =
        RemoteScenario::accepting(&history.key).holding("Held", history.states[..2].to_vec());
    let remote = Remote::start(scenario);
    let (output, result) = set(&scratch, &remote, Some(&git));
    assert_eq!(output.status.code(), Some(1));
    let message = result["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("publish sequence 2") && message.contains("publish sequence 3"),
        "{message}"
    );
    assert!(message.contains(history.states[1].lock["stateRoot"].as_str().unwrap()));
    assert!(
        !document_calls(&remote)
            .iter()
            .any(|call| call.ends_with("/import"))
    );
    assert_eq!(tree(&scratch.0), before);
}

#[test]
fn a_refused_create_or_import_names_the_call_its_status_and_code_and_writes_nothing() {
    let history = history(1);
    for (scenario, call, again) in [
        (
            RemoteScenario::accepting(&history.key).forbidding_create(),
            "create",
            false,
        ),
        (
            RemoteScenario::accepting(&history.key).forbidding_import(),
            "import",
            true,
        ),
    ] {
        let (scratch, git) = published(&history);
        let before = tree(&scratch.0);
        let remote = Remote::start(scenario);
        let (output, result) = set(&scratch, &remote, Some(&git));
        assert_eq!(output.status.code(), Some(1), "{call}");
        assert_eq!(result["outcome"], "error");
        let messages: Vec<&str> = result["errors"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|error| error["message"].as_str())
            .collect();
        let first = messages[0];
        assert!(
            first.contains(&format!("`{call}`"))
                && first.contains("403")
                && first.contains("forbidden"),
            "{first}"
        );
        assert_eq!(
            messages
                .iter()
                .any(|m| m.contains("running `loft remote set` again imports into it")),
            again,
            "{call}"
        );
        assert_eq!(tree(&scratch.0), before, "{call}");
    }
}

#[test]
fn a_history_missing_a_publish_is_refused_before_any_request() {
    let history = history(2);
    let scratch = Scratch::with_reading_list();
    let git = repository(&scratch, &history, &[2]);
    let remote = Remote::start(RemoteScenario::accepting(&history.key));
    let (output, result) = set(&scratch, &remote, Some(&git));
    assert_eq!(output.status.code(), Some(1));
    let message = result["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("publish sequence 1"), "{message}");
    assert!(remote.requests().is_empty(), "no request is sent");
}

#[test]
fn a_state_whose_files_do_not_verify_is_refused_naming_its_sequence() {
    let mut history = history(2);
    let (path, text) = history.states[0]
        .files
        .iter()
        .find(|(path, _)| path.contains("products/"))
        .map(|(path, text)| (path.clone(), text.clone()))
        .unwrap();
    history.states[0]
        .files
        .insert(path, text.replacen("Keeps", "Holds", 1));
    let (scratch, git) = published(&history);
    let remote = Remote::start(RemoteScenario::accepting(&history.key));
    let (output, result) = set(&scratch, &remote, Some(&git));
    assert_eq!(output.status.code(), Some(1));
    let message = result["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("publish sequence 1"), "{message}");
    assert!(remote.requests().is_empty());
}

#[test]
fn a_published_document_outside_a_repository_is_refused_before_any_request() {
    let history = history(1);
    let (scratch, _) = published(&history);
    let remote = Remote::start(RemoteScenario::accepting(&history.key));
    let (output, _) = set(&scratch, &remote, None);
    assert_eq!(output.status.code(), Some(1));
    assert!(remote.requests().is_empty());
}

#[test]
fn set_for_a_connected_document_names_the_address_and_sends_only_a_sign_in_s_requests() {
    let history = history(1);
    let (scratch, git) = published(&history);
    let remote = Remote::start(RemoteScenario::accepting(&history.key));
    name_remote(&scratch, Some("https://old.example/documents/one"), true);
    keep(
        &scratch,
        &json!({ remote.address(): granted(&remote, 3600) }),
    );
    let (output, result) = set(&scratch, &remote, Some(&git));
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(result["outcome"], "set");
    assert!(
        remote.requests().is_empty(),
        "the kept sign-in is used as it is"
    );
    let manifest = std::fs::read_to_string(scratch.file("loftspec.yaml")).unwrap();
    assert!(
        manifest.contains(&format!("remote: \"{}\"", remote.address())),
        "{manifest}"
    );
    assert!(!manifest.contains("old.example"));
    assert_eq!(
        std::fs::read(scratch.0.join("loft.lock.json")).unwrap(),
        lock_bytes(&history.states[0])
    );
}

#[test]
fn a_kept_sign_in_that_expires_soon_is_refreshed_and_one_that_lapsed_is_made_anew() {
    for (scenario, refreshed, visited) in [
        (RemoteScenario::accepting("doc-refresh"), 1, 0),
        (RemoteScenario::accepting("doc-lapsed").lapsing(), 0, 1),
    ] {
        let remote = Remote::start(scenario);
        let scratch = Scratch::with_reading_list();
        name_remote(&scratch, None, true);
        let kept = granted(&remote, 60);
        keep(&scratch, &json!({ remote.address(): kept }));
        let (output, browser) = run(
            &scratch,
            &["remote", "set", &remote.address()],
            BrowserScenario::Visiting,
            None,
        );
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert_eq!(remote.refreshes(), refreshed);
        assert_eq!(browser.visits().len(), visited);
        let fresh = &sign_ins(&scratch)[remote.address()];
        assert_ne!(fresh["refreshToken"], kept["refreshToken"]);
    }
}

#[test]
fn an_address_that_may_not_be_contacted_is_refused_before_anything() {
    let scratch = Scratch::with_reading_list();
    let before = tree(&scratch.0);
    keep(
        &scratch,
        &json!({ "https://other.example/doc": { "accessToken": "a" } }),
    );
    let file = std::fs::read(crate::common::signed_in::sign_ins_file(&scratch)).unwrap();
    let (output, browser) = run(
        &scratch,
        &[
            "remote",
            "set",
            "http://remote.example/documents/one",
            "--format",
            "json",
        ],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(1));
    let message = document(&output)["errors"][0]["message"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        message.contains("http://remote.example/documents/one"),
        "{message}"
    );
    assert_eq!(browser.visits(), Vec::<String>::new());
    assert_eq!(tree(&scratch.0), before);
    assert_eq!(
        std::fs::read(crate::common::signed_in::sign_ins_file(&scratch)).unwrap(),
        file
    );
}

/// The document key of the tree under `scratch`.
fn codec_key(scratch: &Scratch) -> String {
    loftspec::codec::read(&tree(&scratch.0))
        .unwrap()
        .id
        .key()
        .to_string()
}
