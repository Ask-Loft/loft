//! The contract suite of the remote simulator, for the calls that write: a publish, a create
//! and an import. Each scenario is asked of the simulator here; the real half of the suite lies
//! beside it, in `remote_contract.rs`.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use software_plant::remote::{Remote, RemoteScenario, StoredState};

use crate::common::remote::{history, spec, wire, with_criterion};
use crate::remote_contract::{assert_problem, get, holding, mark, post};

/// The body of a publish of `files` from `base`, with `retry` as its retry key.
fn publish_body(
    key: &str,
    base: &StoredState,
    files: &BTreeMap<String, String>,
    retry: &str,
) -> Value {
    let changed: Vec<Value> = files
        .iter()
        .filter(|(path, text)| base.files.get(*path) != Some(*text) && *path != "spec/labels.yaml")
        .map(|(path, text)| json!({ "path": path, "text": text }))
        .collect();
    json!({
        "documentKey": key,
        "retryKey": retry,
        "base": mark(base),
        "added": [],
        "changed": changed,
        "removed": [],
    })
}

#[test]
fn a_publish_is_answered_as_published_replayed_unchanged_or_conflict() {
    let (remote, history) = holding(1);
    let base = &history.states[0];
    let changed = wire(&with_criterion(
        &spec(&base.files),
        "01a18e30-0000-7000-8000-0000000000aa",
    ));
    let body = publish_body(&history.key, base, &changed, "retry-1");
    let answer = post(&remote, "/publish", &body);
    assert_eq!(answer.body["outcome"], "published", "{:?}", answer.body);
    assert_eq!(answer.body["lock"]["publishSequence"], 2);
    let labels = answer.body["labels"].as_array().unwrap();
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0]["key"], "01a18e30-0000-7000-8000-0000000000aa");
    assert!(
        answer.body["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["path"] == "spec/labels.yaml")
    );
    let replay = post(&remote, "/publish", &body);
    assert_eq!(replay.body["outcome"], "replayed");
    assert_eq!(replay.body["lock"], answer.body["lock"]);
    let moved = post(
        &remote,
        "/publish",
        &publish_body(&history.key, base, &changed, "retry-2"),
    );
    assert_problem(&moved, 409, "conflict");
    assert_eq!(moved.body["head"]["publishSequence"], 2);
    let head = &remote.states()[1];
    let same = post(
        &remote,
        "/publish",
        &publish_body(&history.key, head, &head.files, "retry-3"),
    );
    assert_eq!(same.body["outcome"], "unchanged");
    assert_eq!(same.body["lock"], head.lock);
    assert_eq!(
        (same.body["files"].clone(), same.body["labels"].clone()),
        (json!([]), json!([]))
    );
    assert_eq!(remote.states().len(), 2);
    let again = post(
        &remote,
        "/publish",
        &publish_body(&history.key, head, &head.files, "retry-3"),
    );
    assert_eq!(again.body["outcome"], "replayed");
    let mut keyless = publish_body(&history.key, head, &head.files, "");
    assert_problem(&post(&remote, "/publish", &keyless), 400, "invalid_request");
    keyless.as_object_mut().unwrap().remove("retryKey");
    assert_problem(&post(&remote, "/publish", &keyless), 400, "invalid_request");
}

#[test]
fn a_publish_the_remote_refuses_names_its_reason_or_its_paths() {
    let history = history(1);
    let base = &history.states[0];
    let held =
        || RemoteScenario::accepting(&history.key).holding("Reading List", history.states.clone());
    let closed = Remote::start(held().closed("the document takes no more writes"));
    let answer = post(
        &closed,
        "/publish",
        &publish_body(&history.key, base, &base.files, "r"),
    );
    assert_problem(&answer, 409, "conflict");
    assert_eq!(answer.body["head"], mark(base));
    assert_eq!(answer.body["detail"], "the document takes no more writes");
    let remote = Remote::start(held());
    let mut body = publish_body(&history.key, base, &base.files, "r");
    body["removed"] = json!(["spec/none.yaml"]);
    let invalid = post(&remote, "/publish", &body);
    assert_problem(&invalid, 400, "invalid_request");
    assert_eq!(invalid.body["paths"], json!(["spec/none.yaml"]));
}

#[test]
fn a_create_holds_the_key_and_name_or_is_refused() {
    let key = "01a18e30-0000-7000-8000-000000000003";
    let remote = Remote::start(RemoteScenario::accepting(key));
    for (body, detail) in [
        (json!({ "documentKey": key, "name": " " }), true),
        (json!({ "documentKey": key, "name": "x".repeat(201) }), true),
        (
            json!({ "documentKey": "01a18e30-0000-7000-8000-000000000009", "name": "N" }),
            false,
        ),
        (json!({ "documentKey": key, "name": "N", "extra": 1 }), true),
    ] {
        let answer = post(&remote, "/create", &body);
        assert_problem(&answer, 400, "invalid_request");
        assert_eq!(answer.body["paths"], json!([]));
        assert_eq!(answer.body.get("detail").is_some(), detail, "{body}");
    }
    let created = post(
        &remote,
        "/create",
        &json!({ "documentKey": key, "name": "Notes" }),
    );
    assert_eq!(created.body, json!({ "documentKey": key, "head": null }));
    assert_eq!(get(&remote, "/head").body, created.body);
    let again = post(
        &remote,
        "/create",
        &json!({ "documentKey": key, "name": "Notes" }),
    );
    assert_problem(&again, 409, "conflict");
    assert_eq!(
        again.body["head"],
        json!({ "publishSequence": 0, "stateRoot": null })
    );
    let forbidden = Remote::start(RemoteScenario::accepting(key).forbidding_create());
    let refused = post(
        &forbidden,
        "/create",
        &json!({ "documentKey": key, "name": "Notes" }),
    );
    assert_problem(&refused, 403, "forbidden");
}

fn import_body(key: &str, states: &[StoredState]) -> Value {
    let states: Vec<Value> = states
        .iter()
        .map(|state| {
            let files: Vec<Value> = state
                .files
                .iter()
                .map(|(path, text)| json!({ "path": path, "text": text }))
                .collect();
            json!({ "lock": state.lock, "files": files })
        })
        .collect();
    json!({ "documentKey": key, "states": states })
}

#[test]
fn an_import_takes_the_states_as_the_document_s_history() {
    let history = history(2);
    let remote =
        Remote::start(RemoteScenario::accepting(&history.key).holding("Reading List", Vec::new()));
    let answer = post(
        &remote,
        "/import",
        &import_body(&history.key, &history.states),
    );
    assert_eq!(answer.status, 200, "{:?}", answer.body);
    assert_eq!(answer.body, get(&remote, "/head").body);
    assert_eq!(answer.body["head"]["publishSequence"], 2);
    let again = post(
        &remote,
        "/import",
        &import_body(&history.key, &history.states),
    );
    assert_problem(&again, 409, "conflict");
    assert_eq!(again.body["head"]["publishSequence"], 2);
    let drafted = Remote::start(
        RemoteScenario::accepting(&history.key).draft("feat/x", history.states[0].files.clone()),
    );
    let refused = post(
        &drafted,
        "/import",
        &import_body(&history.key, &history.states),
    );
    assert_problem(&refused, 409, "conflict");
    assert_eq!(
        refused.body["head"],
        json!({ "publishSequence": 0, "stateRoot": null })
    );
    let blank =
        Remote::start(RemoteScenario::accepting(&history.key).draft("feat/x", BTreeMap::new()));
    let accepted = post(
        &blank,
        "/import",
        &import_body(&history.key, &history.states),
    );
    assert_eq!(accepted.status, 200, "an empty draft holds nothing");
}

#[test]
fn an_import_whose_states_do_not_hold_together_is_refused() {
    let history = history(2);
    let empty = || RemoteScenario::accepting(&history.key).holding("Reading List", Vec::new());
    let start = || Remote::start(empty());
    let body = import_body(&history.key, &history.states);
    let mut tampered = history.states.clone();
    tampered[0].files.insert(
        "spec/loftspec.yaml".to_owned(),
        "format: loftspec/1\n".to_owned(),
    );
    let mut reversed = history.states.clone();
    reversed.reverse();
    let mut backwards = history.states.clone();
    backwards[1].lock["publishedAt"] = json!("2000-01-01T00:00:00Z");
    let mut other = history.states[..1].to_vec();
    other[0].lock["documentKey"] = json!("01a18e30-0000-7000-8000-000000000004");
    let mut outside = history.states[..1].to_vec();
    let manifest = outside[0].files.remove("spec/loftspec.yaml").unwrap();
    outside[0]
        .files
        .insert("loftspec.yaml".to_owned(), manifest);
    let refused = post(&start(), "/import", &import_body(&history.key, &outside));
    assert_problem(&refused, 400, "invalid_request");
    assert_eq!(refused.body["paths"], json!(["loftspec.yaml"]));
    for states in [tampered, reversed, backwards, other] {
        assert_problem(
            &post(&start(), "/import", &import_body(&history.key, &states)),
            400,
            "invalid_request",
        );
    }
    let wrong_key = import_body("01a18e30-0000-7000-8000-000000000005", &history.states);
    assert_problem(
        &post(&start(), "/import", &wrong_key),
        400,
        "invalid_request",
    );
    let limited = Remote::start(empty().import_limit(10));
    assert_problem(&post(&limited, "/import", &body), 413, "payload_too_large");
    let one_state = Remote::start(empty().state_limit(10));
    assert_problem(
        &post(&one_state, "/import", &body),
        413,
        "payload_too_large",
    );
    let forbidden = Remote::start(empty().forbidding_import());
    assert_problem(&post(&forbidden, "/import", &body), 403, "forbidden");
}
