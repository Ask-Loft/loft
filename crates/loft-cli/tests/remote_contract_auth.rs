//! The contract suite of the remote simulator's authorization server: the two metadata
//! documents, registration, consent, the code exchange and the rotation of refresh tokens, as
//! OAuth 2.0 shapes them. The real half reads the metadata of the remote at
//! `LOFT_REMOTE_CONTRACT_ADDRESS` and registers a client there, and refreshes the sign-in that
//! `loft remote login` kept for that address in the `loft/remotes.json` that
//! `LOFT_REMOTE_CONTRACT_SIGN_IN` names, which the run spends: it ends revoked. It runs only on
//! demand.

use std::time::Duration;

use serde_json::{Value, json};
use software_plant::BrowserScenario;
use software_plant::remote::wire::value;
use software_plant::remote::{Companion, Remote, RemoteScenario};

use crate::common::Scratch;
use crate::common::oauth::{
    CALLBACK, KEY, REDIRECT, VERIFIER, assert_oauth_error, authorize, authorize_for, coded,
    discover, exchange, form, location, metadata, refresh, register, registered,
};
use crate::common::signed_in::{kept_sign_in, name_remote, run};

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

#[test]
fn a_public_client_with_loopback_redirects_is_registered_and_others_are_refused() {
    let remote = Remote::start(RemoteScenario::accepting(KEY));
    let (_, server) = metadata(&remote);
    let first = register(&server, &[REDIRECT], "none");
    let second = register(&server, &[REDIRECT], "none");
    assert_eq!(first.status, 201);
    assert_ne!(first.body["client_id"], second.body["client_id"]);
    assert_eq!(
        register(&server, &["http://example.com/callback"], "none").body["error"],
        "invalid_redirect_uri"
    );
    let secret = register(&server, &[REDIRECT], "client_secret_basic");
    assert_eq!(
        (secret.status, secret.body["error"].clone()),
        (400, json!("invalid_client_metadata"))
    );
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
    let plain = location(&authorize(
        &server,
        &client,
        resource,
        &[("code_challenge_method", "plain")],
    ));
    assert_eq!(value(&plain, "error"), Some("invalid_request"));
    let elsewhere = location(&authorize(
        &server,
        &client,
        resource,
        &[("resource", "https://elsewhere.example")],
    ));
    assert_eq!(value(&elsewhere, "error"), Some("invalid_target"));
    assert_eq!(authorize(&server, "client-0", resource, &[]).status, 400);
    let foreign = authorize(
        &server,
        &client,
        resource,
        &[("redirect_uri", "http://127.0.0.1:4567/other")],
    );
    assert_eq!(foreign.status, 400);
    let refusing = Remote::start(RemoteScenario::accepting(KEY).refusing_consent());
    let (resource, server) = metadata(&refusing);
    let resource = resource["resource"].as_str().unwrap();
    let client = registered(&server);
    let denied = location(&authorize(&server, &client, resource, &[]));
    assert_eq!(value(&denied, "error"), Some("access_denied"));
    assert_eq!(value(&denied, "state"), Some("the-state"));
}

#[test]
fn an_authorization_request_is_answered_for_the_first_step_it_fails_in_the_order_judged() {
    let notes = Companion::new("notes", &["notes"]);
    let remote = Remote::start(RemoteScenario::accepting(KEY).accompanied_by(notes.clone()));
    let (_, server) = metadata(&remote);
    let client = registered(&server);
    let own = remote.resource();
    let elsewhere = format!("{}/elsewhere", remote.origin());
    // A client that is not registered is answered without a redirect, whatever else it fails.
    let unregistered = authorize(&server, "client-0", &own, &[("response_type", "")]);
    assert_eq!(unregistered.status, 400);
    assert_eq!(unregistered.header("location"), None);
    // Each request fails a step and a later one, and is answered for the first.
    for (resource, failing, error) in [
        (
            &own,
            vec![
                ("response_type", "token"),
                ("code_challenge_method", "plain"),
            ],
            "unsupported_response_type",
        ),
        (
            &elsewhere,
            vec![("code_challenge_method", "plain")],
            "invalid_request",
        ),
        (&elsewhere, vec![("scope", "")], "invalid_target"),
        (&own, vec![("scope", "")], "invalid_scope"),
    ] {
        let answer = authorize(&server, &client, resource, &failing);
        let named = format!("{resource} with {failing:?}");
        assert_eq!(value(&location(&answer), "error"), Some(error), "{named}");
    }
    // The scopes asked of each resource are judged before the person's consent.
    let refusing = Remote::start(
        RemoteScenario::accepting(KEY)
            .accompanied_by(notes)
            .refusing_consent(),
    );
    let (_, server) = metadata(&refusing);
    let client = registered(&server);
    let asked = [refusing.resource(), refusing.companion("notes")];
    let asked = [asked[0].as_str(), asked[1].as_str()];
    let unasked = authorize_for(&server, &client, &asked, "documents");
    assert_eq!(value(&location(&unasked), "error"), Some("invalid_target"));
    let denied = authorize_for(&server, &client, &asked, "documents notes");
    assert_eq!(value(&location(&denied), "error"), Some("access_denied"));
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
    assert_eq!(answer.status, 200, "{:?}", answer.body);
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
    assert_ne!(rotated["refresh_token"], first["refresh_token"]);
    assert_ne!(rotated["access_token"], first["access_token"]);
    assert_eq!(refresh(&server, &client, &resource, old).body, rotated);
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
    assert_eq!(refresh(&server, &client, &resource, old).body, rotated);
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
    let (server, client, resource, code) = coded(&remote);
    let first = exchange(&server, &client, &resource, &code).body;
    let old = first["refresh_token"].as_str().unwrap();
    let rotated = refresh(&server, &client, &resource, old).body;
    let successor = rotated["refresh_token"].as_str().unwrap();
    let newest = refresh(&server, &client, &resource, successor).body;
    assert_eq!(
        refresh(&server, &client, &resource, old).body["error"],
        "invalid_grant"
    );
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
    let elsewhere = format!("{}/documents/elsewhere", remote.origin());
    assert!(kept_sign_in(&kept, &elsewhere).is_null());
}

#[test]
#[ignore = "spends the sign-in that LOFT_REMOTE_CONTRACT_SIGN_IN keeps"]
fn a_real_remote_answers_a_rotated_refresh_token_again_until_its_successor_is_presented() {
    let address = std::env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    let file = std::env::var("LOFT_REMOTE_CONTRACT_SIGN_IN").expect("a loft/remotes.json");
    let kept: Value = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
    let sign_in = kept_sign_in(&kept, &address);
    let server = json!({ "token_endpoint": sign_in["tokenEndpoint"] });
    let field = |name: &str| sign_in[name].as_str().expect(name).to_owned();
    let (client, resource, old) = (field("clientId"), field("resource"), field("refreshToken"));
    let rotated = refresh(&server, &client, &resource, &old);
    assert_eq!(rotated.status, 200, "{:?}", rotated.body);
    assert_eq!(
        refresh(&server, &client, &resource, &old).body,
        rotated.body,
        "within the grace window, the same answer"
    );
    let successor = rotated.body["refresh_token"].as_str().unwrap();
    assert_eq!(refresh(&server, &client, &resource, successor).status, 200);
    assert_eq!(
        refresh(&server, &client, &resource, &old).body["error"],
        "invalid_grant",
        "once the successor is presented"
    );
}
