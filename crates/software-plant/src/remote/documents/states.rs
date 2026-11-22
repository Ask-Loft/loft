//! A published state, by its publish sequence, and a draft, by its branch: each answered with
//! the text of every file it holds, or `not_found`.

use serde_json::json;

use super::files::texts;
use crate::http::Response;
use crate::remote::State;
use crate::remote::problem::refusal;

pub(super) fn published(state: &State, sequence: &str) -> Response {
    let found = state.held.as_ref().and_then(|held| {
        held.states
            .iter()
            .find(|stored| sequence.parse() == Ok(stored.sequence()))
    });
    match found {
        Some(stored) => Response::json(
            200,
            &json!({
                "documentKey": state.scenario.key,
                "lock": stored.lock,
                "files": texts(&stored.files),
            }),
        ),
        None => refusal("not_found"),
    }
}

pub(super) fn draft(state: &State, branch: &str) -> Response {
    match state.held.as_ref().and_then(|held| held.drafts.get(branch)) {
        Some(files) => Response::json(
            200,
            &json!({
                "documentKey": state.scenario.key,
                "branch": branch,
                "files": texts(files),
            }),
        ),
        None => refusal("not_found"),
    }
}
