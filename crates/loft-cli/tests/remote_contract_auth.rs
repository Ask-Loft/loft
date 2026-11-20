//! The contract suite of the remote simulator's authorization server: the two metadata
//! documents, registration, consent, the code exchange and the rotation of refresh tokens, as
//! OAuth 2.0 shapes them. An authorization request that the remote refuses before any person
//! consents has a suite of its own, `remote_contract_refusals.rs`.
//!
//! The real halves run only on demand. Two read the metadata of the remote at
//! `LOFT_REMOTE_CONTRACT_ADDRESS` and register clients there: one a client the remote registers,
//! and one each client that the contract refuses, showing a refused registration's status and
//! `error` alone, since a registration's body may hold a secret. The third refreshes the sign-in
//! that `loft remote login` kept for that address in the `loft/remotes.json` that
//! `LOFT_REMOTE_CONTRACT_SIGN_IN` names, which the run spends: it ends revoked. Each shares its
//! scenario with its simulator half, so that the two cannot drift. A scenario that needs a person
//! to consent, a code, or the simulator's clock runs against the simulator alone.

use std::time::Duration;

use serde_json::{Value, json};
use software_plant::BrowserScenario;
use software_plant::remote::wire::value;
use software_plant::remote::{Remote, RemoteScenario, document_path};

use crate::common::Scratch;
use crate::common::oauth::{
    CALLBACK, GRANTS, KEY, REDIRECT, VERIFIER, assert_oauth_error, assert_oauth_error_for,
    assert_same_answer, authorize, client_metadata, coded, discover, exchange, form, location,
    metadata, refresh, register, register_as, registered,
};
use crate::common::resources::{Signed, kept, signed_over};
use crate::common::signed_in::{kept_sign_in, name_remote, run};
use crate::remote_contract_refreshes::UNREFRESHED;

#[test]
fn the_metadata_names_the_resource_and_an_authorization_server_that_takes_pkce_alone() {
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let (resource, server) = metadata(&remote);
    assert!(resource["resource"].is_string());
    assert_eq!(
        resource["authorization_servers"].as_array().unwrap().len(),
        1
    );
    assert!(resource["scopes_supported"].is_array());
    assert!(
        resource["authorization_servers"][0]
            .as_str()
            .unwrap()
            .starts_with(remote.origin())
    );
    assert_eq!(server["issuer"], resource["authorization_servers"][0]);
    for field in [
        "authorization_endpoint",
        "token_endpoint",
        "registration_endpoint",
        "response_types_supported",
        "grant_types_supported",
    ] {
        assert!(server.get(field).is_some(), "{field}");
    }
    assert_eq!(server["code_challenge_methods_supported"], json!(["S256"]));
    assert_eq!(
        server["token_endpoint_auth_methods_supported"],
        json!(["none"])
    );
}

/// Refuses `400 Bad Request` each registration with `server`, the authorization server's
/// metadata, of a client that the contract refuses: one that redirects over `http` off the
/// loopback interface, `invalid_redirect_uri`; and one that authenticates at the token endpoint
/// with a secret, or uses other grants than `authorization_code` and `refresh_token`,
/// `invalid_client_metadata`. A failure names the metadata sent and shows the answer's status and
/// `error` alone: the body of a registration that a remote accepted may hold a secret.
fn refuses_a_client_it_cannot_register(server: &Value) {
    for (metadata, error) in [
        (
            client_metadata(&["http://example.com/callback"], "none", &GRANTS),
            "invalid_redirect_uri",
        ),
        (
            client_metadata(&[REDIRECT], "client_secret_basic", &GRANTS),
            "invalid_client_metadata",
        ),
        (
            client_metadata(&[REDIRECT], "none", &["authorization_code"]),
            "invalid_client_metadata",
        ),
    ] {
        let answer = register_as(server, &metadata);
        let (status, answered) = (answer.status, &answer.body["error"]);
        assert!(
            status == 400 && answered == error,
            "registering {metadata}: `400 Bad Request` with the OAuth error `{error}`, not \
             {status} with `error` {answered}"
        );
    }
}

#[test]
fn a_public_client_with_loopback_redirects_is_registered_and_others_are_refused() {
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let (_, server) = metadata(&remote);
    let first = register(&server, &[REDIRECT], "none");
    let second = register(&server, &[REDIRECT], "none");
    assert_eq!(first.status, 201);
    assert_ne!(first.body["client_id"], second.body["client_id"]);
    refuses_a_client_it_cannot_register(&server);
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_refuses_to_register_a_redirect_over_http_off_the_loopback_interface_a_secret_or_other_grants()
 {
    // Registers each client the contract refuses: a remote that registers one anyway keeps a
    // client nobody uses.
    let address = std::env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    let (_, server) = discover(&address);
    refuses_a_client_it_cannot_register(&server);
}

#[test]
fn an_authorization_request_is_answered_as_the_person_consents() {
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let (resource, server) = metadata(&remote);
    let resource = resource["resource"].as_str().unwrap();
    let client = registered(&server);
    let granted = location(&authorize(&server, &client, resource, &[]));
    assert!(value(&granted, "code").is_some());
    assert_eq!(value(&granted, "state"), Some("the-state"));
    let refusing = Remote::start(RemoteScenario::accepting(KEY).refusing_consent());
    let (resource, server) = metadata(&refusing);
    let resource = resource["resource"].as_str().unwrap();
    let client = registered(&server);
    let denied = location(&authorize(&server, &client, resource, &[]));
    assert_eq!(value(&denied, "error"), Some("access_denied"));
    assert_eq!(value(&denied, "state"), Some("the-state"));
}

#[test]
fn a_code_is_exchanged_once_for_tokens_with_its_verifier_redirect_and_resource() {
    let remote = Remote::start(RemoteScenario::accepting(KEY).expiring_in(600));
    let (server, client, resource, code) = coded(&remote);
    let token_url = server["token_endpoint"].as_str().unwrap();
    let presented = |redirect_uri: &str, verifier: &str| {
        form(
            token_url,
            &[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", redirect_uri),
                ("client_id", &client),
                ("code_verifier", verifier),
                ("resource", &resource),
            ],
        )
    };
    assert_oauth_error(&presented(CALLBACK, "wrong"), "invalid_grant");
    // A loopback redirect on another port matches the registered one, but is not the one the
    // authorization request named.
    let another_port = "http://127.0.0.1:4568/callback";
    assert_oauth_error(&presented(another_port, VERIFIER), "invalid_grant");
    let wrong_resource = exchange(&server, &client, "https://elsewhere.example", &code);
    assert_eq!(wrong_resource.body["error"], "invalid_target");
    let answer = exchange(&server, &client, &resource, &code);
    assert_eq!(answer.status, 200, "{answer:?}");
    assert_eq!(answer.body["token_type"], "Bearer");
    assert_eq!(answer.body["expires_in"], 600);
    for field in ["access_token", "refresh_token", "scope"] {
        assert!(answer.body[field].is_string(), "{field}");
    }
    assert!(remote.admits(answer.body["access_token"].as_str().unwrap()));
    let again = exchange(&server, &client, &resource, &code);
    assert_oauth_error(&again, "invalid_grant");
}

#[test]
fn a_refresh_rotates_and_a_rotated_token_is_answered_again_only_within_the_grace() {
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let (server, client, resource, code) = coded(&remote);
    let first = exchange(&server, &client, &resource, &code).body;
    let old = first["refresh_token"].as_str().unwrap();
    let rotated = refresh(&server, &client, &resource, old).body;
    for token in ["refresh_token", "access_token"] {
        let renewed = rotated[token].is_string() && rotated[token] != first[token];
        assert!(renewed, "a refresh issues another `{token}`");
    }
    let repeated = refresh(&server, &client, &resource, old).body;
    assert_same_answer(&repeated, &rotated, "a repeat within the grace window");
    assert_eq!(remote.refreshes(), 1);
    let strict = Remote::start(RemoteScenario::accepting(KEY).without_grace());
    let (server, client, resource, code) = coded(&strict);
    let first = exchange(&server, &client, &resource, &code).body;
    let old = first["refresh_token"].as_str().unwrap();
    let rotated = refresh(&server, &client, &resource, old).body;
    assert_eq!(
        refresh(&server, &client, &resource, old).body["error"],
        "invalid_grant"
    );
    assert!(!strict.admits(rotated["access_token"].as_str().unwrap()));
    let newest = rotated["refresh_token"].as_str().unwrap();
    assert_eq!(
        refresh(&server, &client, &resource, newest).body["error"],
        "invalid_grant"
    );
    let lapsed = Remote::start(RemoteScenario::accepting(KEY).lapsing());
    let (server, client, resource, code) = coded(&lapsed);
    let first = exchange(&server, &client, &resource, &code).body;
    let answer = refresh(
        &server,
        &client,
        &resource,
        first["refresh_token"].as_str().unwrap(),
    );
    assert_eq!(
        (answer.status, answer.body["error"].clone()),
        (400, json!("invalid_grant"))
    );
}

#[test]
fn a_rotated_refresh_token_is_answered_again_for_120_seconds_and_until_its_successor_is_presented()
{
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let (server, client, resource, code) = coded(&remote);
    let first = exchange(&server, &client, &resource, &code).body;
    let old = first["refresh_token"].as_str().unwrap();
    let rotated = refresh(&server, &client, &resource, old).body;
    remote.advance(Duration::from_secs(119));
    let repeated = refresh(&server, &client, &resource, old).body;
    assert_same_answer(&repeated, &rotated, "a repeat 119 seconds on");
    assert_eq!(remote.refreshes(), 1);
    assert!(remote.admits(rotated["access_token"].as_str().unwrap()));
    remote.advance(Duration::from_secs(2));
    assert_eq!(
        refresh(&server, &client, &resource, old).body["error"],
        "invalid_grant"
    );
    assert!(!remote.admits(rotated["access_token"].as_str().unwrap()));

    // Within the window, a repeat after the successor was itself presented revokes the sign-in.
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let own = remote.resource();
    let signed = signed_over(&remote, &[&own], "documents");
    let newest = answers_a_rotated_refresh_token_again_until_its_successor_is_presented(&signed);
    assert!(!remote.admits(newest["access_token"].as_str().unwrap()));

    // A scenario names another length.
    let remote = Remote::start(RemoteScenario::accepting(KEY).grace(Duration::from_secs(10)));
    let (server, client, resource, code) = coded(&remote);
    let first = exchange(&server, &client, &resource, &code).body;
    let old = first["refresh_token"].as_str().unwrap();
    refresh(&server, &client, &resource, old);
    remote.advance(Duration::from_secs(11));
    assert_eq!(
        refresh(&server, &client, &resource, old).body["error"],
        "invalid_grant"
    );
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_s_authorization_server_answers_in_the_shape_the_simulator_does() {
    let address = std::env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    let (real_resource, real_server) = discover(&address);
    let issuer = real_resource["authorization_servers"][0].as_str().unwrap();
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let (resource, server) = metadata(&remote);
    let keys = |value: &Value, fields: &[&str]| {
        fields
            .iter()
            .map(|f| value.get(*f).is_some())
            .collect::<Vec<_>>()
    };
    let resource_fields = ["resource", "authorization_servers"];
    assert_eq!(
        keys(&real_resource, &resource_fields),
        keys(&resource, &resource_fields)
    );
    let server_fields = [
        "issuer",
        "authorization_endpoint",
        "token_endpoint",
        "registration_endpoint",
        "code_challenge_methods_supported",
    ];
    assert_eq!(
        keys(&real_server, &server_fields),
        keys(&server, &server_fields)
    );
    assert_eq!(real_server["issuer"], json!(issuer));
    assert!(
        real_server["code_challenge_methods_supported"]
            .as_array()
            .unwrap()
            .contains(&json!("S256"))
    );
    let real = register(&real_server, &[REDIRECT], "none");
    let simulated = register(&server, &[REDIRECT], "none");
    assert_eq!(
        (real.status, real.body["client_id"].is_string()),
        (simulated.status, simulated.body["client_id"].is_string())
    );
}

#[test]
fn a_sign_in_that_login_keeps_is_found_by_its_address_and_refreshes() {
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(&remote.address()), true);
    let (output, _) = run(
        &scratch,
        &["remote", "login", "--format", "json"],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let kept = scratch.setting("remotes.json");
    let sign_in = kept_sign_in(&kept, &remote.address());
    let server = json!({ "token_endpoint": sign_in["tokenEndpoint"] });
    let field = |name: &str| sign_in[name].as_str().expect(name).to_owned();
    let (client, resource, old) = (field("clientId"), field("resource"), field("refreshToken"));
    assert_eq!(refresh(&server, &client, &resource, &old).status, 200);
    assert_eq!(remote.refreshes(), 1);
    let elsewhere = format!("{}{}", remote.origin(), document_path("elsewhere"));
    assert!(kept_sign_in(&kept, &elsewhere).is_null());
}

/// Rotates the refresh token of `signed` for the sign-in's own resource, and presents it again
/// for that resource: within the grace window it gets the same answer, until its successor has
/// itself been presented, after which it is refused `invalid_grant`. Gives the answer to the
/// successor.
fn answers_a_rotated_refresh_token_again_until_its_successor_is_presented(
    signed: &Signed,
) -> Value {
    let (own, old) = (signed.resource.as_str(), signed.refresh_token.as_str());
    let rotated = signed.refresh(own, old);
    assert_eq!(rotated.status, 200, "{UNREFRESHED}: {rotated:?}");
    let repeated = signed.refresh(own, old).body;
    let within = "within the grace window, the same answer";
    assert_same_answer(&repeated, &rotated.body, within);
    let successor = rotated.body["refresh_token"].as_str();
    let successor = successor.unwrap_or_else(|| panic!("a successor: {rotated:?}"));
    let next = signed.refresh(own, successor);
    assert_eq!(next.status, 200, "the successor refreshes: {next:?}");
    let again = signed.refresh(own, old);
    let presented = "the rotated refresh token, presented again once its successor is presented";
    assert_oauth_error_for(&again, "invalid_grant", presented);
    next.body
}

#[test]
#[ignore = "spends the sign-in that LOFT_REMOTE_CONTRACT_SIGN_IN keeps"]
fn a_real_remote_answers_a_rotated_refresh_token_again_until_its_successor_is_presented() {
    // Needs a sign-in that any `loft remote login` kept for the address, which nothing has
    // refreshed since. The run revokes it.
    answers_a_rotated_refresh_token_again_until_its_successor_is_presented(&kept());
}
