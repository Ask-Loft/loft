//! The refresh of a sign-in kept in `loft/remotes.json` that is about to expire, when two
//! processes refresh it at once and when the answer to a refresh is lost, run with `loft pull`
//! of a tree at its remote's head, which reaches the remote and writes nothing.

use std::fs;
use std::process::Output;
use std::time::Duration;

use serde_json::{Value, json};
use software_plant::BrowserScenario;
use software_plant::remote::RemoteScenario;
use software_plant::remote::wire::{pairs, value};

use crate::common::connected::{Connected, connected};
use crate::common::signed_in::{self, granted, keep, sign_ins, sign_ins_file};

/// A tree at its remote's head, under `scenario`, whose kept sign-in expires in a minute.
fn expiring(scenario: impl FnOnce(&str) -> RemoteScenario) -> (Connected, Value) {
    let run = connected(1, 1, scenario);
    let kept = granted(&run.remote, 60);
    keep(&run.scratch, &json!({ run.remote.address(): kept }));
    (run, kept)
}

/// Starts `loft pull` over the tree of `run`.
fn pull(run: &Connected) -> std::process::Child {
    let (mut command, _) = signed_in::command(&run.scratch, BrowserScenario::Absent, None);
    command
        .args(["pull", "--format", "json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap()
}

/// The refresh token of each refresh the remote received, in order.
fn presented(run: &Connected) -> Vec<String> {
    run.remote
        .requests()
        .into_iter()
        .filter(|request| request.path == "/auth/token")
        .filter_map(|request| {
            let form = pairs(&String::from_utf8_lossy(&request.body));
            value(&form, "refresh_token").map(str::to_owned)
        })
        .collect()
}

#[test]
fn two_processes_that_refresh_one_sign_in_at_once_refresh_it_once() {
    // A slow refresh holds the first process while the second starts, and a token presented
    // again would revoke the sign-in, so a second refresh could not go unseen.
    let (run, kept) = expiring(|key| {
        RemoteScenario::accepting(key)
            .delaying_refreshes(Duration::from_millis(500))
            .without_grace()
    });
    let outputs: Vec<Output> = [pull(&run), pull(&run)]
        .into_iter()
        .map(|child| child.wait_with_output().unwrap())
        .collect();
    for output in &outputs {
        assert_eq!(output.status.code(), Some(0), "{output:?}");
    }
    assert_eq!(run.remote.refreshes(), 1);
    assert_eq!(presented(&run), [kept["refreshToken"].as_str().unwrap()]);
    let now = &sign_ins(&run.scratch)[run.remote.address()];
    let bearer = format!("Bearer {}", now["accessToken"].as_str().unwrap());
    let heads: Vec<_> = run
        .remote
        .requests()
        .into_iter()
        .filter(|request| request.path.ends_with("/head"))
        .collect();
    assert_eq!(heads.len(), 2);
    for head in heads {
        assert_eq!(head.header("authorization"), Some(bearer.as_str()));
    }
}

#[test]
fn a_refresh_whose_answer_is_lost_leaves_the_file_and_the_next_run_presents_its_token_again() {
    let (run, kept) = expiring(|key| RemoteScenario::accepting(key).losing_a_refresh_answer());
    let before = fs::read(sign_ins_file(&run.scratch)).unwrap();
    let lost = pull(&run).wait_with_output().unwrap();
    assert_eq!(lost.status.code(), Some(1), "{lost:?}");
    assert_eq!(fs::read(sign_ins_file(&run.scratch)).unwrap(), before);
    let again = pull(&run).wait_with_output().unwrap();
    assert_eq!(again.status.code(), Some(0), "{again:?}");
    let old = kept["refreshToken"].as_str().unwrap();
    assert_eq!(presented(&run), [old, old]);
    assert_eq!(
        run.remote.refreshes(),
        1,
        "the repeat is answered as the first was"
    );
    let now = &sign_ins(&run.scratch)[run.remote.address()];
    assert_ne!(now["refreshToken"], kept["refreshToken"]);
}
