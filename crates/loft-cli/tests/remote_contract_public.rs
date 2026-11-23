//! The contract suite of the remote simulator for a document its remote serves to anyone: its
//! head and its published states are answered with no token, its drafts and its writes are
//! challenged as for every document, and its head names to whom the remote serves it.

use serde_json::{Value, json};
use software_plant::remote::{Remote, RemoteScenario, TOKEN, Visibility};

use crate::common::calls::{Answer, call};
use crate::common::remote::history;
use crate::remote_contract::{assert_problem, get};

/// A remote holding the Reading List published twice, whose scenario names `visibility`.
fn serving(visibility: Option<Visibility>) -> Remote {
    let history = history(2);
    let scenario =
        RemoteScenario::accepting(&history.key).holding("Reading List", history.states.clone());
    Remote::start(
        visibility
            .into_iter()
            .fold(scenario, RemoteScenario::visibility),
    )
}

#[test]
fn a_public_document_s_head_and_states_are_answered_without_a_token_as_with_one() {
    let remote = serving(Some(Visibility::Public));
    for path in ["/head", "/states/1", "/states/2"] {
        let url = format!("{}{path}", remote.address());
        let anonymous = call("GET", &url, None, None);
        assert_eq!(anonymous.status, 200, "{path}: {anonymous:?}");
        let member = call("GET", &url, Some(TOKEN), None);
        assert_eq!(anonymous.body, member.body, "{path}");
    }
}

/// Asserts what the remote of the public document at `address`, which holds a published state,
/// answers a client that sends no token: its head, naming the visibility `public` and the
/// document key, and its first published state; its draft and its publish are challenged, so
/// that a sign-in reads the challenge from its draft. The simulator and a real remote are held to
/// the same assertions.
fn assert_public_answers(address: &str) {
    let head = call("GET", &format!("{address}/head"), None, None);
    assert_eq!(head.status, 200, "{head:?}");
    assert_eq!(head.body["visibility"], "public");
    assert!(head.body["documentKey"].is_string(), "{head:?}");
    assert!(
        head.body["head"]["publishSequence"].as_u64() >= Some(1),
        "{head:?}"
    );
    let state = call("GET", &format!("{address}/states/1"), None, None);
    assert_eq!(state.status, 200, "{state:?}");
    assert!(state.body["lock"].is_object(), "{state:?}");
    assert!(state.body["files"].is_array(), "{state:?}");
    for (method, path, body) in [
        ("GET", "/drafts/main", None),
        ("POST", "/publish", Some(json!({}))),
    ] {
        let answer = call(method, &format!("{address}{path}"), None, body.as_ref());
        assert_challenged(&answer);
    }
}

#[test]
fn a_public_document_answers_a_client_without_a_token_as_the_contract_states() {
    assert_public_answers(&serving(Some(Visibility::Public)).address());
}

#[test]
#[ignore = "reaches the public document at LOFT_REMOTE_CONTRACT_PUBLIC_ADDRESS"]
fn a_real_remote_answers_a_client_without_a_token_for_a_public_document_as_the_simulator_does() {
    let address = std::env::var("LOFT_REMOTE_CONTRACT_PUBLIC_ADDRESS").expect("an address");
    assert_public_answers(&address);
}

/// Asserts that `answer` is `401 Unauthorized` with the code `unauthenticated` and the challenge.
fn assert_challenged(answer: &Answer) {
    assert_problem(answer, 401, "unauthenticated");
    let challenge = answer.header("www-authenticate").unwrap();
    assert!(
        challenge.starts_with("Bearer resource_metadata=\""),
        "{challenge}"
    );
}

#[test]
fn the_head_names_the_visibility_its_scenario_names_beside_the_document_key() {
    for (visibility, named) in [
        (Some(Visibility::Public), json!("public")),
        (Some(Visibility::Private), json!("private")),
        (None, Value::Null),
    ] {
        let remote = serving(visibility);
        let head = get(&remote, "/head").body;
        assert_eq!(head["visibility"], named, "{visibility:?}");
        assert!(head["documentKey"].is_string());
        let named = head.as_object().unwrap().contains_key("visibility");
        assert_eq!(named, visibility.is_some(), "{visibility:?}");
    }
}

#[test]
fn a_private_document_s_head_is_challenged_without_a_token() {
    let remote = serving(Some(Visibility::Private));
    let answer = call("GET", &format!("{}/head", remote.address()), None, None);
    assert_challenged(&answer);
}

#[test]
fn a_public_scenario_that_holds_no_document_challenges_its_head_without_a_token() {
    let scenario = RemoteScenario::accepting("01a20625-0000-7000-8000-000000000001")
        .visibility(Visibility::Public);
    let remote = Remote::start(scenario);
    let answer = call("GET", &format!("{}/head", remote.address()), None, None);
    assert_challenged(&answer);
}
