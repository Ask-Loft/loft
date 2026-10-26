//! Codex's native hook boundary, run through the shipped binary with simulated git.

use std::fs;

use serde_json::{Value, json};

use crate::common::{stderr, stdout};
use crate::yard::Yard;

pub fn event(yard: &Yard, name: &str) -> Value {
    json!({
        "session_id": "codex-session",
        "cwd": yard.scratch.0.canonicalize().unwrap(),
        "hook_event_name": name,
        "turn_id": "turn-one",
        "permission_mode": "default",
        "model": "test-model",
    })
}

pub fn hook(yard: &Yard, event: &Value) -> Value {
    let output = yard.run_with(&["codex", "hook"], &event.to_string());
    assert!(output.status.success(), "{}", stderr(&output));
    serde_json::from_slice(&output.stdout).unwrap()
}

pub fn state(yard: &Yard) -> Value {
    let file = yard
        .scratch
        .home()
        .join(".yard/codex/codex-session/state.json");
    serde_json::from_slice(&fs::read(file).unwrap()).unwrap()
}

#[test]
fn invalid_codex_hook_identity_changes_no_adapter_state() {
    let yard = Yard::new();
    for (field, invalid) in [("session_id", "../bad"), ("cwd", "relative")] {
        let mut input = event(&yard, "UserPromptSubmit");
        input[field] = json!(invalid);
        let output = yard.run_with(&["codex", "hook"], &input.to_string());
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        let failed: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(failed["outcome"], "error");
        assert!(!yard.scratch.home().join(".yard/codex").exists());
    }
    let malformed = yard.run_with(&["codex", "hook"], "{not json");
    assert_eq!(malformed.status.code(), Some(1));
    assert!(!yard.scratch.home().join(".yard/codex").exists());
}

#[test]
fn codex_activity_declares_coverage_without_retaining_native_words() {
    let yard = Yard::new();
    let mut input = event(&yard, "UserPromptSubmit");
    input["prompt"] = json!("PRIVATE PROMPT");
    hook(&yard, &input);
    input["hook_event_name"] = json!("PreToolUse");
    input["tool_name"] = json!("Bash");
    input["tool_use_id"] = json!("call-one");
    input["tool_input"] = json!({"command":"echo PRIVATE COMMAND"});
    hook(&yard, &input);
    input["hook_event_name"] = json!("PostToolUse");
    input["tool_response"] = json!({"secret":"PRIVATE RESULT"});
    hook(&yard, &input);
    let kept = state(&yard);
    let events = kept["events"].as_array().unwrap();
    assert_eq!(events.len(), 3, "{kept}");
    assert_eq!(events[0]["kind"], "turn.start");
    assert_eq!(events[1]["call"], "call-one");
    assert_eq!(events[2]["kind"], "tool.end");
    assert!(events[2].get("failed").is_none());
    for event in events {
        assert_eq!(event["host"], "codex");
        assert_eq!(event["session"], "codex-session");
        assert_eq!(event["dir"], json!(yard.scratch.0.canonicalize().unwrap()));
        assert_eq!(event["at"], "2026-01-01T00:00:00Z");
        assert_eq!(event["adapter"]["capabilities"]["usage"], json!([]));
        assert_eq!(event["adapter"]["capabilities"]["nativeQuestions"], false);
        assert_eq!(event["adapter"]["capabilities"]["nativePlans"], false);
        assert_eq!(event["adapter"]["capabilities"]["taskMirroring"], true);
        assert!(event.get("tokens").is_none());
    }
    assert!(!kept.to_string().contains("PRIVATE"));
}

#[test]
fn codex_hooks_record_observed_modes_compaction_and_subagent_lifetimes() {
    let yard = Yard::new();
    let mut input = event(&yard, "UserPromptSubmit");
    input["permission_mode"] = json!("plan");
    hook(&yard, &input);
    input["hook_event_name"] = json!("SubagentStart");
    input["agent_id"] = json!("child-one");
    input["agent_type"] = json!("reviewer");
    hook(&yard, &input);
    input["hook_event_name"] = json!("SubagentStop");
    hook(&yard, &input);
    input.as_object_mut().unwrap().remove("agent_id");
    input["hook_event_name"] = json!("PostCompact");
    hook(&yard, &input);
    input["hook_event_name"] = json!("Interrupt");
    input["permission_mode"] = json!("default");
    hook(&yard, &input);
    let kept = state(&yard);
    let kinds: Vec<_> = kept["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "plan.start",
            "turn.start",
            "subagent.start",
            "subagent.end",
            "compaction",
            "plan.end",
            "turn.end"
        ]
    );
    assert_eq!(kept["events"][2]["agent"]["id"], "child-one");
    assert!(kept["events"][4].get("before").is_none());
}

#[test]
fn codex_session_context_is_a_peek_with_explicit_caller_arguments() {
    let yard = Yard::new();
    yard.write(
        "codex-session",
        r#"[{"id":"build","kind":"stage","title":"Build","goal":"It works","status":"active"}]"#,
        &["--host", "codex"],
    );
    yard.read("codex-session", &["--host", "codex", "--mark-read"]);
    let sent = yard.write(
        "codex-session",
        r#"[{"id":"later","kind":"question","stage":"build","prompt":"Continue?","header":"Next","options":[{"label":"Yes"},{"label":"No"}]}]"#,
        &["--host", "codex", "--send"],
    );
    let read_before = fs::read(yard.scratch.home().join(".yard/last-read.json")).ok();
    let context = hook(&yard, &event(&yard, "SessionStart"));
    let text = context["hookOutput"]["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        text.contains("--host codex --session codex-session"),
        "{text}"
    );
    assert!(text.contains("build"), "{text}");
    assert!(text.contains("Revision: 2; last read: 1"), "{text}");
    assert!(text.contains("Pending submissions:"), "{text}");
    assert!(
        text.contains(sent["submission"].as_str().unwrap()),
        "{text}"
    );
    assert!(text.contains("Page:"), "{text}");
    assert!(text.chars().count() <= 2000);
    assert_eq!(
        fs::read(yard.scratch.home().join(".yard/last-read.json")).ok(),
        read_before
    );
    let mut other = event(&yard, "SessionStart");
    other["session_id"] = json!("a-distinct-session");
    hook(&yard, &other);
    assert!(
        !yard
            .scratch
            .home()
            .join(".yard/continuations.json")
            .exists()
    );
}

#[test]
fn an_unknown_codex_hook_is_an_empty_native_response() {
    let yard = Yard::new();
    let input = event(&yard, "FutureEvent");
    let path = yard.scratch.home().join("native-hook.json");
    fs::write(&path, input.to_string()).unwrap();
    let output = yard.scratch.loft_with(
        &["yard", "codex", "hook", "--input", path.to_str().unwrap()],
        Some(&yard.git),
        &yard.clock,
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&output)).unwrap(),
        json!({})
    );
    assert!(!yard.scratch.home().join(".yard/codex").exists());
}

#[test]
fn codex_context_distinguishes_absent_closed_and_ignored_logs_and_supplies_subagent_identity() {
    let mut yard = Yard::new();
    let absent = hook(&yard, &event(&yard, "SessionStart"));
    assert!(absent.to_string().contains("no Yard Log yet"));
    yard.write(
        "codex-session",
        r#"[{"id":"build","kind":"stage","title":"Build","status":"active"}]"#,
        &["--host", "codex"],
    );
    yard.git = yard.git.clone().ignoring(":/.yard/");
    let ignored = hook(&yard, &event(&yard, "SessionStart"));
    assert!(ignored.to_string().contains("Git ignores .yard/"));
    let mut child = event(&yard, "SubagentStart");
    child["agent_id"] = json!("child");
    child["agent_type"] = json!("reviewer");
    let context = hook(&yard, &child);
    assert!(
        context
            .to_string()
            .contains("--agent child --agent-type reviewer")
    );
    let end = yard.run(&["close", "--session", "codex-session", "--host", "codex"]);
    assert!(end.status.success(), "{}", stderr(&end));
    let closed = hook(&yard, &event(&yard, "SessionStart"));
    assert!(closed.to_string().contains("Yard Log is closed"));
}

#[test]
fn codex_omits_ambiguous_agent_events_and_keeps_explicit_tool_failures_distinct_from_unknown() {
    let yard = Yard::new();
    let mut input = event(&yard, "SubagentStart");
    input["agent_id"] = json!("child");
    input["agent_type"] = json!("reviewer");
    hook(&yard, &input);
    let mut tool = event(&yard, "PostToolUse");
    tool["tool_name"] = json!("Bash");
    tool["tool_use_id"] = json!("call");
    tool["tool_response"] = json!({"exit_code":7});
    hook(&yard, &tool);
    assert_eq!(state(&yard)["events"].as_array().unwrap().len(), 1);
    tool["agent_id"] = json!("child");
    hook(&yard, &tool);
    let kept = state(&yard);
    assert_eq!(kept["events"][1]["agent"]["id"], "child");
    assert_eq!(kept["events"][1]["failed"], true);
    tool["tool_response"] = json!({"exit_code":0});
    hook(&yard, &tool);
    assert_eq!(state(&yard)["events"][2]["failed"], false);
    tool["tool_response"] = json!("an opaque result");
    hook(&yard, &tool);
    assert!(state(&yard)["events"][3].get("failed").is_none());
}

#[test]
fn a_codex_hook_queues_activity_without_contacting_the_page_server() {
    let yard = Yard::new();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let input = event(&yard, "UserPromptSubmit");
    let output = yard.run_with(
        &[
            "codex",
            "hook",
            "--port",
            &listener.local_addr().unwrap().port().to_string(),
        ],
        &input.to_string(),
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    assert_eq!(state(&yard)["events"].as_array().unwrap().len(), 1);
}

#[test]
fn a_codex_session_end_discards_subagents_before_the_session_resumes() {
    let yard = Yard::new();
    let mut child = event(&yard, "SubagentStart");
    child["agent_id"] = json!("child");
    child["agent_type"] = json!("reviewer");
    hook(&yard, &child);
    hook(&yard, &event(&yard, "SessionEnd"));
    hook(&yard, &event(&yard, "SessionStart"));
    let mut tool = event(&yard, "PreToolUse");
    tool["tool_name"] = json!("Bash");
    tool["tool_use_id"] = json!("call-after-resume");
    hook(&yard, &tool);
    let kept = state(&yard);
    assert_eq!(kept["events"][1]["call"], "call-after-resume");
    assert!(kept["events"][1].get("agent").is_none());
}
