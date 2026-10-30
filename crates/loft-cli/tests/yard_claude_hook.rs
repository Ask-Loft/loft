//! The Claude Yard hook's context and hand-off: `loft yard claude hook` run through the shipped
//! binary on Claude Code's hook objects, with simulated git and a fixed clock.

use std::fs;

use serde_json::{Value, json};
use software_plant::{Clock, GitScenario};

use crate::common::{EPOCH, stderr};
use crate::yard::{Yard, document};

/// The line the context ends with, as the Loft mod gives it.
const FOOTER: &str = "Read Yard Log with `yard_read` before writing.";

/// A hook object of Claude Code's for `event`, in the scratch tree, for `session`.
pub fn input(yard: &Yard, event: &str, session: &str) -> Value {
    json!({
        "session_id": session,
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": yard.scratch.0.canonicalize().unwrap(),
        "hook_event_name": event,
        "permission_mode": "default",
    })
}

/// Runs the hook on `input`, handed the Claude Code process `pid` where one is given, and gives
/// its document, held to its schema.
pub fn hook(yard: &Yard, input: &Value, pid: Option<&str>) -> Value {
    let mut args = vec!["claude", "hook"];
    if let Some(pid) = pid {
        args.extend(["--host-pid", pid]);
    }
    let output = yard.run_with(&args, &input.to_string());
    assert!(output.status.success(), "{}", stderr(&output));
    document("claude-hook", &output)
}

/// The context a hook's document gives, where it gives one.
pub fn context(document: &Value) -> Option<&str> {
    document["hookOutput"]["hookSpecificOutput"]["additionalContext"].as_str()
}

/// A session start of `source` for `session`.
fn start(yard: &Yard, session: &str, source: &str) -> Value {
    let mut start = input(yard, "SessionStart", session);
    start["source"] = json!(source);
    start
}

/// A session end of `reason` for `session`.
fn end(yard: &Yard, session: &str, reason: &str) -> Value {
    let mut end = input(yard, "SessionEnd", session);
    end["reason"] = json!(reason);
    end
}

/// Session `s1` opens Yard Log with an active stage and sends a question; the submission's key.
fn sent(yard: &Yard, session: &str) -> String {
    yard.write(
        session,
        r#"[{"id":"build","kind":"stage","title":"Build","goal":"It works","status":"active"}]"#,
        &[],
    );
    let sent = yard.write(
        session,
        r#"[{"id":"q1","kind":"question","stage":"build","prompt":"Continue?","header":"Next","options":[{"label":"Yes"},{"label":"No"}]}]"#,
        &["--send"],
    );
    sent["submission"].as_str().unwrap().to_owned()
}

/// The sessions `~/.yard/continuations.json` records as continuing others.
fn continuations(yard: &Yard) -> Value {
    let path = yard.scratch.home().join(".yard/continuations.json");
    fs::read_to_string(path).map_or(json!({}), |text| serde_json::from_str(&text).unwrap())
}

#[test]
fn a_session_start_gives_the_main_agent_its_briefing_and_the_line_that_tells_it_to_read() {
    let yard = Yard::new();
    let submission = sent(&yard, "s1");
    let briefing = yard.read("s1", &["--context"])["context"]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(briefing.contains(&submission), "{briefing}");
    let answered = hook(&yard, &start(&yard, "s1", "startup"), None);
    assert_eq!(
        answered["hookOutput"]["hookSpecificOutput"]["hookEventName"],
        "SessionStart"
    );
    assert_eq!(
        context(&answered),
        Some(format!("{briefing}\n{FOOTER}").as_str())
    );
}

#[test]
fn the_briefing_s_read_is_recorded_as_the_main_agent_s_last() {
    let yard = Yard::new();
    sent(&yard, "s1");
    assert!(yard.read("s1", &[])["lastRead"].is_null());
    hook(&yard, &start(&yard, "s1", "resume"), None);
    assert_eq!(yard.read("s1", &[])["lastRead"], 2);
}

#[test]
fn a_session_start_after_a_compaction_gives_no_context() {
    let yard = Yard::new();
    sent(&yard, "s1");
    let answered = hook(&yard, &start(&yard, "s1", "compact"), None);
    assert_eq!(answered["hookOutput"], json!({}));
    assert!(yard.read("s1", &[])["lastRead"].is_null());
}

#[test]
fn a_session_start_of_each_other_source_gives_the_context() {
    let yard = Yard::new();
    sent(&yard, "s1");
    for source in ["startup", "resume", "clear", "fork"] {
        let answered = hook(&yard, &start(&yard, "s1", source), None);
        assert!(context(&answered).is_some(), "{source}: {answered}");
    }
}

#[test]
fn a_subagent_s_briefing_names_its_assigned_tasks_and_is_read_in_its_own_name() {
    let yard = Yard::new();
    yard.write(
        "s1",
        r#"[{"id":"build","kind":"stage","title":"Build","status":"active"},
            {"id":"work","kind":"task","stage":"build","title":"Work","state":"open",
             "assignee":{"host":"claude-code","session":"s1","agent":"worker","agentType":"builder"}}]"#,
        &[],
    );
    let mut started = input(&yard, "SubagentStart", "s1");
    started["agent_id"] = json!("worker");
    started["agent_type"] = json!("builder");
    let answered = hook(&yard, &started, None);
    assert_eq!(
        answered["hookOutput"]["hookSpecificOutput"]["hookEventName"],
        "SubagentStart"
    );
    let text = context(&answered).unwrap();
    assert!(text.contains("`work`"), "{text}");
    assert!(text.contains("`build`"), "{text}");
    assert!(text.ends_with(FOOTER), "{text}");
    let worker = ["--agent", "worker", "--agent-type", "builder"];
    assert_eq!(yard.read("s1", &worker)["lastRead"], 1);
}

#[test]
fn the_session_after_clear_is_briefed_as_continuing_the_one_that_ended() {
    let yard = Yard::new();
    let submission = sent(&yard, "s1");
    yard.read("s1", &["--mark-read"]);
    hook(&yard, &end(&yard, "s1", "clear"), Some("4242"));
    let answered = hook(&yard, &start(&yard, "s2", "clear"), Some("4242"));
    assert_eq!(continuations(&yard)["s2"], "s1");
    let text = context(&answered).unwrap();
    assert!(text.contains(&submission), "{text}");
}

#[test]
fn a_hand_off_is_taken_once_and_only_in_the_process_that_left_it() {
    let yard = Yard::new();
    sent(&yard, "s1");
    hook(&yard, &end(&yard, "s1", "resume"), Some("4242"));
    hook(&yard, &start(&yard, "s2", "clear"), Some("5151"));
    assert_eq!(continuations(&yard), json!({}));
    hook(&yard, &start(&yard, "s2", "resume"), Some("4242"));
    assert_eq!(continuations(&yard), json!({"s2": "s1"}));
    hook(&yard, &start(&yard, "s3", "clear"), Some("4242"));
    assert_eq!(continuations(&yard), json!({"s2": "s1"}));
}

#[test]
fn a_session_start_of_a_new_session_takes_no_hand_off() {
    let yard = Yard::new();
    sent(&yard, "s1");
    hook(&yard, &end(&yard, "s1", "clear"), Some("4242"));
    for source in ["startup", "fork", "compact"] {
        hook(&yard, &start(&yard, "s2", source), Some("4242"));
    }
    assert_eq!(continuations(&yard), json!({}));
}

#[test]
fn a_hand_off_left_more_than_a_minute_before_is_discarded() {
    let mut yard = Yard::new();
    sent(&yard, "s1");
    hook(&yard, &end(&yard, "s1", "clear"), Some("4242"));
    yard.clock = Clock::fixed(EPOCH + 61);
    hook(&yard, &start(&yard, "s2", "clear"), Some("4242"));
    assert_eq!(continuations(&yard), json!({}));
    yard.clock = Clock::fixed(EPOCH);
    hook(&yard, &end(&yard, "s1", "clear"), Some("4242"));
    yard.clock = Clock::fixed(EPOCH + 60);
    hook(&yard, &start(&yard, "s2", "clear"), Some("4242"));
    assert_eq!(continuations(&yard), json!({"s2": "s1"}));
}

#[test]
fn a_hook_given_no_claude_code_process_leaves_and_takes_no_hand_off() {
    let yard = Yard::new();
    sent(&yard, "s1");
    hook(&yard, &end(&yard, "s1", "clear"), None);
    assert!(!yard.scratch.home().join(".yard/claude").exists());
    let answered = hook(&yard, &start(&yard, "s2", "clear"), None);
    assert_eq!(continuations(&yard), json!({}));
    assert!(context(&answered).is_some());
    assert_eq!(yard.read("s2", &[])["lastRead"], 2);
}

#[test]
fn hand_offs_are_kept_in_the_yard_home_and_never_in_the_tree() {
    let yard = Yard::new();
    sent(&yard, "s1");
    let tree: Vec<_> = fs::read_dir(&yard.scratch.0).unwrap().collect();
    hook(&yard, &end(&yard, "s1", "clear"), Some("4242"));
    let kept: Value = serde_json::from_str(
        &fs::read_to_string(yard.scratch.home().join(".yard/claude/handoffs.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(kept["4242"]["session"], "s1");
    assert_eq!(
        fs::read_dir(&yard.scratch.0).unwrap().count(),
        tree.len(),
        "the hand-off adds nothing to the tree"
    );
}

#[test]
fn a_session_start_outside_a_git_repository_answers_nothing_and_exits_0() {
    let mut yard = Yard::new();
    yard.git = GitScenario::outside_repository();
    let answered = hook(&yard, &start(&yard, "s1", "startup"), None);
    assert_eq!(answered["hookOutput"], json!({}));
}

#[test]
fn a_hook_of_an_event_the_command_does_not_serve_answers_nothing() {
    let yard = Yard::new();
    sent(&yard, "s1");
    let answered = hook(&yard, &input(&yard, "Notification", "s1"), Some("4242"));
    assert_eq!(answered["hookOutput"], json!({}));
    assert!(!yard.scratch.home().join(".yard/claude").exists());
}

#[test]
fn natively_the_hook_prints_claude_code_s_output_alone_and_nothing_for_no_answer() {
    let yard = Yard::new();
    sent(&yard, "s1");
    let path = yard.scratch.home().join("hook.json");
    fs::write(&path, start(&yard, "s1", "startup").to_string()).unwrap();
    let path = path.to_str().unwrap();
    let native = yard.scratch.loft_with(
        &["yard", "claude", "hook", "--input", path],
        Some(&yard.git),
        &yard.clock,
    );
    let printed: Value = serde_json::from_slice(&native.stdout).unwrap();
    assert_eq!(
        printed["hookSpecificOutput"]["hookEventName"],
        "SessionStart"
    );
    fs::write(path, start(&yard, "s1", "compact").to_string()).unwrap();
    let quiet = yard.scratch.loft_with(
        &["yard", "claude", "hook", "--input", path],
        Some(&yard.git),
        &yard.clock,
    );
    assert!(quiet.status.success());
    assert_eq!(quiet.stdout, b"");
}

#[test]
fn a_malformed_hook_object_is_refused_and_changes_nothing() {
    let yard = Yard::new();
    for (field, invalid) in [("session_id", json!("../bad")), ("cwd", json!("relative"))] {
        let mut bad = start(&yard, "s1", "startup");
        bad[field] = invalid;
        let output = yard.run_with(&["claude", "hook", "--host-pid", "1"], &bad.to_string());
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        assert_eq!(document("claude-hook", &output)["outcome"], "error");
    }
    let output = yard.run_with(&["claude", "hook"], "{not json");
    assert_eq!(output.status.code(), Some(1));
    assert!(!yard.scratch.home().join(".yard/claude").exists());
}

#[test]
fn a_session_start_briefs_the_main_agent_on_its_working_tree_in_another_worktree() {
    let yard = Yard::new();
    let there = crate::common::Scratch::empty();
    let tree = there.0.display().to_string();
    yard.write(
        "s1",
        r#"[{"id":"there","kind":"stage","title":"There","status":"active"}]"#,
        &["--tree", &tree],
    );
    let answered = hook(&yard, &start(&yard, "s1", "startup"), None);
    let given = context(&answered).unwrap();
    let top = there.0.canonicalize().unwrap().display().to_string();
    assert!(given.contains(&top), "{given}");
    assert!(given.contains("There"), "{given}");
}
