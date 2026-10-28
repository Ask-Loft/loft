//! Identified event delivery is idempotent within a host and native session.

use std::fs;

use serde_json::{Value, json};

use crate::served::Served;
use crate::yard::Yard;
use crate::yard_serve_events::{event, open, post, session_file, tokens};

fn request(yard: &Yard, id: &str, input: u64) -> Value {
    event(
        yard,
        "request",
        5,
        &json!({"eventId":id,"model":"fixture","tokens":tokens(input, 2, 0, 3)}),
    )
}

fn recorded(yard: &Yard, session: &str) -> usize {
    fs::read_to_string(
        yard.scratch
            .home()
            .join(".yard/events")
            .join(format!("{session}.jsonl")),
    )
    .unwrap_or_default()
    .lines()
    .count()
}

fn input_is(yard: &Yard, session: &str, input: u64) {
    let text = session_file(yard, "tokens", session).unwrap();
    assert!(
        text.contains(&format!("tokens:\n  input: {input}\n")),
        "{text}"
    );
}

#[test]
fn identical_identified_requests_are_acknowledged_and_counted_once() {
    let (yard, served) = open();
    let sample = request(&yard, "sample-one", 7);
    for batch in [vec![sample.clone(), sample.clone()], vec![sample]] {
        let reply = post(&served, &batch);
        assert_eq!(reply["rejected"], json!([]));
    }
    assert_eq!(recorded(&yard, "s1"), 1);
    input_is(&yard, "s1", 7);
}

#[test]
fn conflicting_reuse_of_an_event_identifier_is_refused_without_changing_totals() {
    let (yard, served) = open();
    post(&served, &[request(&yard, "sample-one", 7)]);
    let reply = post(&served, &[request(&yard, "sample-one", 9)]);
    assert_eq!(reply["rejected"][0]["index"], 0);
    assert!(
        reply["rejected"][0]["message"]
            .as_str()
            .unwrap()
            .contains("eventId")
    );
    assert_eq!(recorded(&yard, "s1"), 1);
    input_is(&yard, "s1", 7);
}

#[test]
fn a_lost_acknowledgement_can_be_retried_after_a_page_server_restart() {
    let (yard, served) = open();
    let sample = request(&yard, "sample-one", 7);
    let _ = post(&served, std::slice::from_ref(&sample));
    drop(served);
    let served = Served::start(&yard, &[]);
    let reply = post(&served, &[sample, request(&yard, "sample-two", 3)]);
    assert_eq!(reply["rejected"], json!([]));
    assert_eq!(recorded(&yard, "s1"), 2);
    input_is(&yard, "s1", 10);
}

#[test]
fn an_identified_retry_repairs_derived_totals_after_replay_without_another_charge() {
    let (yard, served) = open();
    let sample = request(&yard, "recover-derived-totals", 7);
    post(&served, std::slice::from_ref(&sample));
    drop(served);
    fs::remove_file(yard.folder(crate::yard::BRANCH).join("tokens/s1.yaml")).unwrap();
    let served = Served::start(&yard, &[]);
    post(&served, &[sample]);
    assert_eq!(recorded(&yard, "s1"), 1);
    input_is(&yard, "s1", 7);
}

#[test]
fn moving_an_identified_payload_to_another_tree_does_not_escape_its_session_receipt() {
    let (yard, served) = open();
    let other = Yard::new();
    other.write("s1", crate::yard::ASK, &[]);
    let mut sample = request(&yard, "same-session-receipt", 7);
    post(&served, std::slice::from_ref(&sample));
    sample["dir"] = json!(other.scratch.0.canonicalize().unwrap());
    let reply = post(&served, &[sample]);
    assert_eq!(reply["rejected"][0]["index"], 0);
    assert!(session_file(&other, "tokens", "s1").is_none());
    assert_eq!(recorded(&yard, "s1"), 1);
}

#[test]
fn an_event_identifier_is_independent_between_hosts_and_sessions() {
    let (yard, served) = open();
    let first = request(&yard, "same-identifier", 7);
    let mut other_host = first.clone();
    other_host["host"] = json!("codex");
    let mut other_session = first.clone();
    other_session["session"] = json!("s2");
    let reply = post(&served, &[first, other_host, other_session]);
    assert_eq!(reply["rejected"], json!([]));
    for session in ["s1", "codex~s1", "s2"] {
        assert_eq!(recorded(&yard, session), 1);
        input_is(&yard, session, 7);
    }
}

#[test]
fn concurrent_identified_retries_append_and_count_only_once() {
    let (yard, served) = open();
    let sample = request(&yard, "concurrent-sample", 7);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                let reply = post(&served, std::slice::from_ref(&sample));
                assert_eq!(reply["rejected"], json!([]));
            });
        }
    });
    assert_eq!(recorded(&yard, "s1"), 1);
    input_is(&yard, "s1", 7);
}

#[test]
fn missing_event_identifiers_retain_legacy_append_behavior() {
    let (yard, served) = open();
    let mut sample = request(&yard, "legacy", 7);
    sample.as_object_mut().unwrap().remove("eventId");
    post(&served, &[sample.clone(), sample]);
    assert_eq!(recorded(&yard, "s1"), 2);
    input_is(&yard, "s1", 14);
}

#[test]
fn empty_or_non_string_event_identifiers_change_no_event_log() {
    let (yard, served) = open();
    for invalid in [json!(""), Value::Null, json!(1), json!([]), json!({})] {
        let mut sample = request(&yard, "invalid", 7);
        sample["eventId"] = invalid;
        let reply = post(&served, &[sample]);
        assert_eq!(reply["rejected"][0]["index"], 0);
    }
    assert_eq!(recorded(&yard, "s1"), 0);
    assert!(session_file(&yard, "tokens", "s1").is_none());
}

#[test]
fn an_identified_line_kept_without_its_digest_gives_no_receipt_after_a_restart() {
    let (yard, served) = open();
    let sample = request(&yard, "kept-without-digest", 7);
    post(&served, std::slice::from_ref(&sample));
    drop(served);
    let log = yard.scratch.home().join(".yard/events/s1.jsonl");
    let mut line: Value = serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    line.as_object_mut().unwrap().remove("eventDigest").unwrap();
    fs::write(&log, format!("{line}\n")).unwrap();
    let served = Served::start(&yard, &[]);
    let reply = post(&served, &[sample]);
    assert_eq!(reply["rejected"], json!([]));
    assert_eq!(recorded(&yard, "s1"), 2);
    input_is(&yard, "s1", 14);
}
