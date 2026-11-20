//! The calls a contract suite makes of a remote's authorization server, as OAuth 2.0 shapes them,
//! of the simulator and of a real remote alike: the two metadata documents found from the
//! challenge of a request without a token, the parameters of that challenge, the metadata of a
//! companion resource, the registration of a public client, an authorization request with PKCE
//! for one protected resource or more and the redirect that answers it, the exchange of its code,
//! a refresh that names one resource or another number of them, the OAuth error answer that
//! refuses one, a token answer given again, compared without showing its tokens, a resource that
//! no remote serves and a code that no remote issued. A step that finds no member it needs fails
//! naming the member and what it found instead, never a token or a secret.

use serde_json::{Value, json};
use software_plant::remote::Remote;
use software_plant::remote::wire::{base64url, pairs, query, sha256, value};

use ureq::Body;
use ureq::http::Response;

use super::calls::{Answer, TOKENS, agent, call, read, redacted};

/// The code verifier of every authorization request, whose challenge the request sends.
pub const VERIFIER: &str = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
/// The document key of each remote that the suites of the authorization server start.
pub const KEY: &str = "01a18e30-0000-7000-8000-000000000010";
/// The redirect URI a client registers, on the loopback interface.
pub const REDIRECT: &str = "http://127.0.0.1/callback";
/// The redirect URI of every authorization request and exchange: the registered one, on a port
/// of its own.
pub const CALLBACK: &str = "http://127.0.0.1:4567/callback";
/// A protected resource that no remote serves: one on an origin under `.invalid`, which RFC 6761
/// reserves so that it never resolves, and so is no remote's origin.
pub const UNSERVED: &str = "https://elsewhere.invalid/documents";
/// A code that no remote issued.
pub const UNKNOWN_CODE: &str = "a-code-never-issued";

/// The protected resource metadata and the authorization server metadata of `remote`'s
/// document.
pub fn metadata(remote: &Remote) -> (Value, Value) {
    discover(&remote.address())
}

/// The protected resource metadata and the authorization server metadata of the document at
/// `address`, found from the challenge of a request without a token.
pub fn discover(address: &str) -> (Value, Value) {
    let (challenge, resource, server) = fetch(address);
    let metadata = metadata_named(&challenge);
    let named = metadata.replacen("/.well-known/oauth-protected-resource", "", 1);
    let rule = format!("RFC 9728 3.3: the metadata at {metadata} names its resource: {resource}");
    assert_eq!(resource["resource"], named.as_str(), "{rule}");
    (resource, server)
}

/// The challenge of a request without a token to the document at `address`, and the two
/// metadata documents it leads to, with no check of what they say.
pub fn fetch(address: &str) -> (String, Value, Value) {
    let challenge = challenge_at(address);
    let resource = call("GET", &metadata_named(&challenge), None, None).body;
    let issuer = resource["authorization_servers"][0]
        .as_str()
        .unwrap_or_else(|| {
            panic!("protected resource metadata that names an authorization server: {resource}")
        });
    let authority = issuer
        .find("://")
        .unwrap_or_else(|| panic!("an issuer that is an absolute URL: {issuer}"))
        + 3;
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

/// The challenge of a request without a token to the document at `address`.
pub fn challenge_at(address: &str) -> String {
    let answer = call("GET", &format!("{address}/head"), None, None);
    let challenge = answer.header("www-authenticate");
    challenge
        .unwrap_or_else(|| panic!("a challenge: {answer:?}"))
        .to_owned()
}

/// The address of the protected resource metadata that `challenge` names in `resource_metadata`.
fn metadata_named(challenge: &str) -> String {
    parameter(challenge, "resource_metadata")
        .unwrap_or_else(|| panic!("a challenge that names `resource_metadata`: {challenge}"))
}

/// The endpoint `name` of the authorization server whose metadata is `server`.
fn endpoint<'a>(server: &'a Value, name: &str) -> &'a str {
    server[name]
        .as_str()
        .unwrap_or_else(|| panic!("authorization server metadata that names `{name}`: {server}"))
}

/// The value of the parameter `name` of the `Bearer` challenge `challenge`, if it names one as a
/// quoted string.
pub fn parameter(challenge: &str, name: &str) -> Option<String> {
    let quoted = format!("{name}=\"");
    let start = challenge
        .match_indices(&quoted)
        .map(|(at, _)| at)
        .find(|at| matches!(challenge[..*at].chars().last(), Some(' ' | ',')))?
        + quoted.len();
    let length = challenge[start..].find('"')?;
    Some(challenge[start..start + length].to_owned())
}

/// The answer to a request without a token for the protected resource metadata of the resource
/// `identifier`, at the address RFC 9728 derives from it: its origin, then
/// `/.well-known/oauth-protected-resource`, then its path without a terminating `/`, then its
/// query.
pub fn companion_metadata(identifier: &str) -> Answer {
    let authority = identifier.find("://").map_or(0, |at| at + 3);
    let end = identifier[authority..]
        .find(['/', '?'])
        .map_or(identifier.len(), |at| authority + at);
    let (origin, rest) = identifier.split_at(end);
    let (path, query) = rest.find('?').map_or((rest, ""), |at| rest.split_at(at));
    let path = path.trim_end_matches('/');
    let url = format!("{origin}/.well-known/oauth-protected-resource{path}{query}");
    call("GET", &url, None, None)
}

/// The id of a public client registered with `server`. A registration answered without one fails
/// showing its status and its `error` alone: the body of a registration may hold a secret.
pub fn registered(server: &Value) -> String {
    let answer = register(server, &[REDIRECT], "none");
    let id = answer.body["client_id"].as_str().map(str::to_owned);
    id.unwrap_or_else(|| {
        let (status, error) = (answer.status, &answer.body["error"]);
        panic!("a public client registered with a `client_id`, not {status} with `error` {error}")
    })
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
    let registration = endpoint(server, "registration_endpoint");
    call("POST", registration, None, Some(&body))
}

/// The query of an authorization request, with `overrides` replacing its parameters.
pub fn authorize(
    server: &Value,
    client: &str,
    resource: &str,
    overrides: &[(&str, &str)],
) -> Answer {
    authorized(server, client, &[resource], "documents", overrides)
}

/// The answer to an authorization request of `client` that names `resource` once for each of
/// `resources`, none, one or more, and asks for `scope`.
pub fn authorize_for(server: &Value, client: &str, resources: &[&str], scope: &str) -> Answer {
    authorized(server, client, resources, scope, &[])
}

/// The answer to an authorization request of `client` for `resources` and `scope`, with
/// `overrides` replacing the first parameter of each name.
pub fn authorized(
    server: &Value,
    client: &str,
    resources: &[&str],
    scope: &str,
    overrides: &[(&str, &str)],
) -> Answer {
    let challenge = base64url(&sha256(VERIFIER.as_bytes()));
    let mut parameters = vec![
        ("response_type", "code"),
        ("client_id", client),
        ("redirect_uri", CALLBACK),
        ("scope", scope),
    ];
    parameters.extend(resources.iter().map(|resource| ("resource", *resource)));
    parameters.extend([
        ("state", "the-state"),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
    ]);
    for (name, value) in overrides {
        if let Some(entry) = parameters.iter_mut().find(|(field, _)| field == name) {
            entry.1 = *value;
        }
    }
    let authorization = endpoint(server, "authorization_endpoint");
    let url = format!("{authorization}?{}", query(&parameters));
    call("GET", &url, None, None)
}

/// The parameters of a redirect's `Location`, which leads back to the client.
pub fn location(answer: &Answer) -> Vec<(String, String)> {
    assert_eq!(answer.status, 302, "a redirect: {answer:?}");
    let location = answer.header("location").unwrap_or_default();
    let back = location.starts_with(&format!("{CALLBACK}?"));
    assert!(back, "a redirect back to the client: {answer:?}");
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

/// A client registered on `remote` and the code it was given for an authorization request that
/// names each of `resources` and asks for `scope`, with the authorization server's metadata.
pub fn coded_for(remote: &Remote, resources: &[&str], scope: &str) -> (Value, String, String) {
    let (_, server) = metadata(remote);
    let client = registered(&server);
    let answer = authorize_for(&server, &client, resources, scope);
    let code = value(&location(&answer), "code")
        .expect("a code")
        .to_owned();
    (server, client, code)
}

/// The answer to an exchange of `code` for tokens, with the verifier and the redirect it was
/// given for.
pub fn exchange(server: &Value, client: &str, resource: &str, code: &str) -> Answer {
    exchange_for(server, client, &[resource], code)
}

/// The answer to an exchange of `code` for tokens, with the verifier and the redirect it was
/// given for, that names `resource` once for each of `resources`, none, one or more.
pub fn exchange_for(server: &Value, client: &str, resources: &[&str], code: &str) -> Answer {
    let mut fields = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", CALLBACK),
        ("client_id", client),
        ("code_verifier", VERIFIER),
    ];
    fields.extend(resources.iter().map(|resource| ("resource", *resource)));
    form(endpoint(server, "token_endpoint"), &fields)
}

/// Requires `answer` to be an OAuth 2.0 error answer of `error` at `400 Bad Request`.
#[track_caller]
pub fn assert_oauth_error(answer: &Answer, error: &str) {
    assert_oauth_error_for(answer, error, "the request");
}

/// Requires `answer`, to the request that `asked` describes, to be an OAuth 2.0 error answer of
/// `error` at `400 Bad Request`. A failure names the request, which `asked` describes without a
/// code or a token, and shows the answer with every token hidden.
#[track_caller]
pub fn assert_oauth_error_for(answer: &Answer, error: &str, asked: &str) {
    assert_eq!(
        (answer.status, &answer.body["error"]),
        (400, &json!(error)),
        "{asked}: {answer:?}"
    );
}

/// Requires `repeat`, the body of an answer to a token request, to be `answer` again, member for
/// member. A failure shows `what` and the two bodies with every token hidden, never a token: the
/// members are compared with the tokens hidden first, then each token without being shown.
pub fn assert_same_answer(repeat: &Value, answer: &Value, what: &str) {
    assert_eq!(redacted(repeat), redacted(answer), "{what}");
    for member in TOKENS {
        let same = repeat[member] == answer[member];
        assert!(same, "{what}: the same `{member}`");
    }
    assert!(repeat == answer, "{what}: the same tokens at every depth");
}

/// The answer to a refresh of `token`.
pub fn refresh(server: &Value, client: &str, resource: &str, token: &str) -> Answer {
    refresh_for(server, client, &[resource], token)
}

/// The answer to a refresh of `token` that names `resource` once for each of `resources`, none,
/// one or more.
pub fn refresh_for(server: &Value, client: &str, resources: &[&str], token: &str) -> Answer {
    read(send_refresh_for(server, client, resources, token).unwrap())
}

/// The response to a refresh of `token`, or the error that kept it from coming.
pub fn send_refresh(
    server: &Value,
    client: &str,
    resource: &str,
    token: &str,
) -> Result<Response<Body>, ureq::Error> {
    send_refresh_for(server, client, &[resource], token)
}

/// The response to a refresh of `token` that names `resource` once for each of `resources`, or
/// the error that kept it from coming.
fn send_refresh_for(
    server: &Value,
    client: &str,
    resources: &[&str],
    token: &str,
) -> Result<Response<Body>, ureq::Error> {
    let mut fields = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", token),
        ("client_id", client),
    ];
    fields.extend(resources.iter().map(|resource| ("resource", *resource)));
    send_form(endpoint(server, "token_endpoint"), &fields)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::panic::catch_unwind;

    use serde_json::json;

    use super::{Answer, assert_oauth_error_for, assert_same_answer};

    #[test]
    fn an_answer_that_is_not_the_refusal_expected_fails_naming_its_request_without_a_token() {
        let accepted = Answer {
            status: 200,
            headers: BTreeMap::new(),
            body: json!({ "access_token": "a-live-access-token", "scope": "documents" }),
        };
        let asked = "a refresh of the live refresh token that names [\"notes\", \"notes\"]";
        let failed = catch_unwind(|| assert_oauth_error_for(&accepted, "invalid_target", asked));
        let failure = failed.expect_err("an accepted request is no refusal");
        let message = failure
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_default();
        let named = message.contains(asked) && message.contains("invalid_target");
        assert!(
            named && !message.contains("a-live-access-token"),
            "{message}"
        );
    }

    #[test]
    fn two_token_answers_that_differ_only_in_a_token_are_told_apart_without_showing_it() {
        let answer = json!({
            "access_token": "a-live-access-token",
            "granted": { "refresh_token": "a-nested-refresh-token" },
            "refresh_token": "a-live-refresh-token",
            "scope": "documents",
        });
        assert_same_answer(&answer.clone(), &answer, "the same answer");
        for (member, differing) in [
            ("access_token", "the same `access_token`"),
            ("refresh_token", "the same `refresh_token`"),
            ("granted", "the same tokens at every depth"),
        ] {
            let mut other = answer.clone();
            other[member] = if member == "granted" {
                json!({ "refresh_token": "another-token" })
            } else {
                json!("another-token")
            };
            let failed = catch_unwind(|| assert_same_answer(&other, &answer, "a repeat"));
            let failure = failed.expect_err("answers that differ in a token are told apart");
            let message = failure
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_default();
            assert!(message.ends_with(differing), "{member}: {message}");
            for token in ["a-live", "a-nested", "another-token"] {
                assert!(!message.contains(token), "{member}: {message}");
            }
        }
    }
}
