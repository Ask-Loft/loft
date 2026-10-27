//! The unchanged worker discovers decisions without an agent wait loop; policy time is simulated.
#![cfg(unix)]

use std::fs;
use std::process::Child;
use std::time::Duration;

use serde_json::{Value, json};
use software_plant::Clock;
use software_plant::codex::{CodexAppServer, NativeThread, Scenario};

use crate::common::EPOCH;
use crate::served::{Served, within};
use crate::yard::{ASK, Yard, document};
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

fn worker<'a>(yard: &'a Yard, page: &Served, peer: &CodexAppServer) -> Worker<'a> {
    let input = yard.scratch.home().join("notify-hook.json");
    fs::write(&input, event(yard, "SessionStart").to_string()).unwrap();
    let process = yard
        .scratch
        .command(Some(&yard.git), &Clock::fixed(EPOCH).passing())
        .args([
            "yard",
            "codex",
            "watch",
            "--input",
            input.to_str().unwrap(),
            "--port",
            &page.port.to_string(),
            "--app-server-socket",
            peer.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .spawn()
        .unwrap();
    Worker(process, yard)
}

fn peer(yard: &Yard) -> CodexAppServer {
    CodexAppServer::start(Scenario {
        loaded: vec!["codex-session".into()],
        threads: [(
            "codex-session".into(),
            NativeThread {
                directory: yard.scratch.0.canonicalize().unwrap().display().to_string(),
                ..NativeThread::default()
            },
        )]
        .into(),
        ..Scenario::default()
    })
}

fn answer(yard: &Yard) {
    let output = yard.run_with(
        &["answer", "--host", "codex", "--session", "codex-session"],
        r#"[{"block":"q1","labels":["Top"]}]"#,
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(document("answer", &output)["outcome"], "answered");
}

#[test]
fn an_answer_before_the_first_scan_wakes_its_caller_despite_another_readers_fallback() {
    let yard = Yard::new();
    let sent = yard.write("codex-session", ASK, &["--host", "codex", "--send"]);
    answer(&yard);
    yard.read("another-session", &["--host", "codex", "--mark-read"]);
    hook(&yard, &event(&yard, "SessionStart"));
    let page = Served::start(&yard, &[]);
    let peer = peer(&yard);
    let _worker = worker(&yard, &page, &peer);
    assert!(within(Duration::from_secs(3), || peer
        .requests()
        .iter()
        .any(|r| r["method"] == "turn/start")));
    let requests = peer.requests();
    let delivered = requests
        .iter()
        .find(|r| r["method"] == "turn/start")
        .unwrap();
    assert_eq!(delivered["params"]["threadId"], "codex-session");
    let text = delivered["params"]["input"][0]["text"].as_str().unwrap();
    assert!(text.contains(sent["submission"].as_str().unwrap()));
    assert!(text.contains("--host codex --session codex-session"));
    let metadata: Value = serde_json::from_str(
        text.lines()
            .nth(1)
            .unwrap()
            .strip_prefix("Yard has a resolved submission: ")
            .unwrap(),
    )
    .unwrap();
    let read = yard.read("codex-session", &["--host", "codex", "--submissions"]);
    assert_eq!(metadata["yardLog"], read["yardLog"]["key"]);
    assert_eq!(metadata["tree"], read["tree"]);
    assert_eq!(metadata["branch"], read["branch"]);
    assert_eq!(metadata["submission"], sent["submission"]);
    assert_eq!(metadata["revision"], read["revision"]);
    assert!(!text.contains("Which shelf?"));
    assert!(!text.contains("Top"));
    assert!(
        yard.read("codex-session", &["--host", "codex", "--submissions"])["ownLastRead"].is_null()
    );
    assert!(!state(&yard).to_string().contains("Which shelf?"));
    assert_eq!(
        json!(
            peer.requests()
                .iter()
                .filter(|r| r["method"] == "turn/start")
                .count()
        ),
        1
    );
}

fn starts(peer: &CodexAppServer) -> usize {
    peer.requests()
        .iter()
        .filter(|r| r["method"] == "turn/start")
        .count()
}

fn scanned(peer: &CodexAppServer) -> bool {
    peer.requests()
        .iter()
        .filter(|r| r["method"] == "thread/read")
        .count()
        >= 2
}

#[test]
fn a_decision_after_normal_turn_end_wakes_an_idle_caller_without_an_agent_wait_lease() {
    let yard = Yard::new();
    yard.write("codex-session", ASK, &["--host", "codex", "--send"]);
    hook(&yard, &event(&yard, "SessionStart"));
    hook(&yard, &event(&yard, "Stop"));
    let page = Served::start(&yard, &[]);
    let peer = peer(&yard);
    let _worker = worker(&yard, &page, &peer);
    assert!(within(Duration::from_secs(3), || scanned(&peer)));
    assert_eq!(starts(&peer), 0);
    answer(&yard);
    assert!(within(Duration::from_secs(3), || starts(&peer) == 1));
    assert_eq!(
        state(&yard)["notifications"]["callers"]["codex-session"]["armed"],
        true
    );
}

#[test]
fn a_lost_acknowledgement_is_checked_against_native_receipts_before_retrying_input() {
    let yard = Yard::new();
    yard.write("codex-session", ASK, &["--host", "codex", "--send"]);
    answer(&yard);
    hook(&yard, &event(&yard, "SessionStart"));
    let page = Served::start(&yard, &[]);
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["codex-session".into()],
        threads: [(
            "codex-session".into(),
            NativeThread {
                directory: yard.scratch.0.canonicalize().unwrap().display().to_string(),
                ..NativeThread::default()
            },
        )]
        .into(),
        drop_acknowledgements: vec!["turn/start".into()],
        ..Scenario::default()
    });
    let _worker = worker(&yard, &page, &peer);
    assert!(within(Duration::from_secs(3), || {
        state(&yard)["notifications"]["deliveries"]
            .as_object()
            .is_some_and(|d| d.values().any(|t| t["progress"] == "accepted"))
    }));
    assert_eq!(starts(&peer), 1);
    assert!(
        peer.requests()
            .iter()
            .any(|r| r["method"] == "thread/items/list")
    );
}

#[test]
fn acknowledged_delivery_survives_a_worker_restart_without_another_model_input() {
    let yard = Yard::new();
    yard.write("codex-session", ASK, &["--host", "codex", "--send"]);
    answer(&yard);
    hook(&yard, &event(&yard, "SessionStart"));
    let page = Served::start(&yard, &[]);
    let peer = peer(&yard);
    let mut first = worker(&yard, &page, &peer);
    assert!(within(Duration::from_secs(3), || {
        state(&yard)["notifications"]["deliveries"]
            .as_object()
            .is_some_and(|d| d.values().any(|t| t["progress"] == "accepted"))
    }));
    first.0.kill().unwrap();
    first.0.wait().unwrap();
    let before = peer.requests().len();
    let _second = worker(&yard, &page, &peer);
    assert!(within(Duration::from_secs(3), || peer.requests().len()
        >= before + 5));
    assert_eq!(starts(&peer), 1);
}

#[test]
fn an_unloaded_thread_keeps_its_decision_queued_without_reopening_codex() {
    let yard = Yard::new();
    yard.write("codex-session", ASK, &["--host", "codex", "--send"]);
    answer(&yard);
    hook(&yard, &event(&yard, "SessionStart"));
    let page = Served::start(&yard, &[]);
    let peer = CodexAppServer::start(Scenario::default());
    let _worker = worker(&yard, &page, &peer);
    assert!(within(Duration::from_secs(3), || peer
        .requests()
        .iter()
        .filter(|r| r["method"] == "thread/loaded/list")
        .count()
        >= 2));
    assert_eq!(starts(&peer), 0);
    assert!(
        !peer
            .requests()
            .iter()
            .any(|r| r["method"] == "thread/resume")
    );
    assert!(
        state(&yard)["notifications"]["deliveries"]
            .as_object()
            .unwrap()
            .values()
            .all(|t| t["progress"] == "queued")
    );
}

#[test]
fn an_already_read_resolution_is_not_sent_back_to_its_caller() {
    let yard = Yard::new();
    yard.write("codex-session", ASK, &["--host", "codex", "--send"]);
    answer(&yard);
    yard.read("codex-session", &["--host", "codex", "--mark-read"]);
    hook(&yard, &event(&yard, "SessionStart"));
    let page = Served::start(&yard, &[]);
    let peer = peer(&yard);
    let _worker = worker(&yard, &page, &peer);
    assert!(within(Duration::from_secs(3), || scanned(&peer)));
    assert_eq!(starts(&peer), 0);
    assert!(
        state(&yard)["notifications"]["deliveries"]
            .as_object()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn an_active_native_turn_receives_a_steer_without_any_configuration_overrides() {
    let yard = Yard::new();
    yard.write("codex-session", ASK, &["--host", "codex", "--send"]);
    answer(&yard);
    hook(&yard, &event(&yard, "SessionStart"));
    let page = Served::start(&yard, &[]);
    let peer = CodexAppServer::start(Scenario {
        loaded: vec!["codex-session".into()],
        threads: [(
            "codex-session".into(),
            NativeThread {
                directory: yard.scratch.0.canonicalize().unwrap().display().to_string(),
                active_turn: Some("working".into()),
                ..NativeThread::default()
            },
        )]
        .into(),
        ..Scenario::default()
    });
    let _worker = worker(&yard, &page, &peer);
    assert!(within(Duration::from_secs(3), || peer
        .requests()
        .iter()
        .any(|r| r["method"] == "turn/steer")));
    let requests = peer.requests();
    let params = &requests
        .iter()
        .find(|r| r["method"] == "turn/steer")
        .unwrap()["params"];
    assert_eq!(params["expectedTurnId"], "working");
    let mut keys: Vec<_> = params
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["clientUserMessageId", "expectedTurnId", "input", "threadId"]
    );
    assert_eq!(starts(&peer), 0);
}

#[test]
fn context_reports_delivery_status_and_late_cancelled_prompts_cannot_rearm_the_worker() {
    let yard = Yard::new();
    yard.write("codex-session", ASK, &["--host", "codex", "--send"]);
    answer(&yard);
    hook(&yard, &event(&yard, "SessionStart"));
    let page = Served::start(&yard, &[]);
    let peer = peer(&yard);
    let mut running = worker(&yard, &page, &peer);
    assert!(within(Duration::from_secs(3), || {
        state(&yard)["notifications"]["deliveries"]
            .as_object()
            .is_some_and(|d| d.values().any(|t| t["progress"] == "accepted"))
    }));
    running.0.kill().unwrap();
    running.0.wait().unwrap();
    let context = hook(&yard, &event(&yard, "SessionStart"));
    assert!(
        context["hookOutput"]["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("notifications are available")
    );
    let request = peer
        .requests()
        .into_iter()
        .find(|r| r["method"] == "turn/start")
        .unwrap();
    let mut prompt = event(&yard, "UserPromptSubmit");
    prompt["prompt"] = request["params"]["input"][0]["text"].clone();
    assert!(hook(&yard, &prompt)["hookOutput"].get("decision").is_none());
    let mut kept = state(&yard);
    for ticket in kept["notifications"]["deliveries"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        ticket["progress"] = json!("uncertain");
    }
    fs::write(
        yard.scratch
            .home()
            .join(".yard/codex/codex-session/state.json"),
        kept.to_string(),
    )
    .unwrap();
    let context = hook(&yard, &event(&yard, "SessionStart"));
    let text = context["hookOutput"]["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(text.contains("delivery is uncertain"));
    assert!(text.chars().count() <= 2000);
    hook(&yard, &event(&yard, "Interrupt"));
    assert_eq!(hook(&yard, &prompt)["hookOutput"]["decision"], "block");
    assert_eq!(
        state(&yard)["notifications"]["callers"]["codex-session"]["armed"],
        false
    );
    prompt["prompt"] = json!("Continue the work");
    let context = hook(&yard, &prompt);
    assert!(
        context["hookOutput"]["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("notifications are unavailable")
    );
    assert_eq!(
        state(&yard)["notifications"]["callers"]["codex-session"]["armed"],
        true
    );
    assert!(!state(&yard).to_string().contains("Continue the work"));
}

#[test]
fn an_unavailable_worker_recovers_its_queued_identity_when_the_endpoint_returns() {
    let yard = Yard::new();
    yard.write("codex-session", ASK, &["--host", "codex", "--send"]);
    answer(&yard);
    hook(&yard, &event(&yard, "SessionStart"));
    let page = Served::start(&yard, &[]);
    let unavailable = CodexAppServer::start(Scenario::default());
    let mut first = worker(&yard, &page, &unavailable);
    assert!(within(Duration::from_secs(3), || {
        state(&yard)["notifications"]["deliveries"]
            .as_object()
            .is_some_and(|d| !d.is_empty())
    }));
    first.0.kill().unwrap();
    first.0.wait().unwrap();
    let pending = state(&yard)["notifications"]["deliveries"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    assert_eq!(starts(&unavailable), 0);
    let returned = peer(&yard);
    let _second = worker(&yard, &page, &returned);
    assert!(within(Duration::from_secs(3), || starts(&returned) == 1));
    let requests = returned.requests();
    let delivered = requests
        .iter()
        .find(|r| r["method"] == "turn/start")
        .unwrap();
    assert_eq!(delivered["params"]["clientUserMessageId"], pending);
}
