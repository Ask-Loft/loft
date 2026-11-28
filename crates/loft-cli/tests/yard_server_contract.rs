//! The contract suite of the Yard Server simulator: each scenario asks the simulator, and on
//! demand a real Yard Server, the same question and requires the same answer. The real half is
//! ignored by default; run it against a Yard Server with `LOFT_YARD_SERVER_CONTRACT_ADDRESS` and
//! `LOFT_YARD_SERVER_CONTRACT_TOKEN` set:
//! `cargo nextest run -p loft-cli --run-ignored only -E 'test(/^yard_server_contract::/)'`.
//! Every version it sends has a key minted for the run, so a real server that keeps what it
//! stored answers each run as a fresh one.

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use software_plant::YardServer;
use software_plant::yard_server::{ANSWER_FORMAT, BATCHES, FORMAT, MOST_BYTES, TOKEN};
use yardlog::Key;

use crate::common::calls::{Answer, agent, call, read};

/// A Yard Server to ask: its base address and the token it admits.
pub(crate) struct Target {
    address: String,
    token: String,
}

impl Target {
    pub(crate) fn simulated(server: &YardServer) -> Self {
        Self {
            address: server.url(),
            token: TOKEN.to_owned(),
        }
    }

    fn real() -> Self {
        let variable = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| panic!("{name} names the real Yard Server"))
        };
        Self {
            address: variable("LOFT_YARD_SERVER_CONTRACT_ADDRESS"),
            token: variable("LOFT_YARD_SERVER_CONTRACT_TOKEN"),
        }
    }

    pub(crate) fn batches(&self) -> String {
        format!("{}{BATCHES}", self.address)
    }

    pub(crate) fn post(&self, batch: &Value) -> Answer {
        call("POST", &self.batches(), Some(&self.token), Some(batch))
    }
}

/// A record of a note's version, in Yard Log `yard_log`, holding `markdown`.
pub(crate) fn record(yard_log: &str, version: &str, markdown: &str) -> Value {
    json!({
        "record": "version",
        "yardLog": yard_log,
        "stage": Key::mint().to_string(),
        "block": Key::mint().to_string(),
        "kind": "note",
        "id": "contract",
        "version": {
            "key": version,
            "at": "2026-01-01T00:00:00Z",
            "revision": 1,
            "by": { "host": "contract", "session": "s1" },
            "markdown": markdown,
        },
    })
}

/// A batch of the one record `record`.
pub(crate) fn one(record: &Value) -> Value {
    json!({ "format": FORMAT, "records": [record] })
}

/// A batch of `count` fresh versions in one Yard Log.
pub(crate) fn fresh(count: usize) -> Value {
    let yard_log = Key::mint().to_string();
    let records: Vec<Value> = (0..count)
        .map(|n| record(&yard_log, &Key::mint().to_string(), &format!("note {n}")))
        .collect();
    json!({ "format": FORMAT, "records": records })
}

/// A batch of one version whose body is larger than 1 MiB.
fn too_large() -> Value {
    let note = "x".repeat(MOST_BYTES);
    one(&record(
        &Key::mint().to_string(),
        &Key::mint().to_string(),
        &note,
    ))
}

/// The status of a refusal and the code of its problem document, where it is one.
type Refusal = (u16, Option<String>);

/// The status of an answer and the code of its problem document, where it is one.
fn refusal(answer: &Answer) -> Refusal {
    let code = answer.body["code"].as_str().map(str::to_owned);
    (answer.status, code)
}

/// `request` sent with `body` as it is, read as an answer.
fn send(request: ureq::http::request::Builder, body: &str) -> Answer {
    read(
        agent()
            .run(request.body(body.to_owned()).unwrap())
            .expect("Yard Server answers"),
    )
}

/// The answers each scenario gets from `target`, in a shape the simulator and a real Yard
/// Server must agree on.
fn scenarios(target: &Target) -> Vec<(&'static str, Value)> {
    vec![
        ("takes a batch", json!(takes_a_batch(target))),
        ("holds a version once", json!(holds_a_version_once(target))),
        (
            "holds a version under the first Yard Log",
            json!(keeps_the_first_yard_log(target)),
        ),
        (
            "keeps the first contents",
            json!(keeps_the_first_contents(target)),
        ),
        (
            "refuses a record that does not read",
            json!(refuses_a_record_that_does_not_read(target)),
        ),
        ("refuses", json!(refusals(target))),
    ]
}

/// The outcomes of an answer of `200`, each as its outcome and its code where it has one.
pub(crate) fn outcomes(answer: &Answer) -> Vec<String> {
    answer.body["outcomes"]
        .as_array()
        .map(|outcomes| {
            outcomes
                .iter()
                .map(|outcome| {
                    let word = outcome["outcome"].as_str().unwrap_or("?");
                    match outcome["code"].as_str() {
                        Some(code) => format!("{word} {code}"),
                        None => word.to_owned(),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Whether the answer names the format of an answer and each version of `batch`, in its order.
fn names_each_version(answer: &Answer, batch: &Value) -> bool {
    let sent: Vec<&Value> = batch["records"]
        .as_array()
        .map(|records| {
            records
                .iter()
                .map(|record| &record["version"]["key"])
                .collect()
        })
        .unwrap_or_default();
    let named: Vec<&Value> = answer.body["outcomes"]
        .as_array()
        .map(|outcomes| outcomes.iter().map(|outcome| &outcome["version"]).collect())
        .unwrap_or_default();
    answer.body["format"] == ANSWER_FORMAT && named == sent
}

/// The status of a fresh batch of two, its outcomes, and whether it names each version in order.
fn takes_a_batch(target: &Target) -> (u16, Vec<String>, bool) {
    let batch = fresh(2);
    let answer = target.post(&batch);
    (
        answer.status,
        outcomes(&answer),
        names_each_version(&answer, &batch),
    )
}

/// The statuses and outcomes of one batch sent twice.
fn holds_a_version_once(target: &Target) -> (u16, Vec<String>, u16, Vec<String>) {
    let batch = fresh(3);
    let first = target.post(&batch);
    let second = target.post(&batch);
    (
        first.status,
        outcomes(&first),
        second.status,
        outcomes(&second),
    )
}

/// The status and outcome of a version sent again under another Yard Log.
fn keeps_the_first_yard_log(target: &Target) -> (u16, Vec<String>) {
    let version = Key::mint().to_string();
    let (first, second) = (Key::mint().to_string(), Key::mint().to_string());
    target.post(&one(&record(&first, &version, "a")));
    let answer = target.post(&one(&record(&second, &version, "a")));
    (answer.status, outcomes(&answer))
}

/// The outcome of a version sent again with other contents, then with its first contents.
fn keeps_the_first_contents(target: &Target) -> (u16, Vec<String>, Vec<String>) {
    let (yard_log, version) = (Key::mint().to_string(), Key::mint().to_string());
    let first = record(&yard_log, &version, "first");
    target.post(&one(&first));
    let other = target.post(&one(&record(&yard_log, &version, "second")));
    let again = target.post(&one(&first));
    (other.status, outcomes(&other), outcomes(&again))
}

/// The status and outcomes of a batch whose second record holds a version's key but is not a
/// record of a batch of versions.
fn refuses_a_record_that_does_not_read(target: &Target) -> (u16, Vec<String>) {
    let batch = fresh(1);
    let mut records = batch["records"].as_array().unwrap().clone();
    records.push(json!({
        "record": "something-else",
        "version": { "key": Key::mint().to_string() },
    }));
    let answer = target.post(&json!({ "format": FORMAT, "records": records }));
    (answer.status, outcomes(&answer))
}

/// The status and code of each request Yard Server refuses.
fn refusals(target: &Target) -> Vec<(&'static str, Refusal)> {
    let url = target.batches();
    let bearer = format!("Bearer {}", target.token);
    let post = || {
        ureq::http::Request::builder()
            .method("POST")
            .uri(&url)
            .header("Authorization", &bearer)
    };
    let json = || post().header("Content-Type", "application/json");
    let empty = json!({ "format": FORMAT, "records": [] });
    let keyless = json!({ "format": FORMAT, "records": [{ "record": "version" }] });
    vec![
        (
            "no token",
            refusal(&call("POST", &url, None, Some(&fresh(1)))),
        ),
        (
            "another token",
            refusal(&call("POST", &url, Some("not-the-token"), Some(&fresh(1)))),
        ),
        ("not JSON", refusal(&send(json(), "{not json"))),
        (
            "another format",
            refusal(&target.post(&json!({ "format": "yardlog/1", "records": [] }))),
        ),
        ("no record", refusal(&target.post(&empty))),
        ("17 records", refusal(&target.post(&fresh(17)))),
        ("over 1 MiB", refusal(&target.post(&too_large()))),
        ("a record without a key", refusal(&target.post(&keyless))),
        (
            "another method",
            refusal(&call("GET", &url, Some(&target.token), None)),
        ),
        (
            "another content type",
            refusal(&send(
                post().header("Content-Type", "text/plain"),
                &fresh(1).to_string(),
            )),
        ),
        (
            "another path",
            refusal(&call(
                "POST",
                &format!("{}/elsewhere", target.address),
                Some(&target.token),
                Some(&fresh(1)),
            )),
        ),
    ]
}

/// The answers the contract requires of every scenario.
fn required() -> Vec<(&'static str, Value)> {
    let code = |status: u16, code: &str| json!([status, code]);
    vec![
        ("takes a batch", json!([200, ["stored", "stored"], true])),
        (
            "holds a version once",
            json!([
                200,
                ["stored", "stored", "stored"],
                200,
                ["held", "held", "held"]
            ]),
        ),
        (
            "holds a version under the first Yard Log",
            json!([200, ["held"]]),
        ),
        (
            "keeps the first contents",
            json!([200, ["conflict"], ["held"]]),
        ),
        (
            "refuses a record that does not read",
            json!([200, ["stored", "refused invalid_record"]]),
        ),
        (
            "refuses",
            json!([
                ["no token", code(401, "unauthenticated")],
                ["another token", code(401, "unauthenticated")],
                ["not JSON", code(400, "invalid_request")],
                ["another format", code(400, "invalid_request")],
                ["no record", code(400, "invalid_request")],
                ["17 records", code(400, "invalid_request")],
                ["over 1 MiB", code(413, "payload_too_large")],
                ["a record without a key", code(400, "invalid_request")],
                ["another method", code(405, "method_not_allowed")],
                ["another content type", code(415, "unsupported_media_type")],
                ["another path", code(404, "not_found")],
            ]),
        ),
    ]
}

#[test]
fn the_simulator_answers_each_scenario_as_the_contract_requires() {
    let server = YardServer::accepting();
    assert_eq!(scenarios(&Target::simulated(&server)), required());
}

#[test]
fn the_simulator_holds_each_version_once_in_the_order_first_received() {
    let server = YardServer::accepting();
    let target = Target::simulated(&server);
    let (first, second) = (fresh(2), fresh(1));
    target.post(&first);
    target.post(&second);
    target.post(&first);
    let mut sent = first["records"].as_array().unwrap().clone();
    sent.extend(second["records"].as_array().unwrap().clone());
    assert_eq!(server.records(), sent);
}

#[test]
fn a_refused_request_stores_nothing() {
    let server = YardServer::accepting();
    let target = Target::simulated(&server);
    refusals(&target);
    assert_eq!(server.records(), Vec::<Value>::new());
}

#[test]
fn the_simulator_records_each_request_in_the_order_received() {
    let server = YardServer::accepting();
    assert_eq!(server.requests().len(), 0);
    let target = Target::simulated(&server);
    let batch = fresh(1);
    target.post(&batch);
    call("GET", &target.batches(), None, None);
    let requests = server.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| (request.method.as_str(), request.path.as_str()))
            .collect::<Vec<_>>(),
        [("POST", BATCHES), ("GET", BATCHES)]
    );
    assert_eq!(
        requests[0].header("Authorization"),
        Some(format!("Bearer {TOKEN}").as_str())
    );
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body, batch);
}

#[test]
fn a_failing_simulator_answers_its_status_as_a_problem_document() {
    let server = YardServer::failing(503);
    let answer = Target::simulated(&server).post(&fresh(1));
    assert_eq!(answer.status, 503);
    assert_eq!(
        answer.header("content-type"),
        Some("application/problem+json")
    );
    assert_eq!(server.records(), Vec::<Value>::new());
}

#[test]
fn a_failing_simulator_names_the_code_the_contract_gives_its_status() {
    for (status, code) in [
        (401, "unauthenticated"),
        (503, "service_unavailable"),
        (500, "internal_error"),
    ] {
        let server = YardServer::failing(status);
        let answer = Target::simulated(&server).post(&fresh(1));
        assert_eq!(refusal(&answer), (status, Some(code.to_owned())));
    }
}

#[test]
fn a_silent_simulator_never_answers_and_a_refusing_one_refuses() {
    let server = YardServer::silent();
    let quiet: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(300)))
        .build()
        .into();
    let batches = Target::simulated(&server).batches();
    assert!(
        quiet
            .post(&batches)
            .header("Content-Type", "application/json")
            .send(fresh(1).to_string())
            .is_err()
    );
    let refused = agent()
        .post(format!("{}{BATCHES}", YardServer::refusing()))
        .header("Content-Type", "application/json")
        .send(fresh(1).to_string());
    assert!(refused.is_err(), "{refused:?}");
}

#[test]
fn a_hanging_up_simulator_stores_the_batch_and_sends_no_answer() {
    let server = YardServer::hanging_up();
    let batch = fresh(2);
    let sent = agent()
        .post(Target::simulated(&server).batches())
        .header("Authorization", format!("Bearer {TOKEN}"))
        .header("Content-Type", "application/json")
        .send(batch.to_string());
    assert!(sent.is_err(), "{sent:?}");
    assert_eq!(&Value::from(server.records()), &batch["records"]);
}

#[test]
fn a_late_simulator_stores_the_batch_and_answers_after_its_delay() {
    let delay = Duration::from_millis(400);
    let server = YardServer::late(delay);
    let batch = fresh(1);
    let started = Instant::now();
    let answer = Target::simulated(&server).post(&batch);
    assert!(started.elapsed() >= delay);
    assert_eq!(answer.status, 200);
    assert_eq!(outcomes(&answer), ["stored"]);
    assert_eq!(&Value::from(server.records()), &batch["records"]);
}

#[test]
#[ignore = "reaches a real Yard Server"]
fn a_real_yard_server_answers_each_scenario_as_the_simulator_does() {
    let server = YardServer::accepting();
    assert_eq!(
        scenarios(&Target::real()),
        scenarios(&Target::simulated(&server))
    );
}
