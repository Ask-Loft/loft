//! The authorization endpoint: an authorization request of the code grant with PKCE (RFC 7636),
//! from a registered client to one of its redirects, naming the protected resource (RFC 8707),
//! answered by a redirect back that carries a code, or an error, as the person consents.

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
