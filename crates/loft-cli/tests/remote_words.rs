//! What a remote or an authorization server answers never reaches standard error as a control
//! character: an escape sequence in a problem document's `code` or `detail`, or in an OAuth
//! `error`, is written with U+FFFD in place of each control character. Each runs under the
//! remote and git simulators, with a server that answers every request the same way.

use std::process::Output;

use serde_json::json;
use software_plant::http::{Response, Server};
use software_plant::remote::{RemoteScenario, TOKEN, document_address};

use crate::common::connected::{Connected, connected};
use crate::common::remote::Always;

/// Clears the screen, renames the window, and ends with a C1 sequence, a DEL and a carriage
/// return.
const HOSTILE: &str = "\u{1b}[2J\u{1b}]0;renamed\u{7}gone\u{9b}31m\u{7f}\r";

/// Publishes with the variables `env`, its errors written as text on standard error.
fn publish(run: &Connected, env: &[(&str, &str)]) -> Output {
    let mut command = run.command();
    command
        .envs(env.iter().copied())
        .arg("publish")
        .output()
        .unwrap()
}

/// What the failed run wrote to standard error, after asserting that it holds `said`, no
/// control character but line feeds and tabs, and the replacement character where one was.
fn written_without_controls(output: &Output, said: &str) -> String {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = String::from_utf8(output.stderr.clone()).unwrap();
    assert!(text.contains(said), "{text:?}");
    let controls: Vec<char> = text
        .chars()
        .filter(|character| character.is_control() && !matches!(character, '\n' | '\t'))
        .collect();
    assert_eq!(controls, Vec::<char>::new(), "{text:?}");
    assert!(text.contains('\u{fffd}'), "{text:?}");
    text
}

#[test]
fn a_problem_document_s_code_and_detail_reach_standard_error_without_their_control_characters() {
    let run = connected(1, 1, RemoteScenario::accepting);
    run.change();
    let refusal = json!({ "code": "con\u{1b}[31mflict", "detail": HOSTILE });
    let server = Server::start(Always(Response::json(409, &refusal)));
    let address = document_address(&server.url(), &run.history.key);
    let output = publish(
        &run,
        &[
            ("LOFT_REMOTE_ENDPOINT", &address),
            ("LOFT_REMOTE_TOKEN", TOKEN),
        ],
    );
    let text = written_without_controls(&output, "gone");
    assert!(text.contains("con\u{fffd}[31mflict"), "{text:?}");
    assert!(text.contains("\u{fffd}[2J"), "{text:?}");
}

#[test]
fn an_authorization_server_s_error_reaches_standard_error_without_its_control_characters() {
    let run = connected(1, 1, RemoteScenario::accepting);
    run.change();
    let refusal = json!({ "error": "invalid\u{1b}[2J_grant\u{9b}" });
    let server = Server::start(Always(Response::json(400, &refusal)));
    run.refreshing_at(&format!("{}/token", server.url()));
    let text = written_without_controls(&publish(&run, &[]), "_grant");
    assert!(
        text.contains("invalid\u{fffd}[2J_grant\u{fffd}"),
        "{text:?}"
    );
}
