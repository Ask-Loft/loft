//! The contract suite of the failures a remote simulator scripts: metadata a misconfigured
//! authorization server serves, refused registrations and exchanges, a refresh whose answer is
//! lost or slow, and the publish and import answers no faithful remote gives. A real remote is
//! not asked to fail, so each scenario is asked of the simulator alone.

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use software_plant::remote::wire::query;
use software_plant::remote::{Metadata, Remote, RemoteScenario};

use crate::common::remote::{spec, wire, with_criterion};
use crate::remote_contract::{agent, assert_problem, call, holding, post};
use crate::remote_contract_auth::{KEY, coded, exchange, metadata, refresh, register};
use crate::remote_contract_writes::{import_body, publish_body};

/// The challenge of a request without a token, and both metadata documents, as served by a
/// scenario of `metadata`, with no check of what they say.
fn served(metadata: Metadata) -> (Remote, String, Value, Value) {
    let remote = Remote::start(RemoteScenario::accepting(KEY).metadata(metadata));
    let challenge = call("GET", &format!("{}/head", remote.address()), None, None);
    let challenge = challenge.header("www-authenticate").unwrap().to_owned();
    let resource_url = format!(
        "{}/.well-known/oauth-protected-resource/documents",
        remote.origin()
    );
    let resource = call("GET", &resource_url, None, None).body;
    let server_url = format!(
        "{}/.well-known/oauth-authorization-server/auth",
        remote.origin()
    );
    let server = call("GET", &server_url, None, None).body;
    (remote, challenge, resource, server)
}

#[test]
fn each_misconfigured_metadata_serves_what_its_scenario_names() {
    let (_remote, challenge, resource, _) = served(Metadata::Unscoped(vec![
        "documents".into(),
        "profile".into(),
    ]));
    assert!(!challenge.contains("scope="), "{challenge}");
    assert_eq!(
        resource["scopes_supported"],
        json!(["documents", "profile"])
    );

    let (remote, _, resource, _) = served(Metadata::MisnamedResource);
    assert_ne!(
        resource["resource"],
        json!(format!("{}/documents", remote.origin()))
    );

    let (_remote, _, resource, server) = served(Metadata::MisnamedIssuer);
    assert_ne!(server["issuer"], resource["authorization_servers"][0]);

    let (_remote, _, _, server) = served(Metadata::Unregistering);
    assert!(server.get("registration_endpoint").is_none(), "{server}");

    let (_remote, _, _, server) = served(Metadata::Plain);
    assert_eq!(server["code_challenge_methods_supported"], json!(["plain"]));

    let (_remote, _, _, server) = served(Metadata::Unencrypted);
    let endpoint = server["authorization_endpoint"].as_str().unwrap();
    assert!(
        endpoint.starts_with("http://") && !endpoint.starts_with("http://127.0.0.1"),
        "{endpoint}"
    );
}

#[test]
fn a_scenario_refuses_every_registration_or_exchange_with_the_oauth_error_it_names() {
    let remote =
        Remote::start(RemoteScenario::accepting(KEY).refusing_registration("access_denied"));
    let (_, server) = metadata(&remote);
    let refused = register(&server, &["http://127.0.0.1/callback"], "none");
    assert_eq!(
        (refused.status, refused.body["error"].clone()),
        (400, json!("access_denied"))
    );

    let remote = Remote::start(RemoteScenario::accepting(KEY).refusing_exchange("invalid_grant"));
    let (server, client, resource, code) = coded(&remote);
    let refused = exchange(&server, &client, &resource, &code);
    assert_eq!(
        (refused.status, refused.body["error"].clone()),
        (400, json!("invalid_grant"))
    );
}

#[test]
fn a_lost_refresh_answer_rotates_the_token_and_closes_the_connection_once() {
    let remote = Remote::start(RemoteScenario::accepting(KEY).losing_a_refresh_answer());
    let (server, client, resource, code) = coded(&remote);
    let first = exchange(&server, &client, &resource, &code).body;
    let old = first["refresh_token"].as_str().unwrap();
    let fields = [
        ("grant_type", "refresh_token"),
        ("refresh_token", old),
        ("client_id", client.as_str()),
        ("resource", resource.as_str()),
    ];
    let lost = agent()
        .post(server["token_endpoint"].as_str().unwrap())
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(query(&fields));
    assert!(lost.is_err(), "the answer is lost: {lost:?}");
    assert_eq!(remote.refreshes(), 1, "the token rotated");
    // Within the grace window, the same token gets the answer that was lost.
    let repeated = refresh(&server, &client, &resource, old);
    assert_eq!(repeated.status, 200, "{:?}", repeated.body);
    assert_eq!(remote.refreshes(), 1);
    let next = repeated.body["refresh_token"].as_str().unwrap();
    assert_eq!(refresh(&server, &client, &resource, next).status, 200);
}

#[test]
fn a_delaying_scenario_answers_each_refresh_only_after_its_delay() {
    let delay = Duration::from_millis(400);
    let remote = Remote::start(RemoteScenario::accepting(KEY).delaying_refreshes(delay));
    let (server, client, resource, code) = coded(&remote);
    let first = exchange(&server, &client, &resource, &code).body;
    let started = Instant::now();
    let answer = refresh(
        &server,
        &client,
        &resource,
        first["refresh_token"].as_str().unwrap(),
    );
    assert_eq!(answer.status, 200);
    assert!(started.elapsed() >= delay, "{:?}", started.elapsed());
}

#[test]
fn a_scenario_refuses_a_publish_as_invalid_naming_the_paths_it_names() {
    let (_, history) = holding(1);
    let remote = Remote::start(
        RemoteScenario::accepting(&history.key)
            .holding("Reading List", history.states.clone())
            .refusing_publish_as_invalid(&["spec/products/reading-list/product.yaml"]),
    );
    let base = &history.states[0];
    let body = publish_body(&history.key, base, &base.files, "retry-1");
    let refused = post(&remote, "/publish", &body);
    assert_problem(&refused, 400, "invalid_request");
    assert_eq!(
        refused.body["paths"],
        json!(["spec/products/reading-list/product.yaml"])
    );
}

#[test]
fn a_misstating_scenario_answers_a_lock_or_an_import_head_its_states_do_not_have() {
    let (_, history) = holding(1);
    let remote = Remote::start(
        RemoteScenario::accepting(&history.key)
            .holding("Reading List", history.states.clone())
            .misstating_its_lock(),
    );
    let base = &history.states[0];
    let changed = wire(&with_criterion(
        &spec(&base.files),
        "01a18e30-0000-7000-8000-0000000000ab",
    ));
    let answer = post(
        &remote,
        "/publish",
        &publish_body(&history.key, base, &changed, "retry-1"),
    );
    assert_eq!(answer.body["outcome"], "published", "{:?}", answer.body);
    let stored = &remote.states()[1].lock;
    assert_eq!(
        answer.body["lock"]["publishSequence"],
        stored["publishSequence"]
    );
    assert_ne!(answer.body["lock"]["stateRoot"], stored["stateRoot"]);

    let remote = Remote::start(
        RemoteScenario::accepting(&history.key)
            .holding("Reading List", Vec::new())
            .misstating_its_import_head(),
    );
    let answer = post(
        &remote,
        "/import",
        &import_body(&history.key, &history.states),
    );
    assert_eq!(answer.status, 200, "{:?}", answer.body);
    assert_ne!(
        answer.body["head"]["stateRoot"],
        history.states[0].lock["stateRoot"]
    );
}
