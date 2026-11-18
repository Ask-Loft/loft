//! The token endpoint: a code exchanged once, with its verifier, its redirect and one resource its
//! authorization request named, for an access token for that resource alone and a refresh token
//! for the sign-in; and a refresh, which rotates the refresh token, answers a rotated one again
//! within the grace window until its successor is presented, and revokes the whole sign-in
//! otherwise. A resource the scenario does not serve, or none, is refused before the code is
//! read.

use super::{Form, Issued, Rotation};
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
    let (client_id, scopes) = (code.client_id.clone(), code.scopes.clone());
    let family = state.auth.sign_in(scopes);
    let token = Issued {
        resource: &resource,
        accepted: &accepted,
        expires_in: state.scenario.expires_in,
    };
    Response::json(200, &state.auth.issue(family, &client_id, &token))
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
    let resource = field("resource");
    let accepted = state.accepted(&resource).unwrap_or_default();
    let token = Issued {
        resource: &resource,
        accepted: &accepted,
        expires_in: state.scenario.expires_in,
    };
    let answer = state.auth.issue(family, &client_id, &token);
    let at = state.auth.now();
    if let Some(entry) = state.auth.refresh.get_mut(&presented) {
        entry.rotated = Some(Rotation {
            answer: answer.clone(),
            at,
        });
    }
    state.auth.refreshes += 1;
    state.answer_lost = std::mem::take(&mut state.scenario.losing_refresh);
    Response::json(200, &answer)
}
