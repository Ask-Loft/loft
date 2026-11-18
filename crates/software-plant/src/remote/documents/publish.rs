//! A publish: a delta from the head, sent once under its retry key and replayed for that key,
//! published with the Format's own publisher made as the document's remote, which gives labels
//! and writes the bindings, and answered with the lock and the files it wrote.

use std::collections::BTreeMap;

use loftspec::codec::{LABELS, SpecPath};
use loftspec::fmt::format;
use loftspec::publish::{self, Lock, Made, Publication, PublishedAt};
use serde_json::{Value, json};

use super::files::{read_tree, spec_files, texts, wire_files};
use super::head::misstated;
use crate::http::Response;
use crate::remote::problem::{self, conflict, invalid, refusal};
use crate::remote::{Held, Misstating, State, StoredState, head_mark};

pub(super) fn publish(state: &mut State, body: &Value) -> Response {
    let Some(held) = &state.held else {
        return refusal("not_found");
    };
    if body["documentKey"] != state.scenario.key.as_str() {
        return invalid(&[], None);
    }
    let Some(retry) = body["retryKey"]
        .as_str()
        .filter(|retry| !retry.is_empty())
        .map(str::to_owned)
    else {
        return invalid(&[], Some("a publish names its `retryKey`"));
    };
    if let Some(reply) = state.replies.get(&retry) {
        let mut reply = reply.clone();
        reply["outcome"] = json!("replayed");
        return Response::json(200, &reply);
    }
    let mark = head_mark(held);
    if body["base"]["publishSequence"] != mark["publishSequence"]
        || body["base"]["stateRoot"] != mark["stateRoot"]
    {
        return conflict(mark, None);
    }
    if let Some(detail) = &state.scenario.closed {
        return conflict(mark, Some(detail));
    }
    if let Some(paths) = &state.scenario.invalid_publish {
        return invalid(paths, None);
    }
    let at = state.scenario.publish_from + held.states.len() as u64;
    match published_from(held, body, at) {
        Ok((mut reply, stored)) => {
            if state.scenario.misstating.contains(&Misstating::Lock) {
                reply["lock"] = misstated(reply["lock"].take());
            }
            if let (Some(stored), Some(held)) = (stored, &mut state.held) {
                held.states.push(stored);
            }
            state.replies.insert(retry, reply.clone());
            Response::json(200, &reply)
        }
        Err(refused) => refused,
    }
}

/// The head's files with the delta of `body` applied.
fn applied(held: &Held, body: &Value) -> Result<BTreeMap<String, String>, Response> {
    let mut files = held
        .states
        .last()
        .map(|state| state.files.clone())
        .unwrap_or_default();
    let mut bad = Vec::new();
    for name in ["added", "changed"] {
        let delta = wire_files(&body[name]).ok_or_else(|| invalid(&[], None))?;
        for (path, text) in delta {
            if !path.starts_with("spec/") {
                bad.push(path.clone());
            }
            files.insert(path, text);
        }
    }
    for path in body["removed"]
        .as_array()
        .ok_or_else(|| invalid(&[], None))?
    {
        let path = path.as_str().unwrap_or_default();
        if files.remove(path).is_none() {
            bad.push(path.to_owned());
        }
    }
    if bad.is_empty() {
        Ok(files)
    } else {
        Err(invalid(&bad, None))
    }
}

/// The answer to a publish from the head, with the state it adds where it adds one.
fn published_from(
    held: &Held,
    body: &Value,
    at: u64,
) -> Result<(Value, Option<StoredState>), Response> {
    let files = applied(held, body)?;
    let mut spec = spec_files(&files);
    // The remote writes the bindings: a node the delta removed keeps its binding, departed. A
    // tree that does not format does not read either, and is refused as such below.
    if let Ok(formatted) = format(&spec) {
        let labels = SpecPath::new(LABELS);
        if let Some(bytes) = formatted.changed.get(&labels) {
            spec.insert(labels, bytes.clone());
        }
    }
    let tree = read_tree(&spec)?;
    let previous = held
        .states
        .last()
        .map(|state| Lock::read(state.lock.to_string().as_bytes()).expect("a held lock reads"));
    let at = PublishedAt::from_unix_seconds(at).expect("a publish time");
    let publication = publish::publish(tree, None, previous.as_ref(), at, Made::AsRemote)
        .map_err(|refusal| invalid(&[], Some(&format!("{refusal:?}"))))?;
    match publication {
        Publication::Unchanged(lock) => Ok((
            json!({ "outcome": "unchanged", "lock": lock_json(&lock), "files": [], "labels": [] }),
            None,
        )),
        Publication::Published {
            lock,
            files: rewritten,
            given_back,
            allocated,
            ..
        } => {
            let mut stored = files;
            let mut returned = BTreeMap::new();
            for (path, bytes) in rewritten {
                let path = format!("spec/{}", path.as_str());
                let text = String::from_utf8_lossy(&bytes).into_owned();
                stored.insert(path.clone(), text.clone());
                returned.insert(path, text);
            }
            let labels: Vec<Value> = given_back
                .iter()
                .chain(&allocated)
                .map(|given| json!({ "key": given.element.key().to_string(), "label": given.label.to_string() }))
                .collect();
            let lock = lock_json(&lock);
            let reply = json!({
                "outcome": "published",
                "lock": lock,
                "files": texts(&returned),
                "labels": labels,
            });
            Ok((
                reply,
                Some(StoredState {
                    lock,
                    files: stored,
                }),
            ))
        }
        Publication::Prepared { .. } => Err(problem::refusal("internal_error")),
    }
}

fn lock_json(lock: &Lock) -> Value {
    serde_json::from_slice(&lock.to_bytes()).unwrap_or(Value::Null)
}
