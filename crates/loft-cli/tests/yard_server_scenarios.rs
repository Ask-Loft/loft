//! The scenarios a test scripts the Yard Server simulator with, beyond Yard Server's contract:
//! asking to come back later, refusing or putting off named versions, and answering with a
//! body the test names. A real Yard Server cannot be scripted, so these run against the
//! simulator alone; the contract suite holds what both must answer alike.

use serde_json::{Value, json};
use software_plant::YardServer;
use software_plant::yard_server::{FORMAT, TOKEN};
use yardlog::Key;

use crate::common::calls::agent;
use crate::yard_server_contract::{Target, fresh, one, outcomes, record};

/// The keys of the versions of `batch`, in its order.
fn keys(batch: &Value) -> Vec<String> {
    batch["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| record["version"]["key"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn a_simulator_that_asks_to_come_back_answers_its_status_and_retry_after() {
    for (status, code, retry_after) in [
        (429, "rate_limited", "120"),
        (503, "service_unavailable", "Wed, 21 Oct 2026 07:28:00 GMT"),
    ] {
        let server = YardServer::coming_back(status, retry_after);
        let answer = Target::simulated(&server).post(&fresh(1));
        assert_eq!(answer.status, status);
        assert_eq!(answer.body["code"], code);
        assert_eq!(answer.header("retry-after"), Some(retry_after));
        assert_eq!(server.records(), Vec::<Value>::new());
    }
}

#[test]
fn a_simulator_refusing_named_versions_refuses_them_and_stores_the_others() {
    let batch = fresh(3);
    let keys = keys(&batch);
    let server = YardServer::refusing_versions(&[(&keys[1], "too_old")]);
    let answer = Target::simulated(&server).post(&batch);
    assert_eq!(answer.status, 200);
    assert_eq!(outcomes(&answer), ["stored", "refused too_old", "stored"]);
    let held: Vec<Value> = server
        .records()
        .iter()
        .map(|record| record["version"]["key"].clone())
        .collect();
    assert_eq!(held, [json!(keys[0]), json!(keys[2])]);
}

#[test]
fn a_simulator_putting_named_versions_off_answers_them_later_and_stores_none_of_them() {
    let batch = fresh(2);
    let keys = keys(&batch);
    let server = YardServer::putting_off(&[(&keys[0], "unknown_report_version")]);
    let target = Target::simulated(&server);
    let answer = target.post(&batch);
    assert_eq!(
        outcomes(&answer),
        ["later unknown_report_version", "stored"]
    );
    assert_eq!(server.records(), vec![batch["records"][1].clone()]);
    let again = target.post(&batch);
    assert_eq!(outcomes(&again), ["later unknown_report_version", "held"]);
}

#[test]
fn a_simulator_answering_with_a_named_body_answers_exactly_it() {
    for body in [
        "not json at all",
        r#"{"format":"yardlog-answer/1","outcomes":[]}"#,
    ] {
        let server = YardServer::answering(body);
        let answer = agent()
            .post(Target::simulated(&server).batches())
            .header("Authorization", format!("Bearer {TOKEN}"))
            .header("Content-Type", "application/json")
            .send(fresh(1).to_string())
            .expect("the simulator answers");
        assert_eq!(answer.status(), 200);
        assert_eq!(answer.into_body().read_to_string().unwrap(), body);
        assert_eq!(server.records(), Vec::<Value>::new());
    }
}

#[test]
fn a_version_sent_under_another_yard_log_leaves_one_record_under_the_first() {
    let server = YardServer::accepting();
    let target = Target::simulated(&server);
    let version = Key::mint().to_string();
    let first = record(&Key::mint().to_string(), &version, "a");
    target.post(&one(&first));
    target.post(&one(&record(&Key::mint().to_string(), &version, "a")));
    assert_eq!(server.records(), vec![first]);
}

#[test]
fn a_version_sent_again_with_other_contents_leaves_the_first_record_as_it_was() {
    let server = YardServer::accepting();
    let target = Target::simulated(&server);
    let (yard_log, version) = (Key::mint().to_string(), Key::mint().to_string());
    let first = record(&yard_log, &version, "first");
    target.post(&one(&first));
    let answer = target.post(&one(&record(&yard_log, &version, "second")));
    assert_eq!(outcomes(&answer), ["conflict"]);
    assert_eq!(server.records(), vec![first]);
}

#[test]
fn a_record_that_does_not_read_is_refused_and_the_others_are_stored() {
    let server = YardServer::accepting();
    let batch = fresh(2);
    let mut records = batch["records"].as_array().unwrap().clone();
    records.insert(
        1,
        json!({ "record": "version", "version": { "key": Key::mint().to_string() } }),
    );
    let answer = Target::simulated(&server).post(&json!({ "format": FORMAT, "records": records }));
    assert_eq!(
        outcomes(&answer),
        ["stored", "refused invalid_record", "stored"]
    );
    assert_eq!(
        server.records(),
        batch["records"].as_array().unwrap().clone()
    );
}
