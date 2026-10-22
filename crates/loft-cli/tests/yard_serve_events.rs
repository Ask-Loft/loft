//! The mod's events, as the page server keeps them: each, where its agent's branch has its Yard
//! Log open, in the session's event log in `~/.yard/events/`, with the tree, the branch, the
//! stage active for its agent and the submissions open; and what they add to the session's token
//! totals and activity in that Yard Log's folder, which changes no revision.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::served::Served;
use crate::yard::{ASK, BRANCH, Yard};

/// Yard Log with its stage active for session `s1`'s main agent and `q1` sent, and a page server.
fn open() -> (Yard, Served) {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    (yard, served)
}

/// The directory of `yard`'s tree, as an event names it.
fn dir(yard: &Yard) -> String {
    yard.scratch.0.canonicalize().unwrap().display().to_string()
}

/// An event of `kind` of session `s1`'s main agent in `yard`'s tree at second `second` of
/// 10:00, with `more` fields.
fn event(yard: &Yard, kind: &str, second: u32, more: &Value) -> Value {
    let mut event = json!({
        "session": "s1",
        "dir": dir(yard),
        "at": format!("2026-01-01T10:00:{second:02}.250Z"),
        "kind": kind,
    });
    for (name, value) in more.as_object().unwrap() {
        event[name] = value.clone();
    }
    event
}

fn tokens(input: u64, read: u64, write: u64, output: u64) -> Value {
    json!({ "input": input, "cacheRead": read, "cacheWrite": write, "output": output })
}

fn post(served: &Served, events: &[Value]) -> Value {
    let reply = served.post_as_mod("/api/events", &json!(events));
    assert_eq!(reply.status, 200);
    reply.json()
}

/// A session's file in the folder of `yard`'s Yard Log, as text.
fn session_file(yard: &Yard, folder: &str, session: &str) -> Option<String> {
    let path = yard
        .folder(BRANCH)
        .join(folder)
        .join(format!("{session}.yaml"));
    fs::read_to_string(path).ok()
}

/// Session `session`'s event log, a line to an event.
fn log(yard: &Yard, session: &str) -> Vec<Value> {
    let path: PathBuf = yard
        .scratch
        .home()
        .join(".yard/events")
        .join(format!("{session}.jsonl"));
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

/// The key of the stage `shape` and of the one submission.
fn keys(yard: &Yard) -> (String, String) {
    let read = yard.read("s1", &[]);
    let stage = read["stages"][0]["key"].as_str().unwrap().to_owned();
    let submission = read["submissions"][0]["open"][0]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    (stage, submission)
}

#[test]
fn an_event_is_kept_in_the_session_s_event_log_with_its_tree_branch_stage_and_open_submissions() {
    let (yard, served) = open();
    let (stage, submission) = keys(&yard);
    let posted = post(&served, &[event(&yard, "turn.start", 0, &json!({}))]);
    assert_eq!(posted["recorded"], 1);
    let kept = log(&yard, "s1");
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0]["kind"], "turn.start");
    assert_eq!(kept[0]["tree"], dir(&yard).as_str());
    assert_eq!(kept[0]["branch"], BRANCH);
    assert_eq!(kept[0]["stage"], stage.as_str());
    assert_eq!(kept[0]["submissions"], json!([submission]));
    let in_tree = walk(&yard.scratch.0);
    assert!(
        !in_tree.iter().any(|path| Path::new(path)
            .extension()
            .is_some_and(|extension| extension == "jsonl")),
        "{in_tree:?}"
    );
}

fn walk(dir: &std::path::Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(walk(&path));
        } else {
            found.push(path.display().to_string());
        }
    }
    found
}

#[test]
fn an_event_whose_branch_has_no_yard_log_or_a_closed_one_is_not_recorded() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    let posted = post(&served, &[event(&yard, "turn.start", 0, &json!({}))]);
    assert_eq!(posted["skipped"], 1);
    yard.write("s1", ASK, &[]);
    yard.run(&["close", "--session", "s1"]);
    let request = json!({ "model": "opus", "tokens": tokens(1, 1, 1, 1) });
    let posted = post(
        &served,
        &[
            event(&yard, "turn.start", 1, &json!({})),
            event(&yard, "request", 2, &request),
        ],
    );
    assert_eq!(posted["skipped"], 2);
    assert_eq!(log(&yard, "s1"), Vec::<Value>::new());
    assert!(session_file(&yard, "activity", "s1").is_none());
    assert!(session_file(&yard, "tokens", "s1").is_none());
}

#[test]
fn an_event_whose_session_cannot_name_a_file_is_rejected_and_nothing_is_written() {
    let (yard, served) = open();
    let mut bad = event(&yard, "turn.start", 0, &json!({}));
    bad["session"] = json!("../outside");
    let posted = post(&served, &[bad]);
    assert_eq!(posted["rejected"][0]["index"], 0);
    assert!(!yard.scratch.home().join(".yard/outside.jsonl").exists());
    assert!(!yard.scratch.home().join(".yard/events").exists());
}

#[test]
fn a_request_s_tokens_are_counted_in_all_by_stage_by_agent_and_model_and_by_open_submission() {
    let (yard, served) = open();
    let (stage, submission) = keys(&yard);
    let revision = yard.read("s1", &[])["revision"].clone();
    post(
        &served,
        &[
            event(
                &yard,
                "request",
                5,
                &json!({ "model": "opus", "tokens": tokens(10, 100, 5, 20) }),
            ),
            event(
                &yard,
                "request",
                9,
                &json!({ "model": "haiku", "agent": { "id": "a1", "type": "Explore" }, "tokens": tokens(1, 2, 3, 4) }),
            ),
            event(
                &yard,
                "compaction",
                12,
                &json!({ "before": 900, "after": 100, "tokens": tokens(7, 0, 0, 3) }),
            ),
        ],
    );
    let text = session_file(&yard, "tokens", "s1").unwrap();
    assert_eq!(text, totals_of_three_requests(&stage, &submission));
    assert_eq!(yard.read("s1", &[])["revision"], revision);
    let folder = yard.folder(BRANCH).join("tokens");
    let names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["s1.yaml"], "no temporary file is left");
}

/// The totals of the main agent's request on `opus` in stage `stage`, a subagent's on `haiku`
/// under no stage of its own, and a compaction, all while `submission` was open.
fn totals_of_three_requests(stage: &str, submission: &str) -> String {
    format!(
        "session: s1
host: claude-code
firstEnded: 2026-01-01T10:00:05Z
lastEnded: 2026-01-01T10:00:12Z
tokens:
  input: 18
  cacheRead: 102
  cacheWrite: 8
  output: 27
stages:
  - stage: {stage}
    input: 17
    cacheRead: 100
    cacheWrite: 5
    output: 23
noStage:
  input: 1
  cacheRead: 2
  cacheWrite: 3
  output: 4
agents:
  - agent: main
    model: opus
    input: 10
    cacheRead: 100
    cacheWrite: 5
    output: 20
  - agent: main
    model: unknown
    input: 7
    cacheRead: 0
    cacheWrite: 0
    output: 3
  - agent: a1
    type: Explore
    model: haiku
    input: 1
    cacheRead: 2
    cacheWrite: 3
    output: 4
submissions:
  - submission: {submission}
    input: 18
    cacheRead: 102
    cacheWrite: 8
    output: 27
"
    )
}

#[test]
fn a_session_that_made_no_counted_request_writes_no_totals() {
    let (yard, served) = open();
    post(&served, &[event(&yard, "turn.start", 0, &json!({}))]);
    assert!(session_file(&yard, "tokens", "s1").is_none());
    assert!(session_file(&yard, "activity", "s1").is_some());
}

#[test]
fn the_totals_go_on_from_the_event_log_after_the_page_server_is_replaced() {
    let (yard, served) = open();
    post(
        &served,
        &[event(
            &yard,
            "request",
            5,
            &json!({ "model": "opus", "tokens": tokens(10, 0, 0, 1) }),
        )],
    );
    drop(served);
    let again = Served::start(&yard, &[]);
    post(
        &again,
        &[event(
            &yard,
            "request",
            6,
            &json!({ "model": "opus", "tokens": tokens(5, 0, 0, 1) }),
        )],
    );
    let text = session_file(&yard, "tokens", "s1").unwrap();
    assert!(text.contains("tokens:\n  input: 15\n"), "{text}");
}

#[test]
fn a_request_whose_agent_works_in_another_worktree_is_counted_in_that_worktree_s_yard_log() {
    let (yard, served) = open();
    let other = Yard::new();
    other.write("s1", ASK, &[]);
    let mut elsewhere = event(
        &yard,
        "request",
        5,
        &json!({ "model": "opus", "tokens": tokens(3, 0, 0, 1) }),
    );
    elsewhere["dir"] = json!(dir(&other));
    elsewhere["agent"] = json!({ "id": "a1", "type": "Explore" });
    let here = event(&yard, "turn.start", 6, &json!({}));
    post(&served, &[elsewhere, here]);
    assert!(session_file(&yard, "tokens", "s1").is_none());
    let text = session_file(&other, "tokens", "s1").unwrap();
    assert!(text.contains("tokens:\n  input: 3\n"), "{text}");
    let kept = log(&yard, "s1");
    assert_eq!(kept[0]["tree"], dir(&other).as_str());
    assert_eq!(kept[1]["tree"], dir(&yard).as_str());
    assert_eq!(
        kept[1]["kind"], "turn.start",
        "the events in the order they arrived"
    );
}

#[test]
fn turns_runs_plan_and_spec_mode_and_loft_runs_are_kept_with_when_they_started_and_ended() {
    let (yard, served) = open();
    let (stage, _) = keys(&yard);
    let revision = yard.read("s1", &[])["revision"].clone();
    post(&served, &a_turn_of_every_kind(&yard));
    assert_eq!(
        session_file(&yard, "activity", "s1").unwrap(),
        activity_of_the_turn(&stage)
    );
    assert_eq!(yard.read("s1", &[])["revision"], revision);
}

/// A turn of the main agent with an edit under `spec/` and one beside it, a stretch of plan mode,
/// a subagent's run, and a shell command that runs `loft` and fails.
fn a_turn_of_every_kind(yard: &Yard) -> Vec<Value> {
    let sub = json!({ "id": "a1", "type": "Explore" });
    vec![
        event(yard, "turn.start", 0, &json!({})),
        event(
            yard,
            "tool.start",
            1,
            &json!({ "tool": "Edit", "call": "e1", "spec": true }),
        ),
        event(
            yard,
            "tool.start",
            2,
            &json!({ "tool": "Edit", "call": "e2", "spec": false }),
        ),
        event(yard, "plan.start", 3, &json!({})),
        event(yard, "plan.end", 4, &json!({})),
        event(
            yard,
            "subagent.start",
            5,
            &json!({ "agent": sub, "model": "haiku" }),
        ),
        event(
            yard,
            "tool.start",
            6,
            &json!({ "tool": "Bash", "call": "b1", "loft": ["check", "fmt"] }),
        ),
        event(
            yard,
            "tool.end",
            7,
            &json!({ "tool": "Bash", "call": "b1", "failed": true }),
        ),
        event(yard, "subagent.end", 8, &json!({ "agent": sub })),
        event(
            yard,
            "turn.end",
            9,
            &json!({ "tokens": tokens(1, 1, 1, 1) }),
        ),
    ]
}

/// The activity [`a_turn_of_every_kind`] keeps, its `loft` run in the stage `stage`.
fn activity_of_the_turn(stage: &str) -> String {
    format!(
        "session: s1
lastEnded: 2026-01-01T10:00:09Z
turns:
  - started: 2026-01-01T10:00:00Z
    ended: 2026-01-01T10:00:09Z
subagents:
  - agent: a1
    type: Explore
    started: 2026-01-01T10:00:05Z
    ended: 2026-01-01T10:00:08Z
plan:
  - started: 2026-01-01T10:00:03Z
    ended: 2026-01-01T10:00:04Z
spec:
  - started: 2026-01-01T10:00:01Z
    ended: 2026-01-01T10:00:02Z
loft:
  - commands:
      - check
      - fmt
    agent: main
    started: 2026-01-01T10:00:06Z
    ended: 2026-01-01T10:00:07Z
    failed: true
    stage: {stage}
"
    )
}

#[test]
fn a_turn_under_way_when_yard_log_opened_is_kept_from_the_first_event_heard() {
    let (yard, served) = open();
    post(
        &served,
        &[event(
            &yard,
            "tool.start",
            4,
            &json!({ "tool": "Read", "call": "r1" }),
        )],
    );
    let text = session_file(&yard, "activity", "s1").unwrap();
    assert!(
        text.contains("turns:\n  - started: 2026-01-01T10:00:04Z\n"),
        "{text}"
    );
}

#[test]
fn a_subagent_s_spec_edits_and_plan_mode_are_not_kept_as_stretches() {
    let (yard, served) = open();
    let sub = json!({ "id": "a1", "type": "Plan" });
    post(
        &served,
        &[
            event(&yard, "turn.start", 0, &json!({})),
            event(
                &yard,
                "tool.start",
                1,
                &json!({ "agent": sub, "tool": "Edit", "call": "e1", "spec": true }),
            ),
            event(&yard, "plan.start", 2, &json!({ "agent": sub })),
        ],
    );
    let text = session_file(&yard, "activity", "s1").unwrap();
    assert!(!text.contains("spec:"), "{text}");
    assert!(!text.contains("plan:"), "{text}");
}

#[test]
fn an_event_that_starts_or_ends_nothing_leaves_the_activity_file_as_it_was() {
    let (yard, served) = open();
    post(&served, &[event(&yard, "turn.start", 0, &json!({}))]);
    let path = yard.folder(BRANCH).join("activity/s1.yaml");
    let before = (
        fs::read(&path).unwrap(),
        fs::metadata(&path).unwrap().modified().unwrap(),
    );
    post(
        &served,
        &[event(
            &yard,
            "tool.start",
            1,
            &json!({ "tool": "Read", "call": "r1" }),
        )],
    );
    let after = (
        fs::read(&path).unwrap(),
        fs::metadata(&path).unwrap().modified().unwrap(),
    );
    assert_eq!(before, after);
}

#[test]
fn no_other_session_s_activity_file_is_written_for_a_session_s_events() {
    let (yard, served) = open();
    post(&served, &[event(&yard, "turn.start", 0, &json!({}))]);
    let names: Vec<String> = fs::read_dir(yard.folder(BRANCH).join("activity"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["s1.yaml"]);
}
