//! The contract suite of the remote simulator's authorization server: the two metadata
//! documents, registration, consent, the code exchange and the rotation of refresh tokens, as
//! OAuth 2.0 shapes them. The real half reads the metadata of the remote at
//! `LOFT_REMOTE_CONTRACT_ADDRESS` and registers a client there, and refreshes the sign-in that
//! `loft remote login` kept for that address in the `loft/remotes.json` that
//! `LOFT_REMOTE_CONTRACT_SIGN_IN` names, which the run spends: it ends revoked. It runs only on
//! demand.

use std::time::Duration;

use serde_json::{Value, json};
use software_plant::remote::wire::{base64url, pairs, query, sha256, value};
use software_plant::remote::{Remote, RemoteScenario};

use ureq::Body;
use ureq::http::Response;

use crate::remote_contract::{Answer, agent, call, read};

const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
pub const KEY: &str = "01a18e30-0000-7000-8000-000000000010";
const REDIRECT: &str = "http://127.0.0.1/callback";

pub fn metadata(remote: &Remote) -> (Value, Value) {
    discover(&remote.address())
}

/// The protected resource metadata and the authorization server metadata of the document at
/// `address`, found from the challenge of a request without a token.
fn discover(address: &str) -> (Value, Value) {
    let (challenge, resource, server) = fetch(address);
    let metadata = challenge.split('"').nth(1).unwrap();
    let named = metadata.replacen("/.well-known/oauth-protected-resource", "", 1);
    assert_eq!(resource["resource"], named.as_str(), "RFC 9728 3.3");
    (resource, server)
}

/// The challenge of a request without a token to the document at `address`, and the two
/// metadata documents it leads to, with no check of what they say.
pub fn fetch(address: &str) -> (String, Value, Value) {
    let challenge = call("GET", &format!("{address}/head"), None, None);
    let challenge = challenge
        .header("www-authenticate")
        .expect("a challenge")
        .to_owned();
    let metadata = challenge.split('"').nth(1).unwrap();
    let resource = call("GET", metadata, None, None).body;
    let issuer = resource["authorization_servers"][0].as_str().unwrap();
    let authority = issuer.find("://").unwrap() + 3;
    let (origin, path) = issuer.split_at(
        issuer[authority..]
            .find('/')
            .map_or(issuer.len(), |at| authority + at),
    );
    let server = call(
        "GET",
        &format!("{origin}/.well-known/oauth-authorization-server{path}"),
        None,
        None,
    )
    .body;
    (challenge, resource, server)
}

/// The id of a public client registered with `server`.
pub fn registered(server: &Value) -> String {
    register(server, &[REDIRECT], "none").body["client_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

pub fn register(server: &Value, redirects: &[&str], method: &str) -> Answer {
    let body = json!({
        "client_name": "loft",
        "redirect_uris": redirects,
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "token_endpoint_auth_method": method,
    });
    call(
        "POST",
        server["registration_endpoint"].as_str().unwrap(),
        None,
        Some(&body),
    )
}

/// The query of an authorization request, with `overrides` replacing its parameters.
fn authorize(server: &Value, client: &str, resource: &str, overrides: &[(&str, &str)]) -> Answer {
    let challenge = base64url(&sha256(VERIFIER.as_bytes()));
    let mut parameters = [
        ("response_type", "code".to_owned()),
        ("client_id", client.to_owned()),
        ("redirect_uri", "http://127.0.0.1:4567/callback".to_owned()),
        ("scope", "documents".to_owned()),
        ("resource", resource.to_owned()),
        ("state", "the-state".to_owned()),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256".to_owned()),
    ];
    for (name, value) in overrides {
        if let Some(entry) = parameters.iter_mut().find(|(field, _)| field == name) {
            (*value).clone_into(&mut entry.1);
        }
    }
    let parameters: Vec<(&str, &str)> = parameters
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let url = format!(
        "{}?{}",
        server["authorization_endpoint"].as_str().unwrap(),
        query(&parameters)
    );
    call("GET", &url, None, None)
}

/// The parameters of a redirect's `Location`.
fn location(answer: &Answer) -> Vec<(String, String)> {
    assert_eq!(answer.status, 302, "{:?}", answer.body);
    let location = answer.header("location").unwrap();
    assert!(
        location.starts_with("http://127.0.0.1:4567/callback?"),
        "{location}"
    );
    pairs(location.split_once('?').unwrap().1)
}

pub fn form(url: &str, fields: &[(&str, &str)]) -> Answer {
    read(send_form(url, fields).unwrap())
}

/// The response to a form of `fields` posted to `url`, or the error that kept it from coming.
fn send_form(url: &str, fields: &[(&str, &str)]) -> Result<Response<Body>, ureq::Error> {
    agent()
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(query(fields))
}

/// A client registered on `remote` and a code it was given, with the endpoints.
pub fn coded(remote: &Remote) -> (Value, String, String, String) {
    let (resource, server) = metadata(remote);
    let client = registered(&server);
    let resource = resource["resource"].as_str().unwrap().to_owned();
    let code = value(
        &location(&authorize(&server, &client, &resource, &[])),
        "code",
    )
    .unwrap()
    .to_owned();
    (server, client, resource, code)
}

pub fn exchange(server: &Value, client: &str, resource: &str, code: &str) -> Answer {
    form(
        server["token_endpoint"].as_str().unwrap(),
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", "http://127.0.0.1:4567/callback"),
            ("client_id", client),
            ("code_verifier", VERIFIER),
            ("resource", resource),
        ],
    )
}

pub fn refresh(server: &Value, client: &str, resource: &str, token: &str) -> Answer {
    read(send_refresh(server, client, resource, token).unwrap())
}

/// The response to a refresh of `token`, or the error that kept it from coming.
pub fn send_refresh(
    server: &Value,
    client: &str,
    resource: &str,
    token: &str,
) -> Result<Response<Body>, ureq::Error> {
    send_form(
        server["token_endpoint"].as_str().unwrap(),
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", token),
            ("client_id", client),
            ("resource", resource),
        ],
    )
}

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
fn a_code_is_exchanged_once_for_tokens_with_its_verifier_redirect_and_resource() {
    let remote = Remote::start(RemoteScenario::accepting(KEY).expiring_in(600));
    let (server, client, resource, code) = coded(&remote);
    let token_url = server["token_endpoint"].as_str().unwrap();
    let wrong_verifier = form(
        token_url,
        &[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", "http://127.0.0.1:4567/callback"),
            ("client_id", &client),
            ("code_verifier", "wrong"),
            ("resource", &resource),
        ],
    );
    assert_eq!(wrong_verifier.body["error"], "invalid_grant");
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
    assert_eq!(
        exchange(&server, &client, &resource, &code).body["error"],
        "invalid_grant"
    );
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
#[ignore = "spends the sign-in that LOFT_REMOTE_CONTRACT_SIGN_IN keeps"]
fn a_real_remote_answers_a_rotated_refresh_token_again_until_its_successor_is_presented() {
    let address = std::env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    let file = std::env::var("LOFT_REMOTE_CONTRACT_SIGN_IN").expect("a loft/remotes.json");
    let kept: Value = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
    let sign_in = &kept[address.as_str()];
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
