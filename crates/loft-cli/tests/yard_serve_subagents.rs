//! A subagent's work as the page server keeps it: while the subagent has no stage of its own
//! active, its requests, its Loft runs and its run count toward the stage active for its
//! session's main agent, and each stage's counts are kept by agent.

use serde_json::{Value, json};

use crate::served::Served;
use crate::yard::Yard;
use crate::yard_serve_events::{event, keys, open, post, session_file, tokens};

const EXPLORE: [&str; 4] = ["--agent", "a1", "--agent-type", "Explore"];

/// A request of subagent `a1`, an Explore agent, at second `second`, of `count` tokens each.
fn sub_request(yard: &Yard, second: u32, count: u64) -> Value {
    event(
        yard,
        "request",
        second,
        &json!({ "model": "haiku", "agent": { "id": "a1", "type": "Explore" }, "tokens": tokens(count, count, count, count) }),
    )
}

/// The key of the stage `id` in `yard`'s Yard Log.
fn stage_key(yard: &Yard, id: &str) -> String {
    let read = yard.read("s1", &[]);
    let stages = read["stages"].as_array().unwrap();
    let stage = stages.iter().find(|stage| stage["id"] == id).unwrap();
    stage["key"].as_str().unwrap().to_owned()
}

#[test]
fn a_stageless_subagent_s_request_counts_toward_the_stage_active_for_its_session_s_main_agent() {
    let (yard, served) = open();
    let (stage, _) = keys(&yard);
    post(&served, &[sub_request(&yard, 5, 2)]);
    let text = session_file(&yard, "tokens", "s1").unwrap();
    assert!(
        text.contains(&format!("stages:\n  - stage: {stage}\n    input: 2\n")),
        "{text}"
    );
    assert!(!text.contains("noStage:"), "{text}");
}

#[test]
fn with_no_stage_active_for_either_a_subagent_s_request_counts_under_no_stage() {
    let yard = Yard::new();
    yard.write(
        "s1",
        r#"[{"id": "later", "kind": "stage", "title": "Later", "status": "planned"}]"#,
        &[],
    );
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    post(&served, &[sub_request(&yard, 5, 2)]);
    let text = session_file(&yard, "tokens", "s1").unwrap();
    assert!(!text.contains("stages:"), "{text}");
    assert!(text.contains("noStage:\n  input: 2\n"), "{text}");
}

#[test]
fn once_a_subagent_opens_a_stage_of_its_own_its_requests_count_toward_that_stage_alone() {
    let (yard, served) = open();
    let (main, _) = keys(&yard);
    yard.write(
        "s1",
        r#"[{"id": "scan", "kind": "stage", "title": "Scan", "status": "active"}]"#,
        &EXPLORE,
    );
    let own = stage_key(&yard, "scan");
    post(&served, &[sub_request(&yard, 5, 3)]);
    let text = session_file(&yard, "tokens", "s1").unwrap();
    assert!(
        text.contains(&format!("  - stage: {own}\n    input: 3\n")),
        "{text}"
    );
    assert!(!text.contains(&format!("stage: {main}")), "{text}");
}

#[test]
fn each_stage_s_counts_are_kept_by_agent_the_main_agent_and_each_subagent_by_id_and_type() {
    let (yard, served) = open();
    let (stage, _) = keys(&yard);
    post(
        &served,
        &[
            event(
                &yard,
                "request",
                4,
                &json!({ "model": "opus", "tokens": tokens(10, 10, 10, 10) }),
            ),
            sub_request(&yard, 5, 2),
            sub_request(&yard, 6, 1),
        ],
    );
    let text = session_file(&yard, "tokens", "s1").unwrap();
    let expected = format!(
        "stages:
  - stage: {stage}
    input: 13
    cacheRead: 13
    cacheWrite: 13
    output: 13
    agents:
      - agent: main
        input: 10
        cacheRead: 10
        cacheWrite: 10
        output: 10
      - agent: a1
        type: Explore
        input: 3
        cacheRead: 3
        cacheWrite: 3
        output: 3
agents:
"
    );
    assert!(text.contains(&expected), "{text}");
}

#[test]
fn a_subagent_s_run_is_kept_with_the_stage_its_session_s_main_agent_had_active_as_it_started() {
    let (yard, served) = open();
    let (stage, _) = keys(&yard);
    let sub = json!({ "id": "a1", "type": "Explore" });
    post(
        &served,
        &[
            event(&yard, "turn.start", 0, &json!({})),
            event(
                &yard,
                "subagent.start",
                1,
                &json!({ "agent": sub, "model": "haiku" }),
            ),
            event(&yard, "subagent.end", 3, &json!({ "agent": sub })),
        ],
    );
    let text = session_file(&yard, "activity", "s1").unwrap();
    let run = format!(
        "subagents:
  - agent: a1
    type: Explore
    started: 2026-01-01T10:00:01Z
    ended: 2026-01-01T10:00:03Z
    stage: {stage}
"
    );
    assert!(text.contains(&run), "{text}");
}

#[test]
fn a_stageless_subagent_s_loft_run_is_kept_with_the_main_agent_s_stage() {
    let (yard, served) = open();
    let (stage, _) = keys(&yard);
    let sub = json!({ "id": "a1", "type": "Explore" });
    post(
        &served,
        &[
            event(
                &yard,
                "tool.start",
                1,
                &json!({ "agent": sub, "tool": "Bash", "call": "b1", "loft": ["check"] }),
            ),
            event(
                &yard,
                "tool.end",
                2,
                &json!({ "agent": sub, "tool": "Bash", "call": "b1", "failed": false }),
            ),
        ],
    );
    let text = session_file(&yard, "activity", "s1").unwrap();
    assert!(text.contains("    agent: a1\n"), "{text}");
    assert!(text.contains(&format!("    stage: {stage}\n")), "{text}");
}
