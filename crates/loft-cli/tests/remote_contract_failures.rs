//! The contract suite of the failures a remote simulator scripts: metadata a misconfigured
//! authorization server serves, related resources it misserves or places on another origin, a
//! list of them it scripts, refused registrations and exchanges, a refresh whose answer is lost
//! or slow, and the publish and import answers no faithful remote gives. A real remote is not
//! asked to fail, so each scenario is asked of the simulator alone.

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use software_plant::http::Request;
use software_plant::remote::wire::value;
use software_plant::remote::{Metadata, Related, RelatedFault, Remote, RemoteScenario};

use crate::common::oauth::{
    KEY, assert_oauth_error, authorize_for, coded, exchange, fetch, location, metadata, parameter,
    refresh, register, registered, related_metadata, send_refresh,
};
use crate::common::remote::{spec, wire, with_criterion};
use crate::common::resources::{RELATED_RESOURCES, origin_of, signed_over};
use crate::remote_contract::{assert_problem, holding, post};
use crate::remote_contract_writes::{import_body, publish_body};

/// The challenge of a request without a token, and both metadata documents, as served by a
/// scenario of `metadata`, with no check of what they say.
fn served(metadata: Metadata) -> (Remote, String, Value, Value) {
    let remote = Remote::start(RemoteScenario::accepting(KEY).metadata(metadata));
    let (challenge, resource, server) = fetch(&remote.address());
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
fn each_misserved_related_resource_serves_what_its_scenario_names() {
    let faults = [
        ("other", RelatedFault::UnderAnotherIssuer),
        ("misnamed", RelatedFault::Misnamed),
        ("nameless", RelatedFault::Unnamed),
        ("unscoped", RelatedFault::Scopeless),
        ("empty", RelatedFault::EmptyScopes),
        ("unreadable", RelatedFault::Unreadable),
        ("array", RelatedFault::NotAnObject),
        ("relisting", RelatedFault::Relisting),
    ];
    let scenario = faults
        .iter()
        .fold(RemoteScenario::accepting(KEY), |scenario, (kind, fault)| {
            scenario.relating(Related::new(kind, &[*kind]).faulty(*fault))
        });
    let remote = Remote::start(scenario);
    let (resource, _) = metadata(&remote);
    let listed: Vec<&Value> = resource[RELATED_RESOURCES]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| &entry["kind"])
        .collect();
    let kinds = faults.map(|(kind, _)| json!(kind));
    assert_eq!(listed, kinds.iter().collect::<Vec<_>>());
    let answer_for = |kind: &str| related_metadata(&remote.related(kind));
    for kind in [
        "other", "misnamed", "nameless", "unscoped", "empty", "array",
    ] {
        assert_eq!(answer_for(kind).status, 200, "{kind}");
    }
    let issuer = json!(remote.issuer());
    let other = answer_for("other").body;
    let servers = other["authorization_servers"].as_array().unwrap();
    assert!(!servers.is_empty() && !servers.contains(&issuer), "{other}");
    let misnamed = answer_for("misnamed").body["resource"].clone();
    assert!(misnamed.is_string(), "{misnamed}");
    assert_ne!(misnamed, json!(remote.related("misnamed")));
    assert!(answer_for("nameless").body.get("resource").is_none());
    assert!(
        answer_for("unscoped")
            .body
            .get("scopes_supported")
            .is_none()
    );
    assert_eq!(answer_for("empty").body["scopes_supported"], json!([]));
    assert_problem(&answer_for("unreadable"), 404, "not_found");
    assert!(answer_for("array").body.is_array());
    let relisting = answer_for("relisting");
    assert_eq!(relisting.status, 200);
    let mut faithful = relisting.body;
    let relisted = faithful
        .as_object_mut()
        .and_then(|fields| fields.remove(RELATED_RESOURCES));
    let listing = relisted.is_some_and(|list| list.as_array().is_some_and(|l| !l.is_empty()));
    assert!(listing, "{faithful}");
    assert_eq!(
        faithful,
        json!({
            "resource": remote.related("relisting"),
            "authorization_servers": [issuer],
            "scopes_supported": ["relisting"],
            "bearer_methods_supported": ["header"],
        })
    );
}

#[test]
fn an_authorization_request_for_a_misserved_related_resource_is_refused_invalid_target() {
    let faults = [
        ("other", RelatedFault::UnderAnotherIssuer),
        ("misnamed", RelatedFault::Misnamed),
        ("nameless", RelatedFault::Unnamed),
        ("unscoped", RelatedFault::Scopeless),
        ("empty", RelatedFault::EmptyScopes),
        ("unreadable", RelatedFault::Unreadable),
        ("array", RelatedFault::NotAnObject),
        ("relisting", RelatedFault::Relisting),
        ("away", RelatedFault::OnAnotherOrigin),
    ];
    let scenario = faults
        .iter()
        .fold(RemoteScenario::accepting(KEY), |scenario, (kind, fault)| {
            scenario.relating(Related::new(kind, &[*kind]).faulty(*fault))
        });
    let remote = Remote::start(scenario);
    let (_, server) = metadata(&remote);
    let client = registered(&server);
    let own = remote.resource();
    // Each asks for the scope its scenario names for it, which no other resource accepts, so a
    // resource judged after the scope would be refused `invalid_scope`.
    for (kind, _) in faults {
        let related = remote.related(kind);
        let scope = format!("documents {kind}");
        let answer = authorize_for(&server, &client, &[&own, &related], &scope);
        let redirected = location(&answer);
        let refused = (kind != "relisting").then_some("invalid_target");
        assert_eq!(value(&redirected, "error"), refused, "{kind}");
        let coded = value(&redirected, "code").is_some();
        assert_eq!(coded, refused.is_none(), "{kind}");
    }
}

#[test]
fn a_second_port_serves_what_a_scenario_places_on_another_origin_and_records_its_own_requests() {
    let paths = |requests: Vec<Request>| -> Vec<String> {
        requests.into_iter().map(|request| request.path).collect()
    };
    let away = Related::new("away", &["away"]).faulty(RelatedFault::OnAnotherOrigin);
    let remote = Remote::start(RemoteScenario::accepting(KEY).relating(away));
    let (resource, _) = metadata(&remote);
    let identifier = remote.related("away");
    let elsewhere = origin_of(&identifier);
    assert!(elsewhere.starts_with("http://127.0.0.1:"), "{elsewhere}");
    assert_ne!(elsewhere, remote.origin());
    assert_eq!(
        resource[RELATED_RESOURCES],
        json!([{ "resource": identifier, "kind": "away" }])
    );
    assert!(remote.requests_elsewhere().is_empty());
    let answer = related_metadata(&identifier);
    assert_eq!(answer.status, 200, "{:?}", answer.body);
    assert_eq!(
        answer.body,
        json!({
            "resource": identifier,
            "authorization_servers": [remote.issuer()],
            "scopes_supported": ["away"],
            "bearer_methods_supported": ["header"],
        })
    );
    let metadata_path = "/.well-known/oauth-protected-resource";
    let away_metadata = format!("{metadata_path}/away");
    assert_eq!(paths(remote.requests_elsewhere()), [away_metadata.as_str()]);
    let first = paths(remote.requests());
    assert!(
        first.iter().all(|path| !path.ends_with("/away")),
        "{first:?}"
    );

    let moved = Remote::start(RemoteScenario::accepting(KEY).metadata(Metadata::OnAnotherOrigin));
    let (challenge, resource, _) = fetch(&moved.address());
    let named = parameter(&challenge, "resource_metadata").unwrap();
    let elsewhere = origin_of(&named);
    assert!(elsewhere.starts_with("http://127.0.0.1:"), "{challenge}");
    assert_ne!(elsewhere, moved.origin());
    assert_eq!(
        resource,
        json!({
            "resource": format!("{elsewhere}/documents"),
            "authorization_servers": [moved.issuer()],
            "scopes_supported": ["documents"],
            "bearer_methods_supported": ["header"],
        })
    );
    let documents_metadata = format!("{metadata_path}/documents");
    assert_eq!(
        paths(moved.requests_elsewhere()),
        [documents_metadata.as_str()]
    );
    let first = paths(moved.requests());
    assert!(
        first.iter().all(|path| !path.starts_with(metadata_path)),
        "{first:?}"
    );
}

#[test]
fn a_scripted_listing_of_related_resources_is_served_as_scripted() {
    let scripted = [
        json!([]),
        json!("x"),
        json!(["{origin}/notes"]),
        json!([{ "resource": "{origin}/notes" }]),
        json!([
            { "resource": "{origin}/notes", "kind": "notes" },
            { "resource": "{origin}/notes", "kind": "inbox" },
        ]),
        json!([{ "resource": "{origin}/documents", "kind": "documents" }]),
    ];
    for raw in scripted {
        let remote = Remote::start(RemoteScenario::accepting(KEY).listing(raw.clone()));
        let (resource, _) = metadata(&remote);
        let read = raw.to_string().replace("{origin}", remote.origin());
        let expected: Value = serde_json::from_str(&read).unwrap();
        assert_eq!(resource.get(RELATED_RESOURCES), Some(&expected), "{raw}");
    }
    let both = Remote::start(
        RemoteScenario::accepting(KEY)
            .relating(Related::new("notes", &["notes"]))
            .listing(json!([])),
    );
    let (resource, _) = metadata(&both);
    assert_eq!(resource.get(RELATED_RESOURCES), Some(&json!([])));
    assert_eq!(related_metadata(&both.related("notes")).status, 200);
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
    let lost = send_refresh(&server, &client, &resource, old);
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
fn a_lost_answer_is_repeated_for_its_resource_and_a_refresh_for_another_revokes_the_sign_in() {
    // A sign-in over the document's resource and `notes`, whose refresh for `notes` rotated its
    // refresh token and lost the answer.
    let lost = || {
        let notes = Related::new("notes", &["notes"]);
        let scenario = RemoteScenario::accepting(KEY).losing_a_refresh_answer();
        let remote = Remote::start(scenario.relating(notes));
        let (own, notes) = (remote.resource(), remote.related("notes"));
        let signed = signed_over(&remote, &[&own, &notes], "documents notes");
        let sent = send_refresh(
            &signed.server(),
            &signed.client,
            &notes,
            &signed.refresh_token,
        );
        assert!(sent.is_err(), "the answer is lost: {sent:?}");
        assert_eq!(remote.refreshes(), 1, "the token rotated");
        (remote, signed, own, notes)
    };
    let (remote, signed, own, notes) = lost();
    let repeated = signed.refresh(&notes, &signed.refresh_token);
    assert_eq!(repeated.status, 200, "{:?}", repeated.body);
    assert_eq!(repeated.body["resource"], json!(notes));
    let again = signed.refresh(&notes, &signed.refresh_token);
    assert_eq!(again.body, repeated.body);
    assert_eq!(remote.refreshes(), 1);
    let successor = repeated.body["refresh_token"].as_str().unwrap();
    assert_eq!(signed.refresh(&own, successor).status, 200);

    // Presented first for another resource of the sign-in, the token is reused.
    let (remote, signed, own, notes) = lost();
    for resource in [&own, &notes] {
        let answer = signed.refresh(resource, &signed.refresh_token);
        assert_oauth_error(&answer, "invalid_grant");
    }
    assert!(!remote.admits_at(&own, &signed.access_token));
}

#[test]
fn a_delaying_scenario_answers_each_refresh_only_after_its_delay() {
    let delay = Duration::from_millis(50);
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
