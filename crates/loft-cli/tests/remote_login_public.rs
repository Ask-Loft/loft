//! A sign-in to the remote of a document it serves to anyone: its head answers without a
//! challenge, so the sign-in reads the challenge from the document's drafts; a sign-in to the
//! remote of a private document asks its head alone; and a remote that challenges neither is
//! refused naming the request it asked last.

use serde_json::json;
use software_plant::BrowserScenario;
use software_plant::http::{Answer, Request, Responder, Response, Server};
use software_plant::remote::{Remote, RemoteScenario, Visibility, document_address, document_path};

use crate::common::Scratch;
use crate::common::remote::history;
use crate::common::signed_in::{name_remote, run, sign_ins_file};
use crate::remote_sign_in_refusals::login;

/// The requests under the document's address that `remote` received without a token, by their
/// method and their path below the address.
fn unauthenticated(remote: &Remote, key: &str) -> Vec<String> {
    let document = document_path(key);
    remote
        .requests()
        .into_iter()
        .filter(|request| request.header("Authorization").is_none())
        .filter_map(|request| {
            let below = request.path.strip_prefix(&document)?.to_owned();
            Some(format!("{} {below}", request.method))
        })
        .collect()
}

/// A sign-in to a remote holding the Reading List, whose scenario names `visibility`: the remote,
/// the scratch, the run's output and the addresses the browser visited, and the document's key.
fn signed_in_to(visibility: Visibility) -> (Remote, Scratch, std::process::Output, String) {
    let history = history(1);
    let scenario = RemoteScenario::accepting(&history.key)
        .holding("Reading List", history.states.clone())
        .visibility(visibility);
    let (remote, scratch, output, visits) = login(scenario);
    assert_eq!(visits.len(), 1, "{visits:?}");
    (remote, scratch, output, history.key)
}

#[test]
fn a_sign_in_reads_the_challenge_from_the_drafts_where_the_head_answers_without_one() {
    let (remote, scratch, output, key) = signed_in_to(Visibility::Public);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(sign_ins_file(&scratch).exists());
    assert_eq!(
        unauthenticated(&remote, &key),
        ["GET /head", "GET /drafts/main"]
    );
}

#[test]
fn a_sign_in_to_a_private_document_s_remote_asks_its_head_alone_for_the_challenge() {
    let (remote, scratch, output, key) = signed_in_to(Visibility::Private);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(sign_ins_file(&scratch).exists());
    assert_eq!(unauthenticated(&remote, &key), ["GET /head"]);
}

/// A server whose head answers `200` and whose drafts answer `404`, both without a challenge.
struct Unchallenging;

impl Responder for Unchallenging {
    fn answer(&mut self, request: &Request) -> Answer {
        let status = if request.path.ends_with("/head") {
            200
        } else {
            404
        };
        Response::json(status, &json!({})).into()
    }
}

#[test]
fn a_remote_that_challenges_neither_head_nor_drafts_is_refused_naming_the_drafts_request() {
    let server = Server::start(Unchallenging);
    let address = document_address(&server.url(), "doc-unchallenging");
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(&address), true);
    let (output, browser) = run(
        &scratch,
        &["remote", "login"],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let request = format!("GET {address}/drafts/main answered 404");
    assert!(stderr.contains(&request), "{stderr}");
    assert!(
        stderr.contains("may not be a document's remote"),
        "{stderr}"
    );
    assert_eq!(browser.visits(), Vec::<String>::new());
    assert!(!sign_ins_file(&scratch).exists());
}
