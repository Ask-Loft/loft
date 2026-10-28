//! The Claude Yard hook's events and the page's decision of a plan: plan mode and compactions
//! posted to a running page server, and a `PermissionRequest` for `ExitPlanMode` answered with
//! what the page decided.

use std::fs;
use std::process::Child;
use std::time::Duration;

use serde_json::{Value, json};

use crate::served::{Served, within};
use crate::yard::{BRANCH, Yard, document};
use crate::yard_claude_hook::{hook, input};
use crate::yard_serve_events::{log, open, session_file};

/// A hook of `event` of session `s1`'s main agent naming the permission mode `mode`.
fn moded(yard: &Yard, event: &str, mode: &str) -> Value {
    let mut moded = input(yard, event, "s1");
    moded["permission_mode"] = json!(mode);
    moded
}

/// The kinds of the events session `s1`'s event log holds, in order.
fn kinds(yard: &Yard) -> Vec<String> {
    log(yard, "s1")
        .iter()
        .map(|event| event["kind"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn plan_mode_starting_and_ending_is_posted_once_for_each_change_of_the_main_agent_s_mode() {
    let (yard, _served) = open();
    for (event, mode) in [
        ("UserPromptSubmit", "default"),
        ("PostToolUse", "default"),
        ("Stop", "plan"),
        ("PostToolUse", "plan"),
        ("UserPromptSubmit", "plan"),
        ("Stop", "auto"),
        ("UserPromptSubmit", "acceptEdits"),
    ] {
        hook(&yard, &moded(&yard, event, mode), None);
    }
    assert_eq!(kinds(&yard), ["plan.start", "plan.end"]);
}

#[test]
fn a_session_first_heard_in_plan_mode_starts_it_and_one_first_heard_outside_says_nothing() {
    let (yard, _served) = open();
    hook(&yard, &moded(&yard, "Stop", "auto"), None);
    assert_eq!(kinds(&yard), Vec::<String>::new());
    let mut other = moded(&yard, "UserPromptSubmit", "plan");
    other["session_id"] = json!("s2");
    hook(&yard, &other, None);
    assert_eq!(log(&yard, "s2")[0]["kind"], "plan.start");
}

#[test]
fn a_subagent_s_permission_mode_posts_nothing_and_leaves_the_session_s_mode() {
    let (yard, _served) = open();
    let mut sub = moded(&yard, "PostToolUse", "plan");
    sub["agent_id"] = json!("worker");
    sub["agent_type"] = json!("Plan");
    hook(&yard, &sub, None);
    hook(&yard, &moded(&yard, "Stop", "default"), None);
    assert_eq!(kinds(&yard), Vec::<String>::new());
}

#[test]
fn each_session_s_last_mode_is_kept_in_the_yard_home_and_never_in_the_tree() {
    let (yard, _served) = open();
    let tree = fs::read_dir(&yard.scratch.0).unwrap().count();
    hook(&yard, &moded(&yard, "Stop", "plan"), None);
    let kept: Value = serde_json::from_str(
        &fs::read_to_string(yard.scratch.home().join(".yard/claude/modes.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(kept, json!({"s1": true}));
    assert_eq!(fs::read_dir(&yard.scratch.0).unwrap().count(), tree);
}

#[test]
fn a_manual_and_an_automatic_compaction_are_each_one_event_with_no_sizes_and_no_usage() {
    let (yard, _served) = open();
    for trigger in ["manual", "auto"] {
        let mut compacted = input(&yard, "PostCompact", "s1");
        compacted["trigger"] = json!(trigger);
        compacted["compact_summary"] = json!("what was said");
        hook(&yard, &compacted, None);
    }
    let events = log(&yard, "s1");
    assert_eq!(kinds(&yard), ["compaction", "compaction"]);
    for event in &events {
        for field in ["before", "after", "tokens"] {
            assert!(event.get(field).is_none(), "{event}");
        }
    }
    assert_eq!(session_file(&yard, "tokens", "s1"), None);
}

#[test]
fn an_event_names_the_host_the_session_and_no_agent_and_holds_none_of_the_hook_s_words() {
    let (yard, _served) = open();
    let mut prompt = moded(&yard, "UserPromptSubmit", "plan");
    prompt["prompt"] = json!("a secret request");
    hook(&yard, &prompt, None);
    let events = log(&yard, "s1");
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event["host"], "claude-code");
    assert_eq!(event["session"], "s1");
    assert!(event.get("agent").is_none(), "{event}");
    assert!(!event.to_string().contains("secret"), "{event}");
}

#[test]
fn a_post_compact_hook_with_no_page_server_running_exits_0_and_drops_its_event() {
    let yard = Yard::new();
    let answered = hook(&yard, &input(&yard, "PostCompact", "s1"), None);
    assert_eq!(answered["hookOutput"], json!({}));
    assert!(!yard.scratch.home().join(".yard/events").exists());
}

/// Yard Log with a plan-mode plan, `p1`, sent by session `s1`'s agent named by `agent`, and a
/// page server.
fn presented(agent: &[&str]) -> (Yard, Served) {
    let yard = Yard::new();
    let mut args = vec!["--send"];
    args.extend_from_slice(agent);
    yard.write(
        "s1",
        r#"[{"id":"build","kind":"stage","title":"Build","status":"active"},
            {"id":"p1","kind":"plan","stage":"build","title":"The plan","markdown":"1. Do it","planMode":true}]"#,
        &args,
    );
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    (yard, served)
}

/// The permission request for `ExitPlanMode` of the hook's agent, running.
fn asking(yard: &Yard, agent: Option<&str>) -> Child {
    let mut request = input(yard, "PermissionRequest", "s1");
    request["tool_name"] = json!("ExitPlanMode");
    request["tool_input"] = json!({"plan": "1. Do it", "planFilePath": "/plans/p.md"});
    if let Some(agent) = agent {
        request["agent_id"] = json!(agent);
        request["agent_type"] = json!("Plan");
    }
    let path = yard.scratch.home().join("permission.json");
    fs::write(&path, request.to_string()).unwrap();
    yard.spawn(&["claude", "hook", "--input", path.to_str().unwrap()])
}

/// The document the running hook prints once it ends, held to its schema.
fn answered(mut child: Child) -> Value {
    assert!(
        within(Duration::from_secs(10), || child
            .try_wait()
            .unwrap()
            .is_some()),
        "the hook ended"
    );
    let output = child.wait_with_output().unwrap();
    document("claude-hook", &output)
}

/// Decides `p1` on the page.
fn decide(yard: &Yard, served: &Served, outcome: &Value) {
    let tree = yard.scratch.0.canonicalize().unwrap().display().to_string();
    let sent = json!({"tree": tree, "branch": BRANCH, "answers": [outcome]});
    let reply = served.post_as_page("/api/answer", &sent);
    assert_eq!(reply.status, 200, "{}", reply.json());
}

/// Gives a running hook time to begin its wait before the person decides.
fn settle() {
    std::thread::sleep(Duration::from_millis(400));
}

#[test]
fn a_plan_approved_on_the_page_is_allowed_with_its_input_and_auto_mode() {
    let (yard, served) = presented(&[]);
    let child = asking(&yard, None);
    settle();
    decide(
        &yard,
        &served,
        &json!({"block": "p1", "verdict": "approved"}),
    );
    let output = answered(child);
    assert_eq!(
        output["hookOutput"]["hookSpecificOutput"],
        json!({"hookEventName": "PermissionRequest", "decision": {
            "behavior": "allow",
            "updatedInput": {"plan": "1. Do it", "planFilePath": "/plans/p.md"},
            "updatedPermissions": [{"type": "setMode", "mode": "auto", "destination": "session"}],
        }})
    );
}

#[test]
fn a_plan_rejected_on_the_page_is_denied_with_the_person_s_reason() {
    let (yard, served) = presented(&[]);
    let child = asking(&yard, None);
    settle();
    decide(
        &yard,
        &served,
        &json!({"block": "p1", "verdict": "rejected", "reason": "too big"}),
    );
    let output = answered(child);
    assert_eq!(
        output["hookOutput"]["hookSpecificOutput"]["decision"],
        json!({"behavior": "deny", "message": "The person rejected the plan on the Yard page: too big"})
    );
}

#[test]
fn a_plan_decided_in_the_terminal_first_ends_the_wait_and_the_hook_answers_nothing() {
    let (yard, _served) = presented(&[]);
    let child = asking(&yard, None);
    settle();
    yard.answer("s1", r#"[{"block": "p1", "verdict": "approved"}]"#);
    assert_eq!(answered(child)["hookOutput"], json!({}));
}

#[test]
fn with_no_plan_open_for_the_hook_s_agent_the_request_is_answered_at_once_with_nothing() {
    let (yard, _served) = presented(&["--agent", "planner", "--agent-type", "Plan"]);
    let child = asking(&yard, None);
    assert_eq!(answered(child)["hookOutput"], json!({}));
    let mut other = input(&yard, "PermissionRequest", "s1");
    other["tool_name"] = json!("Bash");
    assert_eq!(hook(&yard, &other, None)["hookOutput"], json!({}));
}

#[test]
fn a_subagent_s_request_waits_on_the_plan_that_subagent_presented() {
    let (yard, served) = presented(&["--agent", "planner", "--agent-type", "Plan"]);
    let child = asking(&yard, Some("planner"));
    settle();
    decide(
        &yard,
        &served,
        &json!({"block": "p1", "verdict": "approved"}),
    );
    let output = answered(child);
    assert_eq!(
        output["hookOutput"]["hookSpecificOutput"]["decision"]["behavior"],
        "allow"
    );
}
