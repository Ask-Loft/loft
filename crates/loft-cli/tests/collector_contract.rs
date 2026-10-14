//! The contract suite of the collector simulator: each scenario sends the same request to the
//! simulator and to the real telemetry collector and requires the same answer. The real half is
//! ignored by default, since it reaches a service on the network; run it, against the
//! collector at `LOFT_COLLECTOR_CONTRACT_URL`, with
//! `cargo nextest run -p loft-cli --run-ignored only -E 'test(/collector_contract/)'`. The
//! collector is built to pass this suite.

use std::env;
use std::time::Duration;

use crate::common::telemetry::client;
use serde_json::{Value, json};
use software_plant::Collector;
use software_plant::collector::LIMIT;
use ureq::Agent;

/// What the suite compares of an answer: its status, whether it sets a cookie, and whether its
/// body is empty.
#[derive(Debug, PartialEq, Eq)]
struct Answer {
    status: u16,
    sets_cookie: bool,
    empty: bool,
}

fn answer(response: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Answer {
    let mut response = response.expect("the collector answers");
    let sets_cookie = response.headers().contains_key("set-cookie");
    let body = response.body_mut().read_to_vec().unwrap_or_default();
    Answer {
        status: response.status().as_u16(),
        sets_cookie,
        empty: body.is_empty(),
    }
}

fn post(url: &str, content_type: &str, body: &[u8]) -> Answer {
    answer(
        client()
            .post(url)
            .header("Content-Type", content_type)
            .send(body),
    )
}

/// A valid command event with a fresh event id.
fn event() -> Value {
    json!({
        "schema": "loft-telemetry-event/1",
        "eventId": uuid::Uuid::new_v4().to_string(),
        "version": env!("CARGO_PKG_VERSION"),
        "os": "linux",
        "arch": "x86_64",
        "installedBy": "cargo",
        "time": "2026-10-14T09:00:00Z",
        "installation": uuid::Uuid::new_v4().to_string(),
        "project": uuid::Uuid::new_v4().to_string(),
        "branch": "9c41e07a5b2fd813",
        "mode": "internal",
        "agent": "unknown",
        "command": "check",
        "outcome": "passed",
        "errorKinds": [],
        "duration": "under-1s",
    })
}

/// A scenario: its name, and the answer to its request at a collector's address.
type Scenario = (&'static str, fn(&str) -> Answer);

/// Each scenario of the contract.
const SCENARIOS: [Scenario; 7] = [
    ("a valid event", |url| {
        post(url, "application/json", event().to_string().as_bytes())
    }),
    ("the same event twice", |url| {
        let body = event().to_string();
        post(url, "application/json", body.as_bytes());
        post(url, "application/json", body.as_bytes())
    }),
    ("an event with a field beyond the schema", |url| {
        let mut extra = event();
        extra["hostname"] = json!("build-01");
        post(url, "application/json", extra.to_string().as_bytes())
    }),
    ("an event with a value outside its set", |url| {
        let mut wrong = event();
        wrong["mode"] = json!("batch");
        post(url, "application/json", wrong.to_string().as_bytes())
    }),
    ("a body larger than the limit", |url| {
        let mut large = event();
        large["outcome"] = json!("x".repeat(LIMIT));
        post(url, "application/json", large.to_string().as_bytes())
    }),
    ("another content type", |url| {
        post(url, "text/plain", event().to_string().as_bytes())
    }),
    ("a method other than POST", |url| {
        answer(client().get(url).call())
    }),
];

/// The answer the contract states for each scenario.
const EXPECTED: [u16; 7] = [202, 202, 400, 400, 413, 415, 405];

fn answers(url: &str) -> Vec<(&'static str, Answer)> {
    SCENARIOS
        .iter()
        .map(|(name, scenario)| (*name, scenario(url)))
        .collect()
}

#[test]
fn the_simulator_answers_each_scenario_as_the_contract_states() {
    let collector = Collector::accepting();
    for ((name, answer), status) in answers(&collector.url()).into_iter().zip(EXPECTED) {
        assert_eq!(answer.status, status, "{name}");
        assert!(!answer.sets_cookie, "{name}");
        if status == 202 {
            assert!(answer.empty, "{name}");
        }
    }
}

#[test]
fn the_simulator_records_an_event_sent_twice_once() {
    let collector = Collector::accepting();
    let body = event().to_string();
    post(&collector.url(), "application/json", body.as_bytes());
    post(&collector.url(), "application/json", body.as_bytes());
    assert_eq!(collector.events(), vec![body.into_bytes()]);
}

#[test]
fn the_simulator_records_events_in_the_order_received() {
    let collector = Collector::accepting();
    let bodies: Vec<String> = (0..3).map(|_| event().to_string()).collect();
    for body in &bodies {
        post(&collector.url(), "application/json", body.as_bytes());
    }
    let expected: Vec<Vec<u8>> = bodies.into_iter().map(String::into_bytes).collect();
    assert_eq!(collector.events(), expected);
}

#[test]
fn the_simulator_records_no_event_it_refuses() {
    let collector = Collector::accepting();
    let _ = answers(&collector.url());
    let valid = collector.events();
    // Only the valid event and the event sent twice are recorded, each once.
    assert_eq!(valid.len(), 2, "{valid:?}");
}

#[test]
fn a_failing_simulator_answers_every_request_with_its_status() {
    let collector = Collector::failing(503);
    let sent = post(
        &collector.url(),
        "application/json",
        event().to_string().as_bytes(),
    );
    assert_eq!(sent.status, 503);
    assert_eq!(collector.events(), Vec::<Vec<u8>>::new());
}

#[test]
fn a_refusing_collector_refuses_the_connection() {
    let refused = client()
        .post(&Collector::refusing())
        .header("Content-Type", "application/json")
        .send(event().to_string().as_bytes());
    assert!(refused.is_err(), "{refused:?}");
}

#[test]
fn a_silent_simulator_accepts_the_connection_and_never_answers() {
    let collector = Collector::silent();
    let quiet: Agent = Agent::config_builder()
        .timeout_global(Some(Duration::from_millis(300)))
        .build()
        .into();
    let sent = quiet
        .post(&collector.url())
        .header("Content-Type", "application/json")
        .send(event().to_string().as_bytes());
    assert!(sent.is_err(), "{sent:?}");
    assert_eq!(collector.requests().len(), 1);
}

#[test]
#[ignore = "reaches the real collector at LOFT_COLLECTOR_CONTRACT_URL"]
fn the_real_collector_answers_each_scenario_as_the_simulator_does() {
    let real = env::var("LOFT_COLLECTOR_CONTRACT_URL")
        .expect("LOFT_COLLECTOR_CONTRACT_URL names the collector to hold to the contract");
    let simulator = Collector::accepting();
    let expected = answers(&simulator.url());
    for ((name, real), (_, simulated)) in answers(&real).into_iter().zip(expected) {
        assert_eq!(real, simulated, "{name}");
    }
}
