//! The calls under the document's address: its head, a published state, a draft, a publish, a
//! create and an import, each admitted only with a bearer token the simulator accepts.

use std::collections::BTreeMap;

use loftspec::codec::{Files, LABELS, SpecPath};
use loftspec::fmt::format;
use loftspec::publish::{self, Lock, Made, Publication, PublishedAt, ReadTree};
use serde_json::{Value, json};

use super::problem::{self, conflict, invalid, refusal};
use super::wire::{decode_segment, sha256_hex};
use super::{
    Call, Held, Metadata, Misstating, RemoteScenario, State, StoredState, TOKEN, head_mark,
};
use crate::http::{Request, Response};

/// The longest name a document may have, in Unicode scalar values.
const NAME_LIMIT: usize = 200;

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

fn head(state: &State) -> Response {
    let Some(held) = &state.held else {
        return refusal("not_found");
    };
    Response::json(
        200,
        &json!({ "documentKey": state.scenario.key, "head": head_of(held) }),
    )
}

/// The head as `GET <address>/head` gives it: `null`, or its lock and its files' digests.
fn head_of(held: &Held) -> Value {
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

/// `head`, a head or a lock, with a state root that is not its own, its last digit changed, in
/// the lock it holds too. A head that is `null` names no state root and stays as it is.
fn misstated(mut head: Value) -> Value {
    let other = |root: &Value| {
        root.as_str().map(|root| {
            let last = if root.ends_with('0') { '1' } else { '0' };
            Value::String(format!("{}{last}", &root[..root.len() - 1]))
        })
    };
    if let Some(root) = other(&head["stateRoot"]) {
        head["stateRoot"] = root;
    }
    if let Some(root) = other(&head["lock"]["stateRoot"]) {
        head["lock"]["stateRoot"] = root;
    }
    head
}

fn texts(files: &BTreeMap<String, String>) -> Vec<Value> {
    files
        .iter()
        .map(|(path, text)| json!({ "path": path, "text": text }))
        .collect()
}

fn published(state: &State, sequence: &str) -> Response {
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

fn draft(state: &State, branch: &str) -> Response {
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

fn create(state: &mut State, body: &Value) -> Response {
    let Some(fields) = body.as_object() else {
        return invalid(&[], Some("the body is not a JSON object"));
    };
    let exact = fields.len() == 2 && fields.contains_key("documentKey");
    let Some(name) = fields.get("name").and_then(Value::as_str).filter(|_| exact) else {
        return invalid(&[], Some("the body holds exactly `documentKey` and `name`"));
    };
    if fields["documentKey"] != state.scenario.key.as_str() {
        return invalid(&[], None);
    }
    if name.trim().is_empty() || name.chars().count() > NAME_LIMIT {
        return invalid(&[], Some("`name` is blank or longer than 200 characters"));
    }
    if state.scenario.forbidden.contains(&Call::Create) {
        return refusal("forbidden");
    }
    if let Some(held) = &state.held {
        return conflict(head_mark(held), None);
    }
    state.held = Some(Held {
        name: name.to_owned(),
        ..Held::default()
    });
    Response::json(
        200,
        &json!({ "documentKey": state.scenario.key, "head": null }),
    )
}

/// The files of a state read from the wire: each `path` and `text`, by the path.
fn wire_files(value: &Value) -> Option<BTreeMap<String, String>> {
    value
        .as_array()?
        .iter()
        .map(|file| {
            Some((
                file["path"].as_str()?.to_owned(),
                file["text"].as_str()?.to_owned(),
            ))
        })
        .collect()
}

/// The files as the Format's reader takes them, by their paths under `spec/`.
fn spec_files(files: &BTreeMap<String, String>) -> Files {
    files
        .iter()
        .map(|(path, text)| {
            let under = path.strip_prefix("spec/").unwrap_or(path);
            (SpecPath::new(under), text.clone().into_bytes())
        })
        .collect()
}

/// The tree of `spec`, or `invalid_request` naming, from the top of the tree, each path a read
/// error names.
fn read_tree(spec: &Files) -> Result<ReadTree<'_>, Response> {
    ReadTree::read(spec).map_err(|errors| {
        let mut paths: Vec<String> = errors
            .iter()
            .flat_map(|error| &error.sites)
            .map(|site| format!("spec/{}", site.path.as_str()))
            .collect();
        paths.sort();
        paths.dedup();
        invalid(&paths, None)
    })
}

fn import(state: &mut State, request: &Request) -> Response {
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

fn publish(state: &mut State, body: &Value) -> Response {
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
    // The remote writes the bindings: a node the delta removed keeps its binding, departed.
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
