//! `loft yard task`: a task changed in place as Yard Log holds it, and an agent's active tasks
//! released, each in one write or none.

use std::process::Output;

use serde_json::Value;

use crate::common::stderr;
use crate::snapshot::snapshot;
use crate::yard::{BRANCH, Yard, block, document};

/// A stage of the main agent of `s1`, with `one` and `two` assigned to its subagent `w1` and
/// `theirs` assigned to its subagent `w2`.
const PLANNED: &str = r#"[
  {"id": "build", "kind": "stage", "title": "Build", "status": "active"},
  {"id": "one", "kind": "task", "stage": "build", "title": "One", "state": "open",
   "detail": "As planned", "type": "fix",
   "assignee": {"host": "claude-code", "session": "s1", "agent": "w1", "agentType": "worker"}},
  {"id": "two", "kind": "task", "stage": "build", "title": "Two", "state": "active",
   "assignee": {"host": "claude-code", "session": "s1", "agent": "w1", "agentType": "worker"}},
  {"id": "done", "kind": "task", "stage": "build", "title": "Done", "state": "done",
   "assignee": {"host": "claude-code", "session": "s1", "agent": "w1", "agentType": "worker"}},
  {"id": "theirs", "kind": "task", "stage": "build", "title": "Theirs", "state": "active",
   "assignee": {"host": "claude-code", "session": "s1", "agent": "w2", "agentType": "worker"}}
]"#;

fn planned() -> Yard {
    let yard = Yard::new();
    let written = yard.write("s1", PLANNED, &[]);
    assert_eq!(written["outcome"], "written", "{written}");
    yard
}

/// Runs `loft yard task` as `s1`'s subagent `agent`.
fn task(yard: &Yard, agent: &str, args: &[&str]) -> Output {
    let mut all = vec!["task"];
    all.extend_from_slice(args);
    all.extend_from_slice(&[
        "--session",
        "s1",
        "--agent",
        agent,
        "--agent-type",
        "worker",
    ]);
    yard.run(&all)
}

fn body<'a>(read: &'a Value, id: &str) -> &'a Value {
    &block(read, id)["version"]["body"]
}

#[test]
fn a_task_change_names_the_task_and_its_state_after_the_change() {
    let yard = planned();
    let output = task(&yard, "w1", &["one", "--state", "active"]);
    let changed = document("task", &output);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(changed["outcome"], "changed");
    assert_eq!(
        (&changed["task"], &changed["state"]),
        (&"one".into(), &"active".into())
    );
    assert_eq!(changed["revision"], 2);
    let read = yard.read("s1", &[]);
    let one = body(&read, "one");
    assert_eq!(one["state"], "active");
    assert_eq!(
        (&one["title"], &one["detail"], &one["type"]),
        (&"One".into(), &"As planned".into(), &"fix".into())
    );
    assert_eq!(one["assignee"]["agent"], "w1");
    assert_eq!(block(&read, "one")["versionCount"], 2);
}

#[test]
fn a_task_change_that_leaves_the_task_as_it_was_is_unchanged_and_writes_nothing() {
    let yard = planned();
    let before = snapshot(&yard.folder(BRANCH));
    let output = task(&yard, "w1", &["one", "--state", "open", "--title", "One"]);
    let unchanged = document("task", &output);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(unchanged["outcome"], "unchanged");
    assert_eq!(
        (&unchanged["task"], &unchanged["state"]),
        (&"one".into(), &"open".into())
    );
    assert_eq!(unchanged["revision"], 1);
    assert!(snapshot(&yard.folder(BRANCH)) == before, "a file changed");
}

#[test]
fn a_task_change_the_delegation_rules_refuse_exits_1_and_changes_no_file() {
    let yard = planned();
    let before = snapshot(&yard.folder(BRANCH));
    let output = task(&yard, "w2", &["one", "--state", "done"]);
    let refused = document("task", &output);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(refused["outcome"], "refused");
    assert_eq!(refused["errors"][0]["kind"], "block");
    assert!(snapshot(&yard.folder(BRANCH)) == before, "a file changed");
}

#[test]
fn a_mirrored_task_change_leaves_another_agent_s_task_alone_and_keeps_a_done_task_s_state() {
    let yard = planned();
    let before = snapshot(&yard.folder(BRANCH));
    let output = task(&yard, "w2", &["one", "--state", "done", "--mirror"]);
    let unchanged = document("task", &output);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(unchanged["outcome"], "unchanged");
    assert_eq!(unchanged["state"], "open");
    assert!(snapshot(&yard.folder(BRANCH)) == before, "a file changed");
    let output = task(
        &yard,
        "w1",
        &[
            "done", "--state", "active", "--title", "Finished", "--mirror",
        ],
    );
    let changed = document("task", &output);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        (&changed["outcome"], &changed["state"]),
        (&"changed".into(), &"done".into())
    );
    assert_eq!(body(&yard.read("s1", &[]), "done")["title"], "Finished");
}

#[test]
fn a_task_id_that_names_no_task_is_refused_naming_it_and_changes_no_file() {
    let yard = planned();
    let before = snapshot(&yard.scratch.0);
    for id in ["missing", "build"] {
        let output = task(&yard, "w1", &[id, "--state", "done"]);
        let refused = document("task", &output);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(refused["outcome"], "refused");
        let message = refused["errors"][0]["message"].as_str().unwrap();
        assert!(message.contains(&format!("`{id}`")), "{message}");
    }
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

#[test]
fn releasing_returns_the_agent_s_active_tasks_to_open_and_lists_them() {
    let yard = planned();
    let output = task(&yard, "w1", &["--release"]);
    let released = document("task", &output);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(released["outcome"], "released");
    assert_eq!(released["released"], serde_json::json!(["two"]));
    assert_eq!(released["revision"], 2);
    let read = yard.read("s1", &[]);
    assert_eq!(body(&read, "two")["state"], "open");
    assert_eq!(body(&read, "two")["assignee"]["agent"], "w1");
    for (id, state) in [("one", "open"), ("done", "done"), ("theirs", "active")] {
        assert_eq!(body(&read, id)["state"], state, "{id}");
        assert_eq!(block(&read, id)["versionCount"], 1, "{id}");
    }
}

#[test]
fn releasing_an_agent_with_no_active_task_is_unchanged_and_writes_nothing() {
    let yard = planned();
    let before = snapshot(&yard.folder(BRANCH));
    let output = task(&yard, "w3", &["--release"]);
    let unchanged = document("task", &output);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(unchanged["outcome"], "unchanged");
    assert_eq!(unchanged["released"], serde_json::json!([]));
    assert!(snapshot(&yard.folder(BRANCH)) == before, "a file changed");
}

#[test]
fn a_task_change_needs_a_task_id_or_release_and_not_both() {
    let yard = planned();
    for args in [&["--state", "done"][..], &["one", "--release"]] {
        let output = task(&yard, "w1", args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
    }
}
