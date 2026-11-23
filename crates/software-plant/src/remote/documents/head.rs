//! The head, as `GET <address>/head` gives it and as an accepted import answers it, and the
//! state root a misstating scenario gives in place of a head's or a lock's own.

use serde_json::{Value, json};

use crate::http::Response;
use crate::remote::problem::refusal;
use crate::remote::wire::sha256_hex;
use crate::remote::{Call, Held, State};

pub(super) fn head(state: &State) -> Response {
    if state.scenario.forbidden.contains(&Call::Head) {
        return refusal("forbidden");
    }
    let Some(held) = &state.held else {
        return refusal("not_found");
    };
    let mut answer = json!({ "documentKey": state.scenario.key, "head": head_of(held) });
    if let Some(visibility) = state.scenario.visibility {
        answer["visibility"] = json!(visibility.name());
    }
    Response::json(200, &answer)
}

/// The head as `GET <address>/head` gives it: `null`, or its lock and its files' digests.
pub(super) fn head_of(held: &Held) -> Value {
    held.states.last().map_or(Value::Null, |state| {
        let files: Vec<Value> = state
            .files
            .iter()
            .map(|(path, text)| json!({ "path": path, "sha256": sha256_hex(text.as_bytes()) }))
            .collect();
        json!({
            "publishSequence": state.sequence(),
            "stateRoot": state.lock["stateRoot"],
            "lock": state.lock,
            "files": files,
        })
    })
}

/// The state root a misstating scenario gives in place of `root`: its last digit changed.
#[must_use]
pub fn misstated_root(root: &str) -> String {
    let last = if root.ends_with('0') { '1' } else { '0' };
    format!("{}{last}", &root[..root.len() - 1])
}

/// `head`, a head or a lock, with a state root that is not its own, in the lock it holds too. A
/// head that is `null` names no state root and stays as it is.
pub(super) fn misstated(mut head: Value) -> Value {
    let other = |root: &Value| {
        root.as_str()
            .map(|root| Value::String(misstated_root(root)))
    };
    if let Some(root) = other(&head["stateRoot"]) {
        head["stateRoot"] = root;
    }
    if let Some(root) = other(&head["lock"]["stateRoot"]) {
        head["lock"]["stateRoot"] = root;
    }
    head
}
