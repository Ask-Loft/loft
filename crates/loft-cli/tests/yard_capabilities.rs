//! Adapter coverage is retained independently of whether a host reports model usage.

use serde_json::{Value, json};

use crate::served::Served;
use crate::yard::{ASK, Yard};
use crate::yard_serve_events::{event, post, session_file};

fn descriptor(usage: &[&str]) -> Value {
    json!({"format":"yard-adapter/1", "version":"1.0.0", "capabilities": {
        "events":["turn.start", "turn.end", "request"], "usage":usage,
        "nativeQuestions":false, "nativePlans":false, "taskMirroring":true
    }})
}

#[test]
fn an_adapter_descriptor_is_kept_without_a_token_event_and_survives_replay() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let tree = yard.scratch.0.display().to_string();
    let served = Served::start(&yard, &["--tree", &tree]);
    let adapter = descriptor(&[]);
    let sent = post(
        &served,
        &[event(&yard, "turn.start", 1, &json!({"adapter":adapter}))],
    );
    assert_eq!(sent["recorded"], 1);
    let activity = session_file(&yard, "activity", "s1").unwrap();
    assert!(activity.contains("adapter:"), "{activity}");
    assert!(!activity.contains("tokens:"));
    assert!(session_file(&yard, "tokens", "s1").is_none());
    drop(served);
    let served = Served::start(&yard, &["--tree", &tree]);
    post(&served, &[event(&yard, "turn.end", 2, &json!({}))]);
    let activity = session_file(&yard, "activity", "s1").unwrap();
    assert!(activity.contains("adapter:"), "{activity}");
    assert!(activity.contains("nativeQuestions: false"), "{activity}");
    assert!(activity.contains("host: claude-code"), "{activity}");
    let tree = yard.scratch.0.canonicalize().unwrap();
    let reply = served.get(&format!(
        "/api/log?tree={}&branch=feat/shelf",
        tree.display()
    ));
    assert_eq!(reply.status, 200);
    let schema =
        std::fs::read_to_string(crate::common::workspace().join("schemas/yard-read.json")).unwrap();
    let read = crate::schema::conforming(&reply.body, &schema);
    assert_eq!(
        read["activity"][0]["adapter"]["capabilities"]["usage"],
        json!([])
    );
    assert_eq!(read["activity"][0]["host"], "claude-code");
}

#[test]
fn missing_usage_is_not_written_as_zero_and_partial_subtotals_are_marked() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let served = Served::start(&yard, &[]);
    let adapter = descriptor(&["input", "output"]);
    post(
        &served,
        &[event(
            &yard,
            "request",
            1,
            &json!({"adapter":adapter, "tokens":{"input":0,"output":5}}),
        )],
    );
    let first = session_file(&yard, "tokens", "s1").unwrap();
    assert!(first.contains("input: 0"));
    assert!(!first.contains("cacheRead:"), "{first}");
    assert!(!first.contains("cacheWrite:"), "{first}");
    post(
        &served,
        &[event(&yard, "request", 2, &json!({"tokens":{"output":3}}))],
    );
    let summed = session_file(&yard, "tokens", "s1").unwrap();
    assert!(summed.contains("output: 8"), "{summed}");
    assert!(summed.contains("partial:\n    - input"), "{summed}");
}

#[test]
fn malformed_descriptors_and_usage_are_rejected_before_recording() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let served = Served::start(&yard, &[]);
    for extra in [
        json!({"adapter":null}),
        json!({"adapter":{"format":"yard-adapter/9","version":"1","capabilities":{}}}),
        json!({"adapter":{"format":"yard-adapter/1","version":"1","capabilities":{"nativePlans":"yes"}}}),
        json!({"adapter":{"format":"yard-adapter/1","version":"1","capabilities":{"usage":["money"]}}}),
        json!({"tokens":{"input":-1}}),
    ] {
        let result = post(&served, &[event(&yard, "request", 1, &extra)]);
        assert_eq!(result["recorded"], 0, "{result}");
        assert_eq!(result["rejected"].as_array().unwrap().len(), 1);
    }
    assert!(session_file(&yard, "activity", "s1").is_none());
    assert!(!yard.scratch.home().join(".yard/events/s1.jsonl").exists());
}
