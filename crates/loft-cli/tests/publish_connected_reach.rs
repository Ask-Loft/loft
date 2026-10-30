//! `loft publish` of a connected document, as the shared rules of reaching a remote hold it:
//! the address and token the environment names, the sign-in and its refresh, the refusals and
//! their waits, and the time limit, run under the remote and git simulators.

use std::fs;

use serde_json::json;
use software_plant::BrowserScenario;
use software_plant::remote::{RemoteScenario, TOKEN};

use crate::common::connected::{Connected, connected};
use crate::common::signed_in::{self, granted, keep, name_remote, sign_ins};
use crate::common::tree;

#[test]
fn the_environment_names_the_address_and_the_token_for_one_run() {
    let run = connected(1, 1, RemoteScenario::accepting);
    name_remote(
        &run.scratch,
        Some("https://elsewhere.example/documents/one"),
        true,
    );
    fs::remove_file(signed_in::sign_ins_file(&run.scratch)).unwrap();
    run.change();
    let address = run.remote.address();
    let output = run.publish_with(&[
        ("LOFT_REMOTE_ENDPOINT", &address),
        ("LOFT_REMOTE_TOKEN", TOKEN),
    ]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    for request in run.remote.requests() {
        assert_eq!(
            request.header("authorization"),
            Some(format!("Bearer {TOKEN}").as_str())
        );
    }
    assert!(!signed_in::sign_ins_file(&run.scratch).exists());
}

#[test]
fn without_a_sign_in_the_publish_is_refused_naming_the_address_and_login() {
    let run = connected(1, 1, RemoteScenario::accepting);
    fs::remove_file(signed_in::sign_ins_file(&run.scratch)).unwrap();
    run.change();
    let output = run.publish();
    assert_eq!(output.status.code(), Some(1));
    let message = Connected::message(&output);
    for said in [
        run.remote.address().as_str(),
        "LOFT_REMOTE_TOKEN",
        "loft remote login",
        &run.history.key,
    ] {
        assert!(message.contains(said), "{said}: {message}");
    }
    assert!(run.remote.requests().is_empty(), "no connection is opened");
}

#[test]
fn an_expiring_sign_in_is_refreshed_first_and_a_lapsed_one_names_login() {
    for (lapsing, code) in [(false, 0), (true, 1)] {
        let run = connected(1, 1, |key| {
            let scenario = RemoteScenario::accepting(key);
            if lapsing {
                scenario.lapsing()
            } else {
                scenario
            }
        });
        let kept = granted(&run.remote, 60);
        keep(&run.scratch, &json!({ run.remote.address(): kept }));
        run.change();
        let (mut command, browser) =
            signed_in::command(&run.scratch, BrowserScenario::Visiting, Some(&run.git));
        let output = command
            .args(["publish", "--format", "json"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        assert_eq!(
            browser.visits(),
            Vec::<String>::new(),
            "no browser is opened"
        );
        let now = &sign_ins(&run.scratch)[run.remote.address()];
        if lapsing {
            let message = Connected::message(&output);
            assert!(
                message.contains("lapsed") && message.contains("loft remote login"),
                "{message}"
            );
            assert_eq!(*now, kept, "nothing is written");
        } else {
            assert_eq!(run.remote.refreshes(), 1);
            assert_ne!(now["refreshToken"], kept["refreshToken"]);
            let publish = run
                .remote
                .requests()
                .into_iter()
                .find(|r| r.path.ends_with("/publish"))
                .unwrap();
            assert_eq!(
                publish.header("authorization"),
                Some(format!("Bearer {}", now["accessToken"].as_str().unwrap()).as_str())
            );
        }
    }
}

#[test]
fn a_refusal_is_reported_with_its_status_and_code_and_says_when_to_try_again() {
    for (status, code, later) in [
        (503, Some("service_unavailable"), true),
        (429, Some("rate_limited"), true),
        (403, Some("forbidden"), false),
        (418, Some("teapot"), false),
        (502, None, false),
    ] {
        let run = connected(1, 1, |key| RemoteScenario::failing(key, status, code));
        run.change();
        let before = tree(&run.scratch.0);
        let output = run.publish();
        assert_eq!(output.status.code(), Some(1), "{status}");
        let message = Connected::message(&output);
        assert!(message.contains(&run.remote.address()), "{message}");
        assert!(message.contains(&status.to_string()), "{message}");
        if let Some(code) = code {
            assert!(message.contains(code), "{message}");
        }
        assert_eq!(message.contains("try again later"), later, "{message}");
        assert!(
            message.contains("published only through its remote"),
            "{message}"
        );
        assert_eq!(tree(&run.scratch.0), before);
    }
}

#[test]
fn a_refusal_the_remote_may_lift_is_waited_out_once_where_loft_remote_wait_allows() {
    for (wait, calls) in [(None, 1), (Some("5"), 2), (Some("1"), 1)] {
        let run = connected(1, 1, |key| {
            RemoteScenario::failing(key, 503, Some("service_unavailable")).retry_after("2")
        });
        let env: Vec<(&str, &str)> = wait
            .map(|wait| ("LOFT_REMOTE_WAIT", wait))
            .into_iter()
            .collect();
        let output = run.publish_with(&env);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(run.calls().len(), calls, "{wait:?}");
    }
}

#[test]
fn a_timeout_or_wait_that_is_not_whole_seconds_is_refused_before_any_connection() {
    for (name, value) in [("LOFT_REMOTE_TIMEOUT", "soon"), ("LOFT_REMOTE_WAIT", "0")] {
        let run = connected(1, 1, RemoteScenario::accepting);
        run.change();
        let output = run.publish_with(&[(name, value)]);
        assert_eq!(output.status.code(), Some(1));
        let message = Connected::message(&output);
        assert!(message.contains(&format!("{name}={value}")), "{message}");
        assert!(run.remote.requests().is_empty());
    }
}

#[test]
fn a_remote_that_does_not_answer_within_the_timeout_is_abandoned() {
    let run = connected(1, 1, RemoteScenario::silent);
    run.change();
    let started = std::time::Instant::now();
    let output = run.publish_with(&[("LOFT_REMOTE_TIMEOUT", "1")]);
    assert_eq!(output.status.code(), Some(1));
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert!(Connected::message(&output).contains("cannot be reached"));
}
