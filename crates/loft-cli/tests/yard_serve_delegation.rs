//! Recorded assignments determine activity and usage attribution without inventing counters.

use std::fs;

use serde_json::{Value, json};

use crate::yard::{BRANCH, Yard};
use crate::yard_serve_events::{event, keys, open, post, session_file};

fn write(yard: &Yard, batch: &Value) {
    let result = yard.write("s1", &batch.to_string(), &[]);
    assert_eq!(result["outcome"], "written", "{result}");
}

fn task(id: &str, stage: &str) -> Value {
    json!({"id":id,"kind":"task","stage":stage,"title":"Work","state":"active","assignee":{"host":"claude-code","session":"s1","agent":"a1","agentType":"Explore"}})
}

fn request(yard: &Yard, second: u32) -> Value {
    event(
        yard,
        "request",
        second,
        &json!({"agent":{"id":"a1","type":"Renamed description"},"tokens":{"input":3,"cacheRead":7,"output":2}}),
    )
}

fn log(yard: &Yard) -> Vec<Value> {
    fs::read_to_string(yard.scratch.home().join(".yard/events/s1.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn assigned_task_stage_precedes_current_parent_and_worker_stages_for_usage_and_runs() {
    let (yard, served) = open();
    let (original, _) = keys(&yard);
    write(
        &yard,
        &json!([task("first","shape"),task("second","shape"),
        {"id":"shape","kind":"stage","title":"Shape it","status":"planned"},
        {"id":"next","kind":"stage","title":"Next","status":"active"}]),
    );
    yard.write(
        "s1",
        r#"[{"id":"own","kind":"stage","title":"Worker stage","status":"active"}]"#,
        &["--agent", "a1", "--agent-type", "Explore"],
    );
    let actor = json!({"id":"a1","type":"Explore"});
    post(
        &served,
        &[
            event(&yard, "subagent.start", 1, &json!({"agent":actor})),
            event(
                &yard,
                "tool.start",
                2,
                &json!({"agent":actor,"tool":"Bash","call":"loft","loft":["check"]}),
            ),
            event(
                &yard,
                "tool.end",
                3,
                &json!({"agent":actor,"tool":"Bash","call":"loft"}),
            ),
            request(&yard, 4),
            event(&yard, "subagent.end", 5, &json!({"agent":actor})),
        ],
    );
    assert!(log(&yard).iter().all(|entry| entry["stage"] == original));
    let usage = session_file(&yard, "tokens", "s1").unwrap();
    assert!(
        usage.contains(&format!("stage: {original}\n    input: 3\n")),
        "{usage}"
    );
    assert!(!usage.contains("cacheWrite:"), "{usage}");
    let activity = session_file(&yard, "activity", "s1").unwrap();
    assert_eq!(
        activity.matches(&format!("stage: {original}")).count(),
        2,
        "{activity}"
    );
    assert!(yard.folder(BRANCH).join("tokens/s1.yaml").exists());
}

#[test]
fn assignments_across_stages_keep_reported_tokens_in_session_totals_without_guessing_a_stage() {
    let (yard, served) = open();
    write(
        &yard,
        &json!([task("first","shape"),{"id":"shape","kind":"stage","title":"Shape it","status":"planned"},{"id":"elsewhere","kind":"stage","title":"Elsewhere","status":"active"},task("second","elsewhere")]),
    );
    post(&served, &[request(&yard, 3)]);
    assert!(log(&yard)[0]["stage"].is_null());
    let usage = session_file(&yard, "tokens", "s1").unwrap();
    assert!(
        usage.contains("tokens:\n  input: 3\n  cacheRead: 7\n  output: 2\n"),
        "{usage}"
    );
    assert!(usage.contains("noStage:\n  input: 3\n"), "{usage}");
    assert!(!usage.contains("stages:"), "{usage}");
    assert!(!usage.contains("cacheWrite:"), "{usage}");
}

#[test]
fn an_explicit_current_assignment_selects_its_stage_and_invalid_bindings_never_guess() {
    let (yard, served) = open();
    let (original, _) = keys(&yard);
    write(
        &yard,
        &json!([task("first","shape"),{"id":"shape","kind":"stage","title":"Shape it","status":"planned"},{"id":"elsewhere","kind":"stage","title":"Elsewhere","status":"active"},task("second","elsewhere")]),
    );
    let mut explicit = request(&yard, 1);
    explicit["task"] = json!("first");
    let mut invalid = request(&yard, 2);
    invalid["task"] = json!("missing");
    let mut wrong = explicit.clone();
    wrong["agent"]["id"] = json!("other");
    wrong["at"] = json!("2026-01-01T10:00:03Z");
    post(&served, &[explicit, invalid, wrong]);
    let kept = log(&yard);
    assert_eq!(kept[0]["stage"], original);
    assert!(kept[1]["stage"].is_null());
    assert!(kept[2]["stage"].is_null());
    let usage = session_file(&yard, "tokens", "s1").unwrap();
    assert!(usage.contains("tokens:\n  input: 9\n"), "{usage}");
    assert!(usage.contains("noStage:\n  input: 6\n"), "{usage}");
}
