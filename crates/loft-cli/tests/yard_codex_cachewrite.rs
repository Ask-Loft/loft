//! The page server counts each request's tokens as the adapter gives them, including on replay,
//! keeps the source report in the raw event, writes each totals file in its totals format, and
//! leaves a totals file of a newer format as it was.
use crate::common::workspace;
use crate::schema;
use crate::served::Served;
use crate::yard::{ASK, BRANCH, Yard};
use crate::yard_serve_events::{event, post, session_file};
use serde_json::{Value, json};
use std::fs;

/// A Codex request as the adapter posts it: `write` in its tokens where it is measured, and
/// `source` as its `usageSource`.
pub(crate) fn request(
    yard: &Yard,
    second: u32,
    source: Option<Value>,
    write: Option<u64>,
) -> Value {
    let mut tokens = json!({"input":70,"cacheRead":30,"output":5});
    if let Some(write) = write {
        tokens["cacheWrite"] = json!(write);
    }
    let mut row = event(
        yard,
        "request",
        second,
        &json!({"host":"codex","model":"fixture","tokens":tokens}),
    );
    if let Some(source) = source {
        row["usageSource"] = source;
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
    let reported = json!({"account":"chatgpt","reported":{"cacheWrite":0}});
    let mut row = request(&yard, 1, Some(reported.clone()), None);
    row["agent"] = json!({"id":"reviewer","type":"review"});
    post(&served, &[row]);
    let doc = read(&yard, &served);
    let totals = session_file(&yard, "tokens", "codex~s1").unwrap();
    assert!(totals.contains("format: yardtokens/2"), "{totals}");
    let sums = &doc["tokenSums"];
    for counts in [
        &sums["all"],
        &sums["sessions"][0]["tokens"],
        &sums["stages"][0]["tokens"],
        &sums["stages"][0]["agents"][0]["tokens"],
        &sums["agents"][0]["tokens"],
        &sums["submissions"][0]["tokens"],
    ] {
        assert!(counts.get("cacheWrite").is_none(), "{counts}");
        assert_eq!(counts["input"], 70);
        assert_eq!(counts["cacheRead"], 30);
        assert_eq!(counts["output"], 5);
    }
    let raw = fs::read_to_string(yard.scratch.home().join(".yard/events/codex~s1.jsonl")).unwrap();
    let kept: Value = serde_json::from_str(raw.lines().next().unwrap()).unwrap();
    assert!(kept["tokens"].get("cacheWrite").is_none(), "{kept}");
    assert_eq!(kept["usageSource"], reported);
    assert_eq!(doc["revision"], 1);
}

#[test]
fn replay_counts_each_request_s_tokens_as_given_whatever_its_source() {
    let (yard, served) = start();
    post(&served, &[request(&yard, 1, None, Some(0))]);
    drop(served);
    let again = Served::start(&yard, &[]);
    let doc = read(&yard, &again);
    assert_eq!(doc["tokenSums"]["all"]["cacheWrite"], 0);
    assert!(doc["tokenSums"]["all"].get("partial").is_none());
    let reported = json!({"account":"chatgpt","reported":{"cacheWrite":0}});
    post(&again, &[request(&yard, 2, Some(reported), None)]);
    let doc = read(&yard, &again);
    assert_eq!(doc["tokenSums"]["all"]["cacheWrite"], 0);
    assert_eq!(doc["tokenSums"]["all"]["partial"], json!(["cacheWrite"]));
    post(
        &again,
        &[request(
            &yard,
            3,
            Some(json!({"account":"chatgpt"})),
            Some(7),
        )],
    );
    let doc = read(&yard, &again);
    assert_eq!(doc["tokenSums"]["all"]["cacheWrite"], 7);
    assert_eq!(doc["tokenSums"]["all"]["input"], 210);
    assert!(
        session_file(&yard, "tokens", "codex~s1")
            .unwrap()
            .contains("format: yardtokens/2")
    );
}

#[test]
fn a_totals_file_of_a_newer_format_keeps_its_bytes_while_the_session_s_requests_go_on() {
    let (yard, served) = start();
    let path = yard.folder(BRANCH).join("tokens/codex~s1.yaml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let newer = "format: yardtokens/3\nsession: s1\nhost: codex\n";
    fs::write(&path, newer).unwrap();
    for second in [1, 2] {
        let reply = post(&served, &[request(&yard, second, None, Some(7))]);
        assert_eq!(reply["recorded"], 1);
        assert_eq!(reply["rejected"], json!([]));
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), newer);
    let raw = fs::read_to_string(yard.scratch.home().join(".yard/events/codex~s1.jsonl")).unwrap();
    assert_eq!(raw.lines().count(), 2, "the events are kept");
    let doc = read(&yard, &served);
    assert_eq!(doc["tokenSums"]["partial"], true);
}
