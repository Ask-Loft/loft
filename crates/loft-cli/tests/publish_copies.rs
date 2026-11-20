//! `loft publish` of a connected document that holds a copy of another document's Interface:
//! each copy is confirmed against the state its pin names, at the other document's remote, before
//! anything is sent to the document's own remote.

use std::fs;

use serde_json::json;
use software_plant::BrowserScenario;
use software_plant::remote::{Remote, RemoteScenario};

use crate::common::connected::{Connected, connected, paths};
use crate::common::others::{COPY, list, shelves, shelves_remote};
use crate::common::signed_in::{self, document, granted, keep, sign_ins};
use crate::common::{stderr, tree};

/// A published, connected tree with a criterion to publish and a copy of the other document's
/// "Shelf lookup", with a sign-in to each remote.
fn holding_a_copy() -> (Connected, Remote) {
    let run = connected(2, 2, RemoteScenario::accepting);
    // The change goes to the first file that holds criteria, which the copy would be once added.
    run.change();
    let other = shelves_remote(&shelves());
    list(&run.scratch, &other.address());
    keep(
        &run.scratch,
        &json!({
            run.remote.address(): granted(&run.remote, 3600),
            other.address(): granted(&other, 3600),
        }),
    );
    let args = ["interface", "add", "shelves:IF-1"];
    let (output, _) = signed_in::run(&run.scratch, &args, BrowserScenario::Absent, None);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    (run, other)
}

/// The requests to the other document's states, each with the bearer token it carried.
fn state_calls(other: &Remote) -> Vec<(String, Option<String>)> {
    (other.requests().into_iter())
        .filter(|request| request.path.contains("/states/"))
        .map(|request| {
            let token = request.header("authorization").map(ToOwned::to_owned);
            (request.path, token)
        })
        .collect()
}

#[test]
fn a_publish_whose_copies_match_is_sent_as_any_publish_is() {
    let (run, other) = holding_a_copy();
    let before = state_calls(&other).len();
    let output = run.publish();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(document(&output)["outcome"], "published");
    let calls = state_calls(&other);
    assert_eq!(calls.len(), before + 1, "{calls:?}");
    let kept = sign_ins(&run.scratch)[other.address()]["accessToken"]
        .as_str()
        .unwrap()
        .to_owned();
    let (path, token) = calls.last().unwrap();
    assert!(path.ends_with("/states/2"), "{path}");
    assert_eq!(token.as_deref(), Some(format!("Bearer {kept}").as_str()));
    let added = paths(&run.sent(), "added");
    assert!(added.contains(&format!("spec/{COPY}")), "{added:?}");
}

#[test]
fn a_copy_that_differs_is_refused_naming_its_file_and_nothing_is_sent_or_written() {
    let (run, _other) = holding_a_copy();
    let text = fs::read_to_string(run.scratch.file(COPY)).unwrap();
    let root = (text.lines())
        .find_map(|line| line.strip_prefix("stateRoot: "))
        .unwrap()
        .trim_matches('"')
        .to_owned();
    fs::write(
        run.scratch.file(COPY),
        text.replace(&root, &"cd".repeat(32)),
    )
    .unwrap();
    let before = tree(&run.scratch.0);
    let lock = run.lock();
    let output = run.publish();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = Connected::message(&output);
    assert!(message.contains(&format!("spec/{COPY}")), "{message}");
    assert!(message.contains("state root"), "{message}");
    assert!(run.calls().is_empty(), "{:?}", run.calls());
    assert_eq!(tree(&run.scratch.0), before);
    assert_eq!(run.lock(), lock);
}
