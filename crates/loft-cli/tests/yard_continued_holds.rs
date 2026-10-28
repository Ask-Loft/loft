//! A session that continues another, as after `/clear`, holds what the session it continues held:
//! it works in the stage that session made active, as its owner, holds the tasks assigned to it,
//! has its tokens and activity counted toward that stage, and is refused a second active stage,
//! while each version it writes keeps its own session id.

use serde_json::{Value, json};

use crate::served::Served;
use crate::yard::{Yard, block, document};
use crate::yard_serve_events::{event, log, post, session_file, tokens};

/// Stage `build`, active and `s1`'s main agent's, with `mine` assigned to that agent and
/// `theirs` to its subagent `w1`.
const BUILD: &str = r#"[
  {"id": "build", "kind": "stage", "title": "Build", "status": "active"},
  {"id": "mine", "kind": "task", "stage": "build", "title": "Mine", "state": "open",
   "assignee": {"host": "claude-code", "session": "s1"}},
  {"id": "theirs", "kind": "task", "stage": "build", "title": "Theirs", "state": "open",
   "assignee": {"host": "claude-code", "session": "s1", "agent": "w1", "agentType": "worker"}}
]"#;

/// Yard Log of [`BUILD`], with `s2` recorded as continuing `s1` where `continued`.
fn built(continued: bool) -> Yard {
    let yard = Yard::new();
    let written = yard.write("s1", BUILD, &[]);
    assert_eq!(written["outcome"], "written", "{written}");
    if continued {
        let output = yard.run(&["continue", "--session", "s2", "--from", "s1"]);
        assert_eq!(output.status.code(), Some(0));
    }
    yard
}

fn stage_key(yard: &Yard) -> Value {
    yard.read("s1", &[])["stages"][0]["key"].clone()
}

#[test]
fn the_continuing_session_is_briefed_with_the_stage_the_ended_session_made_active() {
    let yard = built(true);
    let briefed = &yard.read("s2", &["--context"])["context"];
    assert_eq!(briefed["stage"], json!({"id": "build", "title": "Build"}));
    assert!(
        briefed["text"]
            .as_str()
            .unwrap()
            .contains("Your working stage: `build`"),
        "{briefed}"
    );
    let alone = built(false);
    assert_eq!(
        alone.read("s2", &["--context"])["context"]["stage"],
        Value::Null
    );
}

#[test]
fn the_continuing_session_changes_and_reassigns_the_tasks_of_the_ended_session_s_stage() {
    let yard = built(true);
    let reassigned = r#"[{"id": "theirs", "kind": "task", "stage": "build", "title": "Theirs",
      "state": "open", "assignee": {"host": "claude-code", "session": "s1", "agent": "w2",
      "agentType": "worker"}}]"#;
    let written = yard.write("s2", reassigned, &[]);
    assert_eq!(written["outcome"], "written", "{written}");
    let output = yard.run(&[
        "task",
        "mine",
        "--mirror",
        "--state",
        "done",
        "--session",
        "s2",
    ]);
    assert_eq!(document("task", &output)["outcome"], "changed");
    let read = yard.read("s1", &[]);
    assert_eq!(block(&read, "mine")["version"]["body"]["state"], "done");
    assert_eq!(
        block(&read, "theirs")["version"]["body"]["assignee"]["agent"],
        "w2"
    );
    let alone = built(false);
    assert_eq!(alone.write("s2", reassigned, &[])["outcome"], "refused");
}

#[test]
fn each_version_the_continuing_session_writes_keeps_its_own_session_id() {
    let yard = built(true);
    yard.run(&["task", "mine", "--state", "active", "--session", "s2"]);
    let read = yard.read("s1", &[]);
    assert_eq!(
        block(&read, "mine")["version"]["act"]["by"]["session"],
        "s2"
    );
}

#[test]
fn a_second_stage_made_active_by_the_continuing_session_is_refused_naming_the_first() {
    let review = r#"[{"id": "review", "kind": "stage", "title": "Review", "status": "active"}]"#;
    let yard = built(true);
    let output = yard.run_with(&["write", "--session", "s2"], review);
    let refused = document("write", &output);
    assert_eq!(refused["outcome"], "refused", "{refused}");
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("`build`"), "{message}");
    assert_eq!(built(false).write("s2", review, &[])["outcome"], "written");
}

#[test]
fn the_continuing_session_s_tokens_and_activity_count_toward_the_ended_session_s_stage() {
    for continued in [true, false] {
        let yard = built(continued);
        let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
        let mut request = event(
            &yard,
            "request",
            5,
            &json!({ "model": "opus", "tokens": tokens(10, 100, 5, 20) }),
        );
        request["session"] = json!("s2");
        post(&served, &[request]);
        let stage = stage_key(&yard);
        let counted = log(&yard, "s2")[0]["stage"].clone();
        let totals = session_file(&yard, "tokens", "s2").unwrap_or_default();
        let named = totals.contains(&format!("stage: {}", stage.as_str().unwrap()));
        if continued {
            assert_eq!(counted, stage);
            assert!(named, "{totals}");
        } else {
            assert_eq!(counted, Value::Null);
            assert!(!named, "{totals}");
        }
    }
}
