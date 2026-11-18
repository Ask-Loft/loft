//! The token endpoint: a code exchanged once, with its verifier, its redirect and one resource its
//! authorization request named, for an access token for that resource alone and a refresh token
//! for the sign-in; and a refresh for one resource the sign-in holds, which rotates the refresh
//! token and issues an access token for that resource alone, answers a rotated one again for the
//! resource its rotation named within the grace window until its successor rotates, and revokes
//! the whole sign-in otherwise. A resource the scenario does not serve, or none, is refused before
//! the code or the refresh token is read, and a resource that a live refresh token's sign-in does
//! not hold is refused with nothing rotated.

use std::time::Duration;

use super::{Auth, Form, Issued, Rotation};
use crate::http::{Request, Response};
use crate::remote::State;
use crate::remote::problem::oauth;
use crate::remote::wire::{base64url, sha256};

/// Whether `request` asks the token endpoint for a refresh.
pub(crate) fn refreshing(request: &Request) -> bool {
    request.path == "/auth/token"
        && Form::read(&String::from_utf8_lossy(&request.body)).field("grant_type")
            == "refresh_token"
}

pub(crate) fn token(state: &mut State, request: &Request) -> Response {
    let form = Form::read(&String::from_utf8_lossy(&request.body));
    match form.field("grant_type").as_str() {
        "authorization_code" => exchange(state, &form),
        "refresh_token" => refresh(state, &form),
        _ => oauth("unsupported_grant_type"),
    }
}

fn exchange(state: &mut State, form: &Form) -> Response {
    let field = |name| form.field(name);
    let resource = field("resource");
    let Some(accepted) = state.accepted(&resource) else {
        return oauth("invalid_target");
    };
    if let Some(error) = &state.scenario.refused_exchange {
        return oauth(error);
    }
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
    if !code.resources.contains(&resource) {
        return oauth("invalid_target");
    }
    code.used = true;
    let resources = code.resources.clone();
    let (client_id, scopes) = (code.client_id.clone(), code.scopes.clone());
    let family = state.auth.sign_in(resources, scopes);
    let token = Issued {
        resource: &resource,
        accepted: &accepted,
        expires_in: state.scenario.expires_in,
    };
    Response::json(200, &state.auth.issue(family, &client_id, &token))
}

/// A refresh, judged in order: a resource the scenario does not serve, or none, before the
/// refresh token is read; then a lapsing scenario, a refresh token the simulator does not know,
/// another client's, or one of a revoked sign-in; then a rotated refresh token; then a resource
/// that the sign-in does not hold. Otherwise the refresh token rotates.
fn refresh(state: &mut State, form: &Form) -> Response {
    let field = |name| form.field(name);
    let resource = field("resource");
    let Some(accepted) = state.accepted(&resource) else {
        return oauth("invalid_target");
    };
    if state.scenario.lapsed {
        return oauth("invalid_grant");
    }
    let presented = field("refresh_token");
    let Some(entry) = state.auth.refresh.get(&presented) else {
        return oauth("invalid_grant");
    };
    let (family, client_id) = (entry.family, entry.client_id.clone());
    if client_id != field("client_id") || state.auth.families[family].revoked {
        return oauth("invalid_grant");
    }
    if let Some(answer) = rotated(&mut state.auth, &presented, &resource, state.scenario.grace) {
        return answer;
    }
    if !state.auth.families[family].holds(&resource) {
        return oauth("invalid_target");
    }
    let token = Issued {
        resource: &resource,
        accepted: &accepted,
        expires_in: state.scenario.expires_in,
    };
    let answer = state.auth.issue(family, &client_id, &token);
    let at = state.auth.now();
    if let Some(entry) = state.auth.refresh.get_mut(&presented) {
        entry.rotated = Some(Rotation {
            resource,
            answer: answer.clone(),
            at,
        });
    }
    state.auth.refreshes += 1;
    state.answer_lost = std::mem::take(&mut state.scenario.losing_refresh);
    Response::json(200, &answer)
}

/// The answer to `presented`, a refresh token presented again for `resource` after it rotated:
/// its rotation's answer, where it repeats within `grace`, and otherwise `invalid_grant`, with its
/// sign-in revoked as a reuse. Nothing, where it has not rotated.
fn rotated(auth: &mut Auth, presented: &str, resource: &str, grace: Duration) -> Option<Response> {
    let entry = auth.refresh.get(presented)?;
    let rotation = entry.rotated.as_ref()?;
    if auth.repeats(rotation, resource, grace) {
        return Some(Response::json(200, &rotation.answer));
    }
    let family = entry.family;
    auth.families[family].revoked = true;
    Some(oauth("invalid_grant"))
}
