//! What Loft refuses before it reaches a remote: a token that the environment names without an
//! address, which would otherwise go wherever the manifest's `remote` points, and an address
//! that holds a query, a fragment or user information. Each runs under the remote and git
//! simulators.

use std::process::Output;

use software_plant::BrowserScenario;
use software_plant::remote::{Remote, RemoteScenario, TOKEN};

use crate::common::connected::{Connected, connected};
use crate::common::remote::history;
use crate::common::signed_in::{self, document, run, sign_ins_file};
use crate::common::{Scratch, tree};
use crate::remote_connect::published;

/// Asserts that `output` is a refusal whose message holds each of `said` and none of `unsaid`.
fn refused(output: &Output, said: &[&str], unsaid: &[&str]) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let message = Connected::message(output);
    for text in said {
        assert!(message.contains(text), "{text}: {message}");
    }
    for text in unsaid {
        assert!(!message.contains(text), "{text}: {message}");
    }
}

#[test]
fn a_publish_with_a_token_from_the_environment_and_no_address_from_it_opens_no_connection() {
    let run = connected(1, 1, RemoteScenario::accepting);
    run.change();
    let before = tree(&run.scratch.0);
    let file = std::fs::read(sign_ins_file(&run.scratch)).ok();
    // A variable set to the empty string counts as unset.
    for endpoint in [None, Some("")] {
        let output = run.publish_by(|command| {
            command.env("LOFT_REMOTE_TOKEN", TOKEN);
            if let Some(endpoint) = endpoint {
                command.env("LOFT_REMOTE_ENDPOINT", endpoint);
            }
        });
        refused(
            &output,
            &[
                "LOFT_REMOTE_TOKEN",
                "LOFT_REMOTE_ENDPOINT",
                "only to an address",
            ],
            &[TOKEN],
        );
        assert!(run.remote.requests().is_empty(), "no connection is opened");
        assert_eq!(tree(&run.scratch.0), before, "nothing is written");
        assert_eq!(std::fs::read(sign_ins_file(&run.scratch)).ok(), file);
    }
}

#[test]
fn a_pull_with_a_token_from_the_environment_and_no_address_from_it_opens_no_connection() {
    let run = connected(1, 1, RemoteScenario::accepting);
    let before = tree(&run.scratch.0);
    let (mut command, _) = signed_in::command(&run.scratch, BrowserScenario::Absent, None);
    let output = command
        .env("LOFT_REMOTE_TOKEN", TOKEN)
        .args(["pull", "--format", "json"])
        .output()
        .unwrap();
    refused(
        &output,
        &["LOFT_REMOTE_TOKEN", "LOFT_REMOTE_ENDPOINT"],
        &[TOKEN],
    );
    assert!(run.remote.requests().is_empty(), "no connection is opened");
    assert_eq!(tree(&run.scratch.0), before, "nothing is written");
}

/// The remote's address with each part an address may not hold, and the name of that part.
fn with_parts(address: &str) -> [(String, &'static str); 4] {
    let with_user = address.replacen("://", "://someone:a-secret@", 1);
    [
        (format!("{address}?tenant=other"), "query"),
        (format!("{address}?"), "query"),
        (format!("{address}#head"), "fragment"),
        (with_user, "user information"),
    ]
}

#[test]
fn an_address_with_a_query_a_fragment_or_user_information_is_never_contacted() {
    let run = connected(1, 1, RemoteScenario::accepting);
    run.change();
    let before = tree(&run.scratch.0);
    for (address, part) in with_parts(&run.remote.address()) {
        let output = run.publish_with(&[
            ("LOFT_REMOTE_ENDPOINT", &address),
            ("LOFT_REMOTE_TOKEN", TOKEN),
        ]);
        refused(&output, &[part], &["a-secret", "someone"]);
        assert!(run.remote.requests().is_empty(), "{address}");
        assert_eq!(tree(&run.scratch.0), before, "{address}");
    }
}

#[test]
fn an_address_with_none_of_the_three_is_reached_as_it_is_given() {
    let run = connected(1, 1, RemoteScenario::accepting);
    run.change();
    let address = run.remote.address();
    let output = run.publish_with(&[
        ("LOFT_REMOTE_ENDPOINT", &address),
        ("LOFT_REMOTE_TOKEN", TOKEN),
    ]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(!run.remote.requests().is_empty());
}

#[test]
fn set_refuses_an_address_with_a_query_a_fragment_or_user_information_and_writes_nothing() {
    let origin = signed_in::example_address("https://remote.example");
    for (address, part) in with_parts(&origin) {
        let scratch = Scratch::with_reading_list();
        let before = tree(&scratch.0);
        let (output, browser) = run(
            &scratch,
            &["remote", "set", &address, "--format", "json"],
            BrowserScenario::Visiting,
            None,
        );
        assert_eq!(output.status.code(), Some(1), "{address}");
        let message = document(&output)["errors"][0]["message"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(message.contains(part), "{address}: {message}");
        assert!(message.contains("remote.example"), "{address}: {message}");
        assert!(!message.contains("a-secret"), "{address}: {message}");
        assert_eq!(browser.visits(), Vec::<String>::new(), "{address}");
        assert_eq!(tree(&scratch.0), before, "{address}");
        assert!(!sign_ins_file(&scratch).exists(), "{address}");
    }
}

#[test]
fn set_and_login_with_a_token_alone_in_the_environment_are_not_refused_and_send_none_of_it() {
    let history = history(1);
    let (scratch, git) = published(&history);
    let remote = Remote::start(RemoteScenario::accepting(&history.key));
    let address = remote.address();
    let token = "a-token-from-the-environment";
    for args in [
        &["remote", "set", address.as_str()][..],
        &["remote", "login"],
    ] {
        let (mut command, _browser) =
            signed_in::command(&scratch, BrowserScenario::Visiting, Some(&git));
        let output = command
            .env("LOFT_REMOTE_TOKEN", token)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
    }
    for request in remote.requests() {
        let sent = request.header("authorization").unwrap_or_default();
        assert!(!sent.contains(token), "{}", request.path);
    }
}
