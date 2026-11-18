//! The calls a contract suite makes of a remote's authorization server, as OAuth 2.0 shapes them,
//! of the simulator and of a real remote alike: the two metadata documents found from the
//! challenge of a request without a token, the registration of a public client, an authorization
//! request with PKCE and the redirect that answers it, the exchange of its code, and a refresh.

use serde_json::{Value, json};
use software_plant::remote::Remote;
use software_plant::remote::wire::{base64url, pairs, query, sha256, value};

use ureq::Body;
use ureq::http::Response;

use super::calls::{Answer, agent, call, read};

/// The code verifier of every authorization request, whose challenge the request sends.
const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
/// The document key of each remote that the suites of the authorization server start.
pub const KEY: &str = "01a18e30-0000-7000-8000-000000000010";
/// The redirect URI a client registers, on the loopback interface.
pub const REDIRECT: &str = "http://127.0.0.1/callback";

/// The protected resource metadata and the authorization server metadata of `remote`'s
/// document.
pub fn metadata(remote: &Remote) -> (Value, Value) {
    discover(&remote.address())
}

/// The protected resource metadata and the authorization server metadata of the document at
/// `address`, found from the challenge of a request without a token.
pub fn discover(address: &str) -> (Value, Value) {
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

/// The answer to a registration with `server` of a client that redirects to `redirects` and
/// authenticates at the token endpoint by `method`.
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
pub fn authorize(
    server: &Value,
    client: &str,
    resource: &str,
    overrides: &[(&str, &str)],
) -> Answer {
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
pub fn location(answer: &Answer) -> Vec<(String, String)> {
    assert_eq!(answer.status, 302, "{:?}", answer.body);
    let location = answer.header("location").unwrap();
    assert!(
        location.starts_with("http://127.0.0.1:4567/callback?"),
        "{location}"
    );
    pairs(location.split_once('?').unwrap().1)
}

/// The answer to a form of `fields` posted to `url`.
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

/// The answer to an exchange of `code` for tokens, with the verifier and the redirect it was
/// given for.
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

/// The answer to a refresh of `token`.
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
