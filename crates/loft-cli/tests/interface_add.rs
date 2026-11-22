//! `loft interface add`: a copy of another document's Interface, taken from that document's
//! remote at its head or at the publish sequence `--at` names, with the person's sign-in to that
//! remote and never the tree's own remote's variables.

use std::fs;
use std::process::Output;

use serde_json::{Value, json};
use software_plant::BrowserScenario;
use software_plant::remote::{Remote, RemoteScenario, TOKEN, document_path};

use crate::common::copies::{HANDLE, OTHER};
use crate::common::others::{COPY, listing, shelves, shelves_remote};
use crate::common::remote::History;
use crate::common::signed_in::{self, keep, sign_ins};
use crate::common::{Scratch, stderr, stdout, tree};
use crate::interfaces::LOOKUP;
use crate::schema;

/// A tree connected to its own remote, listing the other document where `listed`, with a
/// sign-in to the other document's remote, which holds two states of it.
pub(crate) fn ready(listed: bool) -> (Scratch, History, Remote) {
    let history = shelves();
    let remote = shelves_remote(&history);
    let scratch = Scratch::with_reading_list();
    listing(&scratch, &remote, listed);
    (scratch, history, remote)
}

/// Runs `loft interface` with `args` and no browser.
pub(crate) fn interface(scratch: &Scratch, args: &[&str]) -> Output {
    let args: Vec<&str> = ["interface"].iter().chain(args).copied().collect();
    signed_in::run(scratch, &args, BrowserScenario::Absent, None).0
}

/// The JSON document a run printed, held to the command's schema.
pub(crate) fn conforming(output: &Output) -> Value {
    schema::conforming(
        &output.stdout,
        include_str!("../../../schemas/interface.json"),
    )
}

fn copy(scratch: &Scratch) -> String {
    fs::read_to_string(scratch.file(COPY)).unwrap()
}

#[test]
fn an_interface_named_by_its_label_is_copied_from_the_state_at_the_head() {
    let (scratch, history, _remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document = conforming(&output);
    assert_eq!(document["outcome"], "added");
    assert_eq!(document["interface"], "shelves:IF-1");
    assert_eq!(document["key"], LOOKUP);
    assert_eq!(document["publishSequence"], 2);
    assert_eq!(document["path"], format!("spec/{COPY}"));
    let copy = copy(&scratch);
    let head = &history.states[1].lock;
    assert!(copy.contains("publishSequence: 2\n"), "{copy}");
    let root = head["stateRoot"].as_str().unwrap();
    assert!(
        copy.contains(&format!("stateRoot: \"{root}\"\n")) || copy.contains(root),
        "{copy}"
    );
    assert!(copy.contains("interfaceHash: "), "{copy}");
    assert!(copy.contains("label: IF-1\n"), "{copy}");
    assert!(
        copy.contains("the title and the author asked for"),
        "{copy}"
    );
    let check = scratch.loft(&["check"]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));
}

#[test]
fn the_output_names_the_copy_with_its_publish_sequence_and_asks_for_a_commit() {
    let (scratch, _history, _remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(
        text.contains("added a copy of shelves:IF-1 at publish sequence 2"),
        "{text}"
    );
    assert!(
        text.contains("Commit spec/: it now holds a copy of shelves:IF-1."),
        "{text}"
    );
}

#[test]
fn an_interface_named_by_its_key_is_copied_from_the_publish_sequence_at_names() {
    let (scratch, _history, _remote) = ready(true);
    let named = format!("{HANDLE}:{LOOKUP}");
    let output = interface(&scratch, &["add", &named, "--at", "1", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document = conforming(&output);
    assert_eq!(document["interface"], "shelves:IF-1");
    assert_eq!(document["publishSequence"], 1);
    let copy = copy(&scratch);
    assert!(copy.contains("publishSequence: 1\n"), "{copy}");
    assert!(!copy.contains("and the author"), "{copy}");
}

#[test]
fn remote_lists_an_unlisted_handle_with_the_key_read_from_the_states_lock() {
    let (scratch, _history, remote) = ready(false);
    let address = remote.address();
    let output = interface(
        &scratch,
        &[
            "add",
            "shelves:IF-1",
            "--remote",
            &address,
            "--format",
            "json",
        ],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(conforming(&output)["listed"], true);
    let manifest = fs::read_to_string(scratch.file("loftspec.yaml")).unwrap();
    let entry = format!("documents:\n  - handle: {HANDLE}\n    key: {OTHER}\n    remote: ");
    assert!(manifest.contains(&entry), "{manifest}");
    assert!(manifest.contains(&address), "{manifest}");
}

#[test]
fn the_calls_made_are_the_head_and_the_published_state() {
    let (scratch, _history, remote) = ready(true);
    let output = interface(&scratch, &["add", "shelves:IF-1"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let calls: Vec<String> = (remote.requests().into_iter())
        .filter(|request| request.path.starts_with(&document_path(OTHER)))
        .map(|request| format!("{} {}", request.method, request.path))
        .collect();
    assert_eq!(
        calls,
        [
            format!("GET {}/head", document_path(OTHER)),
            format!("GET {}/states/2", document_path(OTHER)),
        ]
    );
}

#[test]
fn the_own_remotes_variables_change_no_call_to_another_documents_remote() {
    let (scratch, _history, remote) = ready(true);
    let own = Remote::start(RemoteScenario::accepting(
        "00000000-0000-4000-8000-000000000001",
    ));
    let (mut command, _browser) = signed_in::command(&scratch, BrowserScenario::Absent, None);
    let output = command
        .env("LOFT_REMOTE_ENDPOINT", own.address())
        .env("LOFT_REMOTE_TOKEN", TOKEN)
        .args(["interface", "add", "shelves:IF-1"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(own.requests().is_empty(), "{:?}", own.requests());
    let kept = sign_ins(&scratch)[remote.address()]["accessToken"]
        .as_str()
        .unwrap()
        .to_owned();
    let calls: Vec<_> = (remote.requests().into_iter())
        .filter(|request| request.path.starts_with(&document_path(OTHER)))
        .collect();
    assert_eq!(calls.len(), 2);
    for call in calls {
        assert_eq!(
            call.header("authorization"),
            Some(format!("Bearer {kept}").as_str())
        );
    }
}

#[test]
fn without_a_kept_sign_in_the_person_is_signed_in_to_the_other_remote_first() {
    let (scratch, _history, remote) = ready(true);
    keep(&scratch, &json!({}));
    let args = ["interface", "add", "shelves:IF-1"];
    let (output, browser) = signed_in::run(&scratch, &args, BrowserScenario::Visiting, None);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(browser.visits().len(), 1);
    assert!(sign_ins(&scratch)[remote.address()]["accessToken"].is_string());
    assert!(tree(&scratch.0).contains_key(&loftspec::codec::SpecPath::new(COPY)));
}
