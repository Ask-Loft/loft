//! The contract suite of the remote simulator for a document its remote serves to anyone: its
//! head and its published states are answered with no token, its drafts and its writes are
//! challenged as for every document, and its head names to whom the remote serves it.

use serde_json::{Value, json};
use software_plant::remote::{Remote, RemoteScenario, TOKEN, Visibility};

use crate::common::calls::call;
use crate::common::remote::history;
use crate::remote_contract::{assert_problem, get};

/// A remote holding the Reading List published twice, whose scenario names `visibility`.
fn serving(visibility: Option<Visibility>) -> Remote {
    let history = history(2);
    let scenario =
        RemoteScenario::accepting(&history.key).holding("Reading List", history.states.clone());
    Remote::start(match visibility {
        Some(visibility) => scenario.visibility(visibility),
        None => scenario,
    })
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

#[test]
fn a_public_document_s_drafts_and_writes_are_challenged_without_a_token() {
    let remote = serving(Some(Visibility::Public));
    let address = remote.address();
    for (method, path, body) in [
        ("GET", "/drafts/main", None),
        ("POST", "/publish", Some(json!({}))),
    ] {
        let answer = call(method, &format!("{address}{path}"), None, body.as_ref());
        assert_problem(&answer, 401, "unauthenticated");
        let challenge = answer.header("www-authenticate").unwrap();
        assert!(
            challenge.starts_with("Bearer resource_metadata=\""),
            "{challenge}"
        );
    }
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
    assert_problem(&answer, 401, "unauthenticated");
}

#[test]
fn a_public_scenario_that_holds_no_document_challenges_its_head_without_a_token() {
    let scenario = RemoteScenario::accepting("01a20625-0000-7000-8000-000000000001")
        .visibility(Visibility::Public);
    let remote = Remote::start(scenario);
    let answer = call("GET", &format!("{}/head", remote.address()), None, None);
    assert_problem(&answer, 401, "unauthenticated");
}
