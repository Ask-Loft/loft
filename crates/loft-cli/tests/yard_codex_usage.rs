//! The unchanged worker against the Software Plant's local app-server peer.
#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::Child;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use software_plant::codex::{CodexAppServer, Scenario};

use crate::served::{Served, within};
use crate::yard::Yard;
use crate::yard_codex::{event, hook, state};

struct Worker<'a>(Child, &'a Yard);
impl Drop for Worker<'_> {
    fn drop(&mut self) {
        hook(self.1, &event(self.1, "SessionEnd"));
        let _ = within(Duration::from_secs(2), || {
            self.0.try_wait().unwrap().is_some()
        });
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn worker<'a>(yard: &'a Yard, port: u16, socket: &Path) -> Worker<'a> {
    let path = yard.scratch.home().join("watch.json");
    fs::write(&path, event(yard, "SessionStart").to_string()).unwrap();
    Worker(
        yard.spawn(&[
            "codex",
            "watch",
            "--input",
            path.to_str().unwrap(),
            "--port",
            &port.to_string(),
            "--app-server-socket",
            socket.to_str().unwrap(),
        ]),
        yard,
    )
}

fn setup() -> (Yard, CodexAppServer) {
    let yard = Yard::new();
    yard.write(
        "codex-session",
        r#"[{"id":"build","kind":"stage","title":"Build","status":"active"}]"#,
        &["--host", "codex"],
    );
    let server = CodexAppServer::start(Scenario {
        loaded: vec!["codex-session".into()],
        ..Scenario::default()
    });
    (yard, server)
}

fn ready(yard: &Yard) {
    assert!(within(
        Duration::from_secs(3),
        || state(yard)["usage"]["active"]["codex-session"]["source"] == "available"
    ));
}

fn usage(total: u64) -> Value {
    let last = json!({"inputTokens":100,"cachedInputTokens":30,"cacheWriteInputTokens":0,"outputTokens":5,"reasoningOutputTokens":2,"totalTokens":105});
    let mut cumulative = last.clone();
    cumulative["inputTokens"] = json!(total * 100);
    cumulative["cachedInputTokens"] = json!(total * 30);
    cumulative["outputTokens"] = json!(total * 5);
    cumulative["totalTokens"] = json!(total * 105);
    json!({"method":"thread/tokenUsage/updated","params":{"threadId":"codex-session","turnId":"turn-one","tokenUsage":{"last":last,"total":cumulative}}})
}

fn recorded(yard: &Yard) -> Vec<Value> {
    fs::read_to_string(
        yard.scratch
            .home()
            .join(".yard/events/codex~codex-session.jsonl"),
    )
    .unwrap_or_default()
    .lines()
    .map(|line| serde_json::from_str(line).unwrap())
    .collect()
}

#[test]
fn native_turn_usage_is_normalized_once_and_the_subscription_is_released_on_completion() {
    let (yard, server) = setup();
    let page = Served::start(&yard, &[]);
    hook(&yard, &event(&yard, "UserPromptSubmit"));
    let _worker = worker(&yard, page.port, server.path());
    ready(&yard);
    let mut invalid = usage(1);
    invalid["params"]["tokenUsage"]["last"]["cachedInputTokens"] = json!(101);
    server.emit(&invalid);
    let observed = usage(20);
    server.emit(&observed);
    server.emit(&observed);
    server.emit(&json!({"method":"item/started","params":{"text":"PRIVATE TRANSCRIPT"}}));
    let mut foreign = usage(21);
    foreign["params"]["turnId"] = json!("foreign-turn");
    server.emit(&foreign);
    server.emit(&json!({"method":"turn/completed","params":{"threadId":"codex-session","turn":{"id":"turn-one"}}}));
    assert!(within(Duration::from_secs(3), || server
        .requests()
        .iter()
        .any(|r| r["method"] == "thread/unsubscribe")));
    assert!(within(Duration::from_secs(3), || recorded(&yard)
        .iter()
        .any(|e| e["kind"] == "request")));
    let recorded = recorded(&yard);
    let requests: Vec<_> = recorded.iter().filter(|e| e["kind"] == "request").collect();
    assert_eq!(requests.len(), 1, "{recorded:?}");
    assert_eq!(
        requests[0]["tokens"],
        json!({"input":70,"cacheRead":30,"cacheWrite":0,"output":5})
    );
    assert_eq!(requests[0]["model"], "test-model");
    assert!(requests[0]["stage"].is_string());
    assert!(!state(&yard).to_string().contains("PRIVATE"));
    assert!(
        !serde_json::to_string(&recorded)
            .unwrap()
            .contains("PRIVATE")
    );
    assert!(within(Duration::from_secs(3), || {
        crate::yard_serve_events::session_file(&yard, "tokens", "codex~codex-session")
            .is_some_and(|totals| totals.contains("input: 70"))
    }));
    let totals =
        crate::yard_serve_events::session_file(&yard, "tokens", "codex~codex-session").unwrap();
    assert!(totals.contains("input: 70"), "{totals}");
}

#[test]
fn a_relative_socket_path_is_refused_before_state_or_worker_locks_are_written() {
    let yard = Yard::new();
    let output = yard.run_with(
        &["codex", "watch", "--app-server-socket", "relative.sock"],
        &event(&yard, "UserPromptSubmit").to_string(),
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(!yard.scratch.home().join(".yard/codex").exists());
}

#[test]
fn observer_readiness_does_not_hold_the_hook_lock_or_wait_for_delivery() {
    let (yard, server) = setup();
    hook(&yard, &event(&yard, "SessionStart"));
    let page = Served::start(&yard, &[]);
    let _worker = worker(&yard, page.port, server.path());
    let began = Instant::now();
    hook(&yard, &event(&yard, "UserPromptSubmit"));
    assert!(began.elapsed() < Duration::from_millis(700));
    ready(&yard);
    hook(&yard, &event(&yard, "Interrupt"));
    assert!(within(Duration::from_secs(3), || server
        .requests()
        .iter()
        .any(|r| r["method"] == "thread/unsubscribe")));
}

#[test]
fn an_unreachable_source_keeps_activity_heartbeats_and_unavailable_usage() {
    let (yard, _) = setup();
    let page = Served::start(&yard, &[]);
    hook(&yard, &event(&yard, "UserPromptSubmit"));
    let _worker = worker(
        &yard,
        page.port,
        Path::new("/tmp/no-loft-codex-source.sock"),
    );
    assert!(within(Duration::from_secs(3), || recorded(&yard).len() >= 2));
    let requests: Vec<_> = recorded(&yard)
        .into_iter()
        .filter(|e| e["kind"] == "request")
        .collect();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].get("tokens").is_none());
    assert_eq!(
        yard.home_file("sessions.json")["codex~codex-session"],
        "2026-01-01T00:00:00Z"
    );
}

#[test]
fn source_disconnect_and_worker_restart_preserve_the_checkpoint_and_exact_charge() {
    let (yard, server) = setup();
    let page = Served::start(&yard, &[]);
    hook(&yard, &event(&yard, "UserPromptSubmit"));
    let mut first = worker(&yard, page.port, server.path());
    ready(&yard);
    server.emit(&usage(1));
    assert!(within(Duration::from_secs(3), || recorded(&yard)
        .iter()
        .any(|e| e.get("tokens").is_some())));
    first.0.kill().unwrap();
    first.0.wait().unwrap();
    let mut second = worker(&yard, page.port, server.path());
    assert!(within(Duration::from_secs(3), || server
        .requests()
        .iter()
        .filter(|r| r["method"] == "thread/resume")
        .count()
        == 2));
    server.emit(&usage(1));
    server.emit(&usage(2));
    assert!(within(Duration::from_secs(3), || recorded(&yard)
        .iter()
        .filter(|e| e.get("tokens").is_some())
        .count()
        == 2));
    let requests: Vec<_> = recorded(&yard)
        .into_iter()
        .filter(|e| e["kind"] == "request")
        .collect();
    assert_eq!(requests.len(), 3, "{requests:?}");
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.get("tokens").is_none())
            .count(),
        1
    );
    server.disconnect();
    assert!(within(
        Duration::from_secs(3),
        || state(&yard)["usage"]["active"]["codex-session"]["source"] == "unavailable"
    ));
    hook(&yard, &event(&yard, "SessionEnd"));
    assert!(within(Duration::from_secs(3), || second
        .0
        .try_wait()
        .unwrap()
        .is_some()));
}

#[test]
fn explicit_native_agents_keep_their_model_stage_and_open_submission_attribution() {
    let yard = Yard::new();
    let identity = [
        "--host",
        "codex",
        "--agent",
        "child",
        "--agent-type",
        "reviewer",
        "--send",
    ];
    let submitted = yard.write("codex-session",r#"[{"id":"review","kind":"stage","title":"Review","status":"active"},{"id":"plan","kind":"plan","stage":"review","title":"Review plan","markdown":"Inspect the change."}]"#,&identity);
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["child".into()],
        ..Scenario::default()
    });
    let page = Served::start(&yard, &[]);
    let mut input = event(&yard, "UserPromptSubmit");
    input["agent_id"] = json!("child");
    input["agent_type"] = json!("reviewer");
    hook(&yard, &input);
    let _worker = worker(&yard, page.port, peer.path());
    assert!(within(
        Duration::from_secs(3),
        || state(&yard)["usage"]["active"]["child"]["source"] == "available"
    ));
    let mut observed = usage(1);
    observed["params"]["threadId"] = json!("child");
    peer.emit(&observed);
    assert!(within(Duration::from_secs(3), || recorded(&yard)
        .iter()
        .any(|e| e.get("tokens").is_some())));
    let requests = recorded(&yard);
    let request = requests.iter().find(|e| e.get("tokens").is_some()).unwrap();
    assert_eq!(request["agent"], json!({"id":"child","type":"reviewer"}));
    assert_eq!(request["model"], "test-model");
    assert!(request["stage"].is_string());
    assert_eq!(request["submissions"], json!([submitted["submission"]]));
}
