//! An import: the published states of a document that holds none yet, each verified against its
//! lock, taken as its history and answered with the head they make.

use loftspec::publish::{self, Lock};
use serde_json::{Value, json};

use super::files::{read_tree, spec_files, wire_files};
use super::head::{head_of, misstated};
use crate::http::{Request, Response};
use crate::remote::problem::{conflict, invalid, refusal};
use crate::remote::{Call, Held, Misstating, RemoteScenario, State, StoredState, head_mark};

pub(super) fn import(state: &mut State, request: &Request) -> Response {
    if state
        .scenario
        .import_limit
        .is_some_and(|limit| request.body.len() > limit)
    {
        return refusal("payload_too_large");
    }
    let Some(held) = &state.held else {
        return refusal("not_found");
    };
    if !held.states.is_empty() || held.drafts.values().any(|files| !files.is_empty()) {
        return conflict(head_mark(held), None);
    }
    if state.scenario.forbidden.contains(&Call::Import) {
        return refusal("forbidden");
    }
    let body = serde_json::from_slice::<Value>(&request.body).unwrap_or(Value::Null);
    if body["documentKey"] != state.scenario.key.as_str() {
        return invalid(&[], None);
    }
    match imported(&state.scenario, &body["states"]) {
        Ok(states) => {
            let held = state.held.get_or_insert_with(Held::default);
            held.states = states;
            let mut answered = head_of(held);
            if state.scenario.misstating.contains(&Misstating::ImportHead) {
                answered = misstated(answered);
            }
            Response::json(
                200,
                &json!({ "documentKey": state.scenario.key, "head": answered }),
            )
        }
        Err(refused) => refused,
    }
}

/// The states of an import, each verified against its lock, from publish sequence 1 upward
/// with publish times that never go back.
fn imported(scenario: &RemoteScenario, states: &Value) -> Result<Vec<StoredState>, Response> {
    let key = scenario.key.as_str();
    let states = states.as_array().ok_or_else(|| invalid(&[], None))?;
    let mut read = Vec::with_capacity(states.len());
    let mut last_time = 0;
    for (index, state) in states.iter().enumerate() {
        let files = wire_files(&state["files"]).ok_or_else(|| invalid(&[], None))?;
        let outside: Vec<String> = files
            .keys()
            .filter(|path| !path.starts_with("spec/"))
            .cloned()
            .collect();
        if !outside.is_empty() {
            return Err(invalid(&outside, None));
        }
        let size: usize = files.values().map(String::len).sum();
        if scenario.state_limit.is_some_and(|limit| size > limit) {
            return Err(refusal("payload_too_large"));
        }
        let lock = Lock::read(state["lock"].to_string().as_bytes())
            .map_err(|error| invalid(&[], Some(&error.to_string())))?;
        let expected = u64::try_from(index + 1).unwrap_or(u64::MAX);
        if lock.publish_sequence.get() != expected {
            return Err(invalid(
                &[],
                Some("publish sequences do not run 1, 2, 3 and on"),
            ));
        }
        if lock.document_key.key().to_string() != key {
            return Err(invalid(&[], Some("a lock names another document key")));
        }
        let time = lock.published_at.unix_seconds();
        if time < last_time {
            return Err(invalid(&[], Some("publish times go backwards")));
        }
        last_time = time;
        let spec = spec_files(&files);
        let tree = read_tree(&spec)?;
        if publish::verify(&tree, &lock).is_err() {
            return Err(invalid(
                &[],
                Some("a state's files do not verify against its lock"),
            ));
        }
        read.push(StoredState {
            lock: state["lock"].clone(),
            files,
        });
    }
    Ok(read)
}
