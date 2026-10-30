//! The contract suite of the remote simulator, for the calls under a document's address: each
//! scenario asks the simulator, and on demand a real remote, the same question. The real half
//! is ignored by default; run it against a remote that holds a document with
//! `LOFT_REMOTE_CONTRACT_ADDRESS` and `LOFT_REMOTE_CONTRACT_TOKEN` set:
//! `cargo nextest run -p loft-cli --run-ignored only -E 'test(/remote_contract/)'`.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::{Value, json};
use software_plant::remote::problem::CODES;
use software_plant::remote::{Remote, RemoteScenario, StoredState, TOKEN};

use crate::common::remote::{History, history};

/// An answer: its status, its headers by lower-case name, and its body as JSON or `null`.
pub struct Answer {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
}

impl Answer {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

/// A client that follows no redirect and reads every status as an answer.
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into()
}

/// `method url`, with a bearer `token` and a JSON `body` where given.
pub fn call(method: &str, url: &str, token: Option<&str>, body: Option<&Value>) -> Answer {
    let request = ureq::http::Request::builder().method(method).uri(url);
    let request = match token {
        Some(token) => request.header("Authorization", format!("Bearer {token}")),
        None => request,
    };
    let response = match body {
        Some(body) => agent().run(
            request
                .header("Content-Type", "application/json")
                .body(body.to_string())
                .unwrap(),
        ),
        None => agent().run(request.body(()).unwrap()),
    };
    let mut response = response.expect("the remote answers");
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                value.to_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    let bytes = response.body_mut().read_to_vec().unwrap_or_default();
    Answer {
        status: response.status().as_u16(),
        headers,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

pub fn get(remote: &Remote, path: &str) -> Answer {
    call(
        "GET",
        &format!("{}{path}", remote.address()),
        Some(TOKEN),
        None,
    )
}

pub fn post(remote: &Remote, path: &str, body: &Value) -> Answer {
    call(
        "POST",
        &format!("{}{path}", remote.address()),
        Some(TOKEN),
        Some(body),
    )
}

/// A remote holding the Reading List published `count` times.
pub fn holding(count: u64) -> (Remote, History) {
    let history = history(count);
    let scenario =
        RemoteScenario::accepting(&history.key).holding("Reading List", history.states.clone());
    (Remote::start(scenario), history)
}

pub fn mark(state: &StoredState) -> Value {
    json!({ "publishSequence": state.sequence(), "stateRoot": state.lock["stateRoot"] })
}

/// Requires `answer` to be a problem document of `code` at `status`.
pub fn assert_problem(answer: &Answer, status: u16, code: &str) {
    assert_eq!(answer.status, status, "{:?}", answer.body);
    assert_eq!(answer.body["code"], code, "{:?}", answer.body);
    assert_eq!(answer.body["status"], status);
    assert_eq!(
        answer.header("content-type"),
        Some("application/problem+json")
    );
}

#[test]
fn the_simulator_records_each_request_as_it_was_sent_in_order() {
    let (remote, _) = holding(1);
    assert!(remote.requests().is_empty());
    get(&remote, "/head");
    let body = json!({ "documentKey": "x", "name": "y" });
    post(&remote, "/create", &body);
    let requests = remote.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        (requests[0].method.as_str(), requests[1].method.as_str()),
        ("GET", "POST")
    );
    assert!(requests[0].path.ends_with("/head"));
    assert_eq!(
        requests[0].header("authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    assert_eq!(requests[1].body, body.to_string().into_bytes());
}

#[test]
fn each_scenario_answers_as_it_names() {
    let key = "01a18e30-0000-7000-8000-000000000001";
    let failing = Remote::start(
        RemoteScenario::failing(key, 503, Some("service_unavailable")).retry_after("7"),
    );
    let answer = get(&failing, "/head");
    assert_problem(&answer, 503, "service_unavailable");
    assert_eq!(answer.header("retry-after"), Some("7"));
    let outside = Remote::start(RemoteScenario::failing(key, 418, Some("teapot")));
    assert_eq!(get(&outside, "/head").body["code"], "teapot");
    let plain = Remote::start(RemoteScenario::failing(key, 502, None));
    let answer = get(&plain, "/head");
    assert_eq!((answer.status, answer.body.clone()), (502, Value::Null));
    let refusing = Remote::start(RemoteScenario::refusing(key));
    assert!(
        agent()
            .get(format!("{}/head", refusing.address()))
            .call()
            .is_err()
    );
    let silent = Remote::start(RemoteScenario::silent(key));
    let quick: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(300)))
        .build()
        .into();
    assert!(
        quick
            .get(format!("{}/head", silent.address()))
            .call()
            .is_err()
    );
}

#[test]
fn a_request_is_admitted_only_with_the_scenario_s_token_or_one_the_simulator_issued() {
    let (remote, _) = holding(1);
    let url = format!("{}/head", remote.address());
    for token in [None, Some("another-token")] {
        let answer = call("GET", &url, token, None);
        assert_problem(&answer, 401, "unauthenticated");
        let challenge = answer.header("www-authenticate").unwrap();
        assert!(
            challenge.starts_with("Bearer resource_metadata=\""),
            "{challenge}"
        );
        assert!(
            challenge.contains("/.well-known/oauth-protected-resource/"),
            "{challenge}"
        );
        assert!(challenge.contains("scope=\""), "{challenge}");
    }
    let granted = remote.grant();
    let answer = call("GET", &url, Some(&granted.access_token), None);
    assert_eq!(answer.status, 200);
    assert_eq!(answer.header("content-type"), Some("application/json"));
    assert!(answer.header("set-cookie").is_none());
    assert_problem(&get(&remote, "/elsewhere"), 404, "not_found");
    assert_problem(
        &post(&remote, "/head", &json!({})),
        405,
        "method_not_allowed",
    );
}

#[test]
fn every_call_but_create_for_a_document_the_simulator_does_not_hold_is_not_found() {
    let key = "01a18e30-0000-7000-8000-000000000002";
    let remote = Remote::start(RemoteScenario::accepting(key));
    assert_problem(&get(&remote, "/head"), 404, "not_found");
    assert_problem(
        &post(&remote, "/publish", &json!({ "documentKey": key })),
        404,
        "not_found",
    );
    assert_problem(
        &post(
            &remote,
            "/import",
            &json!({ "documentKey": key, "states": [] }),
        ),
        404,
        "not_found",
    );
    let created = post(
        &remote,
        "/create",
        &json!({ "documentKey": key, "name": "Notes" }),
    );
    assert_eq!(created.status, 200);
}

#[test]
fn the_head_names_the_document_s_key_lock_and_file_digests_in_path_order() {
    let (remote, history) = holding(2);
    let answer = get(&remote, "/head");
    assert_eq!(answer.status, 200);
    assert_eq!(answer.body["documentKey"], history.key.as_str());
    let head = &answer.body["head"];
    let last = &history.states[1];
    assert_eq!(head["publishSequence"], 2);
    assert_eq!(head["stateRoot"], last.lock["stateRoot"]);
    assert_eq!(head["lock"], last.lock);
    let paths: Vec<&str> = head["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(paths, sorted);
    let manifest = &head["files"][paths
        .iter()
        .position(|p| *p == "spec/loftspec.yaml")
        .unwrap()];
    let digest =
        software_plant::remote::wire::sha256_hex(last.files["spec/loftspec.yaml"].as_bytes());
    assert_eq!(manifest["sha256"], digest.as_str());
    let empty = Remote::start(RemoteScenario::accepting(&history.key).holding("Empty", Vec::new()));
    assert_eq!(get(&empty, "/head").body["head"], Value::Null);
}

#[test]
fn a_published_state_and_a_draft_are_answered_with_their_files_or_not_found() {
    let history = history(2);
    let draft = history.states[0].files.clone();
    let scenario = RemoteScenario::accepting(&history.key)
        .holding("Reading List", history.states.clone())
        .draft("feat/a b", draft.clone());
    let remote = Remote::start(scenario);
    let state = get(&remote, "/states/1");
    assert_eq!(state.status, 200);
    assert_eq!(state.body["lock"], history.states[0].lock);
    let files = state.body["files"].as_array().unwrap();
    assert_eq!(files.len(), history.states[0].files.len());
    assert!(
        files
            .windows(2)
            .all(|pair| pair[0]["path"].as_str() < pair[1]["path"].as_str())
    );
    assert_problem(&get(&remote, "/states/9"), 404, "not_found");
    let answer = get(&remote, "/drafts/feat%2Fa%20b");
    assert_eq!(
        (answer.status, answer.body["branch"].clone()),
        (200, json!("feat/a b"))
    );
    assert_eq!(answer.body["files"].as_array().unwrap().len(), draft.len());
    assert_problem(&get(&remote, "/drafts/none"), 404, "not_found");
}

#[test]
fn every_refusal_is_a_problem_document_of_the_closed_set_with_more_than_the_contract_names() {
    let key = "01a18e30-0000-7000-8000-000000000006";
    for (code, status) in CODES {
        let answer = get(
            &Remote::start(RemoteScenario::failing(key, status, Some(code))),
            "/head",
        );
        assert_problem(&answer, status, code);
        assert!(
            answer.body["type"]
                .as_str()
                .is_some_and(|t| t != "about:blank")
        );
        assert!(answer.body["title"].is_string());
        assert!(
            answer.body.get("instance").is_some(),
            "a member the contract does not name"
        );
        if code == "rate_limited" {
            assert!(answer.header("retry-after").is_some());
        }
    }
}

#[test]
#[ignore = "reaches the remote at LOFT_REMOTE_CONTRACT_ADDRESS"]
fn a_real_remote_answers_each_scenario_in_the_shape_the_simulator_does() {
    let address = std::env::var("LOFT_REMOTE_CONTRACT_ADDRESS").expect("an address");
    let token = std::env::var("LOFT_REMOTE_CONTRACT_TOKEN").expect("a token");
    let shape = |answer: &Answer| {
        let body = &answer.body;
        (
            answer.status,
            body["code"].clone(),
            body.get("documentKey").is_some(),
            body.get("head").is_some(),
            answer
                .header("www-authenticate")
                .is_some_and(|c| c.contains("resource_metadata=")),
        )
    };
    let (remote, _) = holding(1);
    for (path, bearer) in [
        ("/head", true),
        ("/head", false),
        ("/elsewhere", true),
        ("/states/999999", true),
    ] {
        let real = call(
            "GET",
            &format!("{address}{path}"),
            bearer.then_some(token.as_str()),
            None,
        );
        let simulated = call(
            "GET",
            &format!("{}{path}", remote.address()),
            bearer.then_some(TOKEN),
            None,
        );
        assert_eq!(shape(&real), shape(&simulated), "{path} {bearer}");
    }
}
