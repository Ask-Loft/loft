//! The calls under the document's address: its head, a published state, a draft, a publish, a
//! create and an import, each admitted only with a bearer token the simulator accepts.
//!
//! The admission and the routing lie here; each call lies in a module of its own.

mod create;
mod files;
mod head;
mod import;
mod publish;
mod states;

use serde_json::Value;

use super::problem::refusal;
use super::wire::decode_segment;
use super::{Metadata, State, TOKEN};
use crate::http::{Request, Response};
use create::create;
use head::head;
use import::import;
use publish::publish;
use states::{draft, published};

pub use head::misstated_root;

impl State {
    /// Whether a request bearing `token` is admitted.
    pub(super) fn admits(&self, token: &str) -> bool {
        token == TOKEN || self.auth.admits(token)
    }

    fn challenge(&self) -> Response {
        let mut response = refusal("unauthenticated");
        let metadata = format!(
            "{}/.well-known/oauth-protected-resource{}",
            self.origin,
            super::RESOURCE
        );
        let scope = match self.scenario.metadata {
            Metadata::Unscoped(_) => String::new(),
            _ => format!(", scope=\"{}\"", self.scenario.scope),
        };
        response.headers.push((
            "WWW-Authenticate".to_owned(),
            format!("Bearer resource_metadata=\"{metadata}\"{scope}"),
        ));
        response
    }
}

/// The answer to a request under the document's address, `rest` being its path after it.
pub(super) fn answer(state: &mut State, request: &Request, rest: &str) -> Response {
    let segments: Vec<&str> = rest.split('/').skip(1).collect();
    let get = request.method == "GET";
    let post = request.method == "POST";
    let known = match segments.as_slice() {
        ["head"] | ["states" | "drafts", _] => get,
        ["publish" | "create" | "import"] => post,
        _ => return refusal("not_found"),
    };
    if !known {
        return refusal("method_not_allowed");
    }
    let bearer = request
        .header("Authorization")
        .and_then(|value| value.strip_prefix("Bearer "));
    if !bearer.is_some_and(|token| state.admits(token)) {
        return state.challenge();
    }
    let body = || serde_json::from_slice::<Value>(&request.body).unwrap_or(Value::Null);
    match segments.as_slice() {
        ["head"] => head(state),
        ["states", sequence] => published(state, sequence),
        ["drafts", branch] => draft(state, &decode_segment(branch)),
        ["publish"] => publish(state, &body()),
        ["create"] => create(state, &body()),
        _ => import(state, request),
    }
}
