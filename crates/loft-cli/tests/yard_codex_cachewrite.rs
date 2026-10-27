//! Source reports survive while ambiguous writes stay out of measured totals, including replay.
use crate::common::workspace;
use crate::schema;
use crate::served::Served;
use crate::yard::{ASK, BRANCH, Yard};
use crate::yard_serve_events::{event, post, session_file};
use serde_json::{Value, json};
use std::fs;

fn request(yard: &Yard, second: u32, account: Option<&str>, write: u64) -> Value {
    let mut row = event(
        yard,
        "request",
        second,
        &json!({"host":"codex","model":"fixture",
        "tokens":{"input":70,"cacheRead":30,"cacheWrite":write,"output":5}}),
    );
    if let Some(account) = account {
        row["usageSource"] = json!({"account":account});
    }
    row
}

fn read(yard: &Yard, served: &Served) -> Value {
    let tree = yard.scratch.0.canonicalize().unwrap().display().to_string();
    let reply = served.get(&format!("/api/log?tree={tree}&branch={BRANCH}"));
    let schema = fs::read_to_string(workspace().join("schemas/yard-read.json")).unwrap();
    schema::conforming(&reply.body, &schema)
}

fn start() -> (Yard, Served) {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--host", "codex", "--send"]);
    let served = Served::start(&yard, &["--tree", yard.scratch.0.to_str().unwrap()]);
    (yard, served)
}

#[test]
fn subscription_zeros_stay_in_raw_events_but_are_absent_from_every_measured_breakdown() {
    let (yard, served) = start();
    let mut row = request(&yard, 1, Some("chatgpt"), 0);
    row["agent"] = json!({"id":"reviewer","type":"review"});
    post(&served, &[row]);
    let doc = read(&yard, &served);
    let totals = &doc["tokens"][0];
    assert_eq!(totals["cacheWriteClassified"], true);
    for counts in [
        &totals["tokens"],
        &totals["stages"][0],
        &totals["stages"][0]["agents"][0],
        &totals["agents"][0],
        &totals["submissions"][0],
    ] {
        assert!(counts.get("cacheWrite").is_none(), "{counts}");
        assert_eq!(counts["input"], 70);
        assert_eq!(counts["cacheRead"], 30);
        assert_eq!(counts["output"], 5);
    }
    let raw = fs::read_to_string(yard.scratch.home().join(".yard/events/codex~s1.jsonl")).unwrap();
    let kept: Value = serde_json::from_str(raw.lines().next().unwrap()).unwrap();
    assert_eq!(kept["tokens"]["cacheWrite"], 0);
    assert_eq!(kept["usageSource"]["account"], "chatgpt");
    assert_eq!(doc["revision"], 1);
}

#[test]
fn replay_preserves_known_api_zeros_and_positive_counts_without_counting_legacy_zeros() {
    let (yard, served) = start();
    post(&served, &[request(&yard, 1, None, 0)]);
    drop(served);
    let again = Served::start(&yard, &[]);
    post(&again, &[request(&yard, 2, Some("apiKey"), 0)]);
    let doc = read(&yard, &again);
    assert_eq!(doc["tokens"][0]["tokens"]["cacheWrite"], 0);
    assert_eq!(doc["tokens"][0]["tokens"]["partial"], json!(["cacheWrite"]));
    post(&again, &[request(&yard, 3, Some("chatgpt"), 7)]);
    let doc = read(&yard, &again);
    assert_eq!(doc["tokens"][0]["tokens"]["cacheWrite"], 7);
    assert_eq!(doc["tokens"][0]["tokens"]["input"], 210);
    assert!(
        session_file(&yard, "tokens", "codex~s1")
            .unwrap()
            .contains("cacheWriteClassified: true")
    );
}
