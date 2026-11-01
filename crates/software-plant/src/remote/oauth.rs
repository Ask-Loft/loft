//! The remote's authorization server, as OAuth 2.0 shapes it: the protected resource metadata
//! (RFC 9728), the authorization server metadata (RFC 8414), dynamic registration of public
//! clients (RFC 7591), the authorization code grant with PKCE (RFC 7636) on a loopback redirect
//! whose port may differ from the registered one (RFC 8252), the resource named by RFC 8707, and
//! refresh tokens that rotate on every use, with a grace window for one presented again and the
//! whole sign-in revoked otherwise. Its clock runs from the machine's, moved on by the test.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::problem::oauth;
use super::wire::{base64url, pairs, query, sha256, value};
use super::{Consent, State};
use crate::http::{Request, Response};

/// A sign-in that the simulator issued: the client it was issued to and its tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

/// An authorization code, until it is exchanged.
struct Code {
    client_id: String,
    redirect_uri: String,
    challenge: String,
    resource: String,
    used: bool,
}

/// A refresh token: the sign-in it belongs to, and its rotation, once it has been rotated.
struct Refresh {
    family: usize,
    client_id: String,
    rotated: Option<Rotation>,
}

/// A refresh token's rotation: the answer it got, when, and the refresh token it issued.
struct Rotation {
    answer: Value,
    at: Instant,
    successor: String,
}

/// One sign-in: its access tokens, each with the moment it expires, and whether it was
/// revoked.
#[derive(Default)]
struct Family {
    access: Vec<(String, Instant)>,
    revoked: bool,
}

/// What the authorization server holds.
#[derive(Default)]
pub(super) struct Auth {
    clients: HashMap<String, Vec<String>>,
    codes: HashMap<String, Code>,
    refresh: HashMap<String, Refresh>,
    families: Vec<Family>,
    issued: u64,
    pub(super) refreshes: usize,
    /// How far the test moved the clock on from the machine's.
    pub(super) moved: Duration,
}

impl Auth {
    /// The simulator's time: the machine's, moved on by the test.
    fn now(&self) -> Instant {
        Instant::now() + self.moved
    }

    fn next(&mut self, kind: &str) -> String {
        self.issued += 1;
        format!("{kind}-{}", self.issued)
    }

    /// Whether `token` is an access token, not yet expired, issued by a sign-in that was not
    /// revoked.
    pub(super) fn admits(&self, token: &str) -> bool {
        self.families.iter().any(|family| {
            !family.revoked
                && family
                    .access
                    .iter()
                    .any(|(issued, expires)| issued == token && self.now() < *expires)
        })
    }

    /// Begins a sign-in, and gives its family.
    fn sign_in(&mut self) -> usize {
        self.families.push(Family::default());
        self.families.len() - 1
    }

    /// Issues an access token and a refresh token in the sign-in `family`, as the answer of the
    /// token endpoint.
    fn issue(&mut self, family: usize, client_id: &str, expires_in: u64, scope: &str) -> Value {
        let access = self.next("access");
        let refresh = self.next("refresh");
        let expires = self.now() + Duration::from_secs(expires_in);
        self.families[family].access.push((access.clone(), expires));
        self.refresh.insert(
            refresh.clone(),
            Refresh {
                family,
                client_id: client_id.to_owned(),
                rotated: None,
            },
        );
        json!({
            "access_token": access,
            "token_type": "Bearer",
            "expires_in": expires_in,
            "refresh_token": refresh,
            "scope": scope,
        })
    }

    /// Whether a refresh token rotated as `rotation` is answered again: within `grace` of its
    /// rotation, and before its successor has itself been presented.
    fn repeats(&self, rotation: &Rotation, grace: Duration) -> bool {
        let successor_rotated = self
            .refresh
            .get(&rotation.successor)
            .is_some_and(|successor| successor.rotated.is_some());
        self.now() < rotation.at + grace && !successor_rotated
    }

    /// A registered client and a new sign-in for it, without a browser.
    pub(super) fn grant(&mut self, expires_in: u64) -> super::Grant {
        let client_id = self.next("client");
        self.clients.insert(client_id.clone(), Vec::new());
        let family = self.sign_in();
        let answer = self.issue(family, &client_id, expires_in, "");
        super::Grant {
            client_id,
            access_token: answer["access_token"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            refresh_token: answer["refresh_token"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            expires_in,
        }
    }
}

impl State {
    fn issuer(&self) -> String {
        format!("{}/auth", self.origin)
    }

    fn resource(&self) -> String {
        format!("{}{}", self.origin, super::RESOURCE)
    }
}

pub(super) fn resource_metadata(state: &State) -> Response {
    Response::json(
        200,
        &json!({
            "resource": state.resource(),
            "authorization_servers": [state.issuer()],
            "scopes_supported": [state.scenario.scope],
            "bearer_methods_supported": ["header"],
        }),
    )
}

pub(super) fn server_metadata(state: &State) -> Response {
    let issuer = state.issuer();
    Response::json(
        200,
        &json!({
            "issuer": issuer,
            "authorization_endpoint": format!("{issuer}/authorize"),
            "token_endpoint": format!("{issuer}/token"),
            "registration_endpoint": format!("{issuer}/register"),
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none"],
            "scopes_supported": [state.scenario.scope],
        }),
    )
}

/// Whether `uri` is `http` on the loopback interface.
fn loopback(uri: &str) -> bool {
    uri.strip_prefix("http://").is_some_and(|rest| {
        let host = rest.split(['/', '?']).next().unwrap_or_default();
        let host = host.rsplit_once(':').map_or(host, |(host, _)| host);
        matches!(host, "127.0.0.1" | "[::1]" | "localhost")
    })
}

/// A URI without the port of its authority, for matching a loopback redirect.
fn portless(uri: &str) -> String {
    let Some(rest) = uri.strip_prefix("http://") else {
        return uri.to_owned();
    };
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let host = authority
        .rsplit_once(':')
        .map_or(authority, |(host, _)| host);
    format!("http://{host}{path}")
}

pub(super) fn register(state: &mut State, request: &Request) -> Response {
    let Ok(metadata) = serde_json::from_slice::<Value>(&request.body) else {
        return oauth("invalid_client_metadata");
    };
    let uris: Vec<String> = metadata["redirect_uris"]
        .as_array()
        .map(|uris| {
            uris.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if uris.is_empty()
        || uris
            .iter()
            .any(|uri| !uri.starts_with("https://") && !loopback(uri))
    {
        return oauth("invalid_redirect_uri");
    }
    let mut grants: Vec<&str> = metadata["grant_types"]
        .as_array()
        .map(|grants| grants.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    grants.sort_unstable();
    if metadata["token_endpoint_auth_method"] != "none"
        || grants != ["authorization_code", "refresh_token"]
    {
        return oauth("invalid_client_metadata");
    }
    let client_id = state.auth.next("client");
    state.auth.clients.insert(client_id.clone(), uris.clone());
    let mut answer = metadata;
    answer["client_id"] = json!(client_id);
    answer["client_id_issued_at"] = json!(state.scenario.publish_from);
    Response::json(201, &answer)
}

fn redirect(uri: &str, parameters: &[(&str, &str)]) -> Response {
    let separator = if uri.contains('?') { '&' } else { '?' };
    Response::redirect(&format!("{uri}{separator}{}", query(parameters)))
}

pub(super) fn authorize(state: &mut State, query: &str) -> Response {
    let query = Form::read(query);
    let field = |name| query.field(name);
    let (client_id, redirect_uri, state_value) =
        (field("client_id"), field("redirect_uri"), field("state"));
    let registered = state.auth.clients.get(&client_id).is_some_and(|uris| {
        uris.iter().any(|uri| {
            uri == &redirect_uri || (loopback(uri) && portless(uri) == portless(&redirect_uri))
        })
    });
    if !registered {
        return Response::empty(400);
    }
    let fail = |error| redirect(&redirect_uri, &[("error", error), ("state", &state_value)]);
    if field("response_type") != "code"
        || field("code_challenge_method") != "S256"
        || field("code_challenge").is_empty()
    {
        return fail("invalid_request");
    }
    if field("resource") != state.resource() {
        return fail("invalid_target");
    }
    if state.scenario.consent == Consent::Deny {
        return fail("access_denied");
    }
    let code = state.auth.next("code");
    state.auth.codes.insert(
        code.clone(),
        Code {
            client_id,
            redirect_uri: redirect_uri.clone(),
            challenge: field("code_challenge"),
            resource: field("resource"),
            used: false,
        },
    );
    redirect(&redirect_uri, &[("code", &code), ("state", &state_value)])
}

/// A query string or a form-encoded body, read by field.
struct Form(Vec<(String, String)>);

impl Form {
    fn read(text: &str) -> Self {
        Self(pairs(text))
    }

    /// The value of the field `name`, or nothing.
    fn field(&self, name: &str) -> String {
        value(&self.0, name).unwrap_or_default().to_owned()
    }
}

pub(super) fn token(state: &mut State, request: &Request) -> Response {
    let form = Form::read(&String::from_utf8_lossy(&request.body));
    match form.field("grant_type").as_str() {
        "authorization_code" => exchange(state, &form),
        "refresh_token" => refresh(state, &form),
        _ => oauth("unsupported_grant_type"),
    }
}

fn exchange(state: &mut State, form: &Form) -> Response {
    let field = |name| form.field(name);
    let Some(code) = state.auth.codes.get_mut(&field("code")) else {
        return oauth("invalid_grant");
    };
    let verified = base64url(&sha256(field("code_verifier").as_bytes())) == code.challenge;
    if code.used
        || code.client_id != field("client_id")
        || code.redirect_uri != field("redirect_uri")
        || !verified
    {
        return oauth("invalid_grant");
    }
    if code.resource != field("resource") {
        return oauth("invalid_target");
    }
    code.used = true;
    let client_id = code.client_id.clone();
    let family = state.auth.sign_in();
    let (expires_in, scope) = (state.scenario.expires_in, state.scenario.scope.clone());
    Response::json(
        200,
        &state.auth.issue(family, &client_id, expires_in, &scope),
    )
}

fn refresh(state: &mut State, form: &Form) -> Response {
    let field = |name| form.field(name);
    if state.scenario.lapsed {
        return oauth("invalid_grant");
    }
    let presented = field("refresh_token");
    let Some(entry) = state.auth.refresh.get(&presented) else {
        return oauth("invalid_grant");
    };
    let family = entry.family;
    if entry.client_id != field("client_id") || state.auth.families[family].revoked {
        return oauth("invalid_grant");
    }
    if field("resource") != state.resource() {
        return oauth("invalid_target");
    }
    if let Some(rotation) = &entry.rotated {
        if state.auth.repeats(rotation, state.scenario.grace) {
            return Response::json(200, &rotation.answer);
        }
        state.auth.families[family].revoked = true;
        return oauth("invalid_grant");
    }
    let client_id = entry.client_id.clone();
    let (expires_in, scope) = (state.scenario.expires_in, state.scenario.scope.clone());
    let answer = state.auth.issue(family, &client_id, expires_in, &scope);
    let at = state.auth.now();
    if let Some(entry) = state.auth.refresh.get_mut(&presented) {
        entry.rotated = Some(Rotation {
            successor: answer["refresh_token"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            answer: answer.clone(),
            at,
        });
    }
    state.auth.refreshes += 1;
    Response::json(200, &answer)
}
