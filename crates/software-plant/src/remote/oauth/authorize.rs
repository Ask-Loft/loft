//! The authorization endpoint: an authorization request of the code grant with PKCE (RFC 7636),
//! from a registered client to one of its redirects, naming each protected resource it asks for
//! (RFC 8707) and the scopes it asks of them, answered by a redirect back that carries a code for
//! all of them, or an error, as the person consents.

use super::register::{loopback, portless};
use super::{Code, Form};
use crate::http::Response;
use crate::remote::wire::query;
use crate::remote::{Consent, State};

fn redirect(uri: &str, parameters: &[(&str, &str)]) -> Response {
    let separator = if uri.contains('?') { '&' } else { '?' };
    Response::redirect(&format!("{uri}{separator}{}", query(parameters)))
}

pub(crate) fn authorize(state: &mut State, query: &str) -> Response {
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
    match field("response_type").as_str() {
        "code" => {}
        "" => return fail("invalid_request"),
        _ => return fail("unsupported_response_type"),
    }
    if field("code_challenge_method") != "S256" || field("code_challenge").is_empty() {
        return fail("invalid_request");
    }
    let (resources, scopes) = match judged(state, &query) {
        Ok(asked) => asked,
        Err(error) => return fail(error),
    };
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
            resources,
            scopes,
            used: false,
        },
    );
    redirect(&redirect_uri, &[("code", &code), ("state", &state_value)])
}

/// The resources and the scopes that the authorization request `query` asks for, or the error it
/// is refused with. Its resources are judged first: at least one, each one the scenario serves,
/// or `invalid_target`. Then its `scope`, which holds scopes separated by single spaces, each
/// once and each accepted by a resource it names, or `invalid_scope`. Then each resource, which
/// is asked for a scope it accepts, or `invalid_target`.
fn judged(state: &State, query: &Form) -> Result<(Vec<String>, Vec<String>), &'static str> {
    let resources = query.fields("resource");
    let accepted: Option<Vec<Vec<String>>> = resources
        .iter()
        .map(|resource| state.accepted(resource))
        .collect();
    let accepted = accepted.filter(|accepted| !accepted.is_empty());
    let accepted = accepted.ok_or("invalid_target")?;
    let mut scopes: Vec<String> = Vec::new();
    for scope in query.field("scope").split(' ') {
        let listed = accepted
            .iter()
            .any(|accepts| accepts.iter().any(|s| s == scope));
        if !listed || scopes.iter().any(|asked| asked == scope) {
            return Err("invalid_scope");
        }
        scopes.push(scope.to_owned());
    }
    let unasked = accepted
        .iter()
        .any(|accepts| !accepts.iter().any(|s| scopes.contains(s)));
    if unasked {
        return Err("invalid_target");
    }
    Ok((resources, scopes))
}
