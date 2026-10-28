//! `loft yard read --context`: the calling agent's briefing, as fields and as text of at most
//! 2,000 characters that names no host's tools.

use std::fs;

use serde_json::{Value, json};
use yardlog_store::lock;

use crate::common::stderr;
use crate::snapshot::snapshot;
use crate::yard::{ASK, Yard};

/// The briefing of session `s1`'s subagent `w1`, or of its main agent where `agent` is none.
fn context(yard: &Yard, agent: Option<&str>, extra: &[&str]) -> Value {
    let mut args = vec!["--context"];
    if let Some(agent) = agent {
        args.extend_from_slice(&["--agent", agent, "--agent-type", "worker"]);
    }
    args.extend_from_slice(extra);
    yard.read("s1", &args)["context"].clone()
}

fn text(context: &Value) -> &str {
    context["text"].as_str().unwrap()
}

/// Names a running page server on port 7500 while `run` runs.
fn served<T>(yard: &Yard, run: impl FnOnce() -> T) -> T {
    let dir = yard.scratch.home().join(".yard");
    fs::create_dir_all(&dir).unwrap();
    let server = json!({ "port": 7500, "pid": 1, "version": "2.2.1", "formats": ["yardlog/1"] });
    fs::write(dir.join("server.json"), server.to_string()).unwrap();
    let held = lock::lock(&dir.join("server.lock")).unwrap();
    let value = run();
    drop(held);
    value
}

fn assignee(agent: &str) -> Value {
    json!({"host": "claude-code", "session": "s1", "agent": agent, "agentType": "worker"})
}

#[test]
fn the_briefing_names_the_page_revision_stage_open_submissions_and_what_was_answered_since() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    yard.answer("s1", r#"[{"block": "q1", "labels": ["Top"]}]"#);
    yard.read("s1", &["--mark-read"]);
    let colour = r#"[
      {"id": "q2", "kind": "question", "stage": "shape", "prompt": "Which colour?",
       "header": "Colour", "options": [{"label": "Red"}, {"label": "Blue"}]},
      {"id": "q3", "kind": "question", "stage": "shape", "prompt": "Which size?",
       "header": "Size", "options": [{"label": "Small"}, {"label": "Large"}]}
    ]"#;
    yard.write("s1", colour, &["--send"]);
    yard.answer("s1", r#"[{"block": "q2", "labels": ["Red"]}]"#);
    let briefed = served(&yard, || context(&yard, None, &[]));
    let page = briefed["page"].as_str().unwrap();
    assert!(page.starts_with("http://127.0.0.1:7500/#tree="), "{page}");
    assert_eq!(briefed["revision"], 4);
    assert_eq!(briefed["since"], 2);
    assert_eq!(
        briefed["stage"],
        json!({"id": "shape", "title": "Shape it"})
    );
    let open = briefed["open"].as_array().unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(
        (&open[0]["blocks"], &open[0]["sentBy"]),
        (&json!(2), &json!("self"))
    );
    assert_eq!(
        briefed["responded"],
        json!([{"id": "q2", "kind": "question", "response": "answered", "title": "Which colour?"}])
    );
    let text = text(&briefed);
    for said in [
        &format!("Its page: {page}"),
        "Yard Log is open, at revision 4.",
        "Your working stage: `shape` — Shape it.",
        "Open submissions (1):",
        "2 blocks, sent by you",
        "Answered or decided since revision 2, which you last read (1):",
        "- `q2` answered: Which colour?",
    ] {
        assert!(text.contains(said), "{said}\n{text}");
    }
    for tool in ["yard_read", "yard_wait", "yard_write", "mcp__"] {
        assert!(!text.contains(tool), "{tool}\n{text}");
    }
}

#[test]
fn the_briefing_lists_the_unfinished_tasks_assigned_to_the_agent_with_their_stages() {
    let yard = Yard::new();
    let batch = json!([
        {"id": "build", "kind": "stage", "title": "Build", "status": "active"},
        {"id": "later", "kind": "stage", "title": "Later", "status": "planned"},
        {"id": "one", "kind": "task", "stage": "build", "title": "One", "state": "active",
         "assignee": assignee("w1")},
        {"id": "two", "kind": "task", "stage": "build", "title": "Two", "state": "done",
         "assignee": assignee("w1")},
        {"id": "three", "kind": "task", "stage": "build", "title": "Three", "state": "open",
         "assignee": assignee("w2")}
    ]);
    yard.write("s1", &batch.to_string(), &[]);
    yard.write(
        "s1",
        r#"[{"id": "four", "kind": "task", "stage": "later", "title": "Four", "state": "open"}]"#,
        &[],
    );
    let claim = json!([{"id": "four", "kind": "task", "stage": "later", "title": "Four",
                        "state": "open", "assignee": assignee("w1")}]);
    let claimed = yard.run_with(
        &[
            "write",
            "--session",
            "s1",
            "--agent",
            "w1",
            "--agent-type",
            "worker",
        ],
        &claim.to_string(),
    );
    assert_eq!(claimed.status.code(), Some(0), "{}", stderr(&claimed));
    let briefed = context(&yard, Some("w1"), &[]);
    assert_eq!(
        briefed["assigned"],
        json!([
            {"id": "one", "stage": "build", "title": "One", "state": "active"},
            {"id": "four", "stage": "later", "title": "Four", "state": "open"}
        ])
    );
    let text = text(&briefed);
    assert!(
        text.contains("Unfinished tasks assigned to you (2):"),
        "{text}"
    );
    assert!(text.contains("- `one`, active, in stage `build`"), "{text}");
    assert!(text.contains("- `four`, open, in stage `later`"), "{text}");
    assert!(
        !text.contains("`two`") && !text.contains("`three`"),
        "{text}"
    );
}

#[test]
fn the_stage_the_briefing_names_is_the_agent_s_working_stage() {
    let yard = Yard::new();
    let batch = json!([
        {"id": "mine", "kind": "stage", "title": "Mine", "status": "active"},
        {"id": "other", "kind": "stage", "title": "Other", "status": "planned"},
        {"id": "one", "kind": "task", "stage": "mine", "title": "One", "state": "open"}
    ]);
    yard.write("s1", &batch.to_string(), &[]);
    // A subagent with no stage of its own works in its session's main agent's stage.
    assert_eq!(
        context(&yard, Some("w1"), &[])["stage"],
        json!({"id": "mine", "title": "Mine"})
    );
    // An agent of another session, with no stage and no task, has none.
    let theirs = yard.read("s2", &["--context"])["context"].clone();
    assert_eq!(theirs["stage"], Value::Null);
    assert!(text(&theirs).contains("You have no working stage."));
}

#[test]
fn the_briefing_s_text_counts_what_it_leaves_out_within_2000_characters() {
    let yard = Yard::new();
    let mut batch =
        vec![json!({"id": "build", "kind": "stage", "title": "Build", "status": "active"})];
    for n in 0..80 {
        batch.push(json!({
            "id": format!("task-{n}"), "kind": "task", "stage": "build",
            "title": format!("Task number {n} with a title long enough to fill the briefing"),
            "state": "open", "assignee": assignee("w1")
        }));
    }
    yard.write("s1", &Value::Array(batch).to_string(), &[]);
    let briefed = context(&yard, Some("w1"), &[]);
    assert_eq!(briefed["assigned"].as_array().unwrap().len(), 80);
    let text = text(&briefed);
    assert!(text.chars().count() <= 2000, "{}", text.chars().count());
    let listed = text
        .lines()
        .filter(|line| line.starts_with("- `task-"))
        .count();
    assert!(listed > 0 && listed < 80, "{listed}");
    let tail = format!("  … and {} more: read Yard Log for them", 80 - listed);
    assert!(text.contains(&tail), "{tail}\n{text}");
    assert!(text.contains("No submission is open."), "{text}");
}

#[test]
fn the_briefing_of_a_branch_with_no_yard_log_names_only_the_page_and_that_there_is_none() {
    let yard = Yard::new();
    let before = snapshot(&yard.scratch.0);
    let briefed = served(&yard, || context(&yard, None, &["--mark-read"]));
    assert_eq!(briefed["noLog"], true);
    assert_eq!(briefed["revision"], 0);
    let page = briefed["page"].as_str().unwrap();
    assert_eq!(
        text(&briefed),
        format!("Its page: {page}\nThe branch has no Yard Log yet: the first write opens it.")
    );
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

#[test]
fn the_briefing_says_whether_yard_log_is_closed_and_whether_git_ignores_yard() {
    let mut yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let open = context(&yard, None, &[]);
    assert_eq!(
        (&open["closed"], &open["ignored"]),
        (&json!(false), &json!(false))
    );
    assert!(text(&open).contains("Yard Log is committed with the work"));
    assert!(text(&open).contains("Yard Log is open, at revision 1."));
    yard.git = yard.git.clone().ignoring(":/.yard/");
    let closed = yard.run(&["close", "--session", "s1"]);
    assert_eq!(closed.status.code(), Some(0), "{}", stderr(&closed));
    let briefed = context(&yard, None, &[]);
    assert_eq!(
        (&briefed["closed"], &briefed["ignored"]),
        (&json!(true), &json!(true))
    );
    let text = text(&briefed);
    assert!(text.contains("git ignores `.yard/`, so Yard Log stays out of the commits."));
    assert!(
        text.contains("Yard Log is closed: it takes no more writes"),
        "{text}"
    );
}

#[test]
fn context_given_without_session_exits_2_and_names_session() {
    let yard = Yard::new();
    let output = yard.run(&["read", "--context"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("--session"), "{}", stderr(&output));
}

#[test]
fn context_with_mark_read_records_the_revision_the_briefing_showed() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let briefed = context(&yard, None, &["--mark-read"]);
    assert_eq!(briefed["since"], 0);
    assert_eq!(yard.read("s1", &[])["lastRead"], 1);
    assert_eq!(context(&yard, None, &[])["since"], 1);
}
