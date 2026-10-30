//! The contract suite of the remote simulator's authorization server: the two metadata
//! documents, registration, consent, the code exchange and the rotation of refresh tokens, as
//! OAuth 2.0 shapes them. The real half reads the metadata of the remote at
//! `LOFT_REMOTE_CONTRACT_ADDRESS` and registers a client there, and runs only on demand.

use serde_json::{Value, json};
use software_plant::remote::wire::{base64url, encode, pairs, sha256, value};
use software_plant::remote::{Remote, RemoteScenario};

use crate::remote_contract::{Answer, agent, call};

const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
const KEY: &str = "01a18e30-0000-7000-8000-000000000010";
const REDIRECT: &str = "http://127.0.0.1/callback";

fn metadata(remote: &Remote) -> (Value, Value) {
    let challenge = call("GET", &format!("{}/head", remote.address()), None, None);
    let header = challenge.header("www-authenticate").unwrap().to_owned();
    let resource_url = header.split('"').nth(1).unwrap();
    let resource = call("GET", resource_url, None, None).body;
    let issuer = resource["authorization_servers"][0].as_str().unwrap();
    let path = issuer.strip_prefix(remote.origin()).unwrap();
    let server = call(
        "GET",
        &format!(
            "{}/.well-known/oauth-authorization-server{path}",
            remote.origin()
        ),
        None,
        None,
    )
    .body;
    (resource, server)
}

fn register(server: &Value, redirects: &[&str], method: &str) -> Answer {
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
    let query: Vec<String> = parameters
        .iter()
        .map(|(name, value)| format!("{name}={}", encode(value)))
        .collect();
    let url = format!(
        "{}?{}",
        server["authorization_endpoint"].as_str().unwrap(),
        query.join("&")
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

fn form(url: &str, fields: &[(&str, &str)]) -> Answer {
    let body: Vec<String> = fields
        .iter()
        .map(|(n, v)| format!("{n}={}", encode(v)))
        .collect();
    let mut response = agent()
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(body.join("&"))
        .unwrap();
    let bytes = response.body_mut().read_to_vec().unwrap_or_default();
    Answer {
        status: response.status().as_u16(),
        headers: std::collections::BTreeMap::new(),
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

/// A client registered on `remote` and a code it was given, with the endpoints.
fn coded(remote: &Remote) -> (Value, String, String, String) {
    let (resource, server) = metadata(remote);
    let client = register(&server, &[REDIRECT], "none").body["client_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let resource = resource["resource"].as_str().unwrap().to_owned();
    let code = value(
        &location(&authorize(&server, &client, &resource, &[])),
        "code",
    )
    .unwrap()
    .to_owned();
    (server, client, resource, code)
}

fn exchange(server: &Value, client: &str, resource: &str, code: &str) -> Answer {
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

fn refresh(server: &Value, client: &str, resource: &str, token: &str) -> Answer {
    form(
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
    let client = register(&server, &[REDIRECT], "none").body["client_id"]
        .as_str()
        .unwrap()
        .to_owned();
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
    let client = register(&server, &[REDIRECT], "none").body["client_id"]
        .as_str()
        .unwrap()
        .to_owned();
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
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_s_authorization_server_answers_in_the_shape_the_simulator_does() {
    let address = std::env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    let real_challenge = call("GET", &format!("{address}/head"), None, None);
    let header = real_challenge
        .header("www-authenticate")
        .expect("a challenge")
        .to_owned();
    let real_resource = call("GET", header.split('"').nth(1).unwrap(), None, None).body;
    let issuer = real_resource["authorization_servers"][0].as_str().unwrap();
    let (origin, path) = issuer.split_at(
        issuer.find("://").unwrap()
            + 3
            + issuer[issuer.find("://").unwrap() + 3..]
                .find('/')
                .unwrap_or(issuer.len()),
    );
    let real_server = call(
        "GET",
        &format!("{origin}/.well-known/oauth-authorization-server{path}"),
        None,
        None,
    )
    .body;
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
