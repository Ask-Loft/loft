//! Worker lifecycle and recovery against the actual local Yard page server.

use std::fs;
use std::process::Child;
use std::time::Duration;

use serde_json::{Value, json};
use yardlog_store::lock;

use crate::served::{Served, within};
use crate::yard::Yard;
use crate::yard_codex::{event, hook};

struct Worker(Child);

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn worker(yard: &Yard, input: &Value, port: u16) -> Worker {
    let path = yard.scratch.home().join(format!(
        "watch-{}.json",
        input["session_id"].as_str().unwrap()
    ));
    fs::write(&path, input.to_string()).unwrap();
    Worker(yard.spawn(&[
        "codex",
        "watch",
        "--input",
        path.to_str().unwrap(),
        "--port",
        &port.to_string(),
    ]))
}

fn events(yard: &Yard) -> Vec<Value> {
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
fn a_codex_worker_delivers_activity_and_recovers_a_stopped_page_server() {
    let yard = Yard::new();
    yard.write(
        "codex-session",
        r#"[{"id":"build","kind":"stage","title":"Build","status":"active"}]"#,
        &["--host", "codex"],
    );
    let mut served = Served::start(&yard, &[]);
    let input = event(&yard, "UserPromptSubmit");
    hook(&yard, &input);
    let mut running = worker(&yard, &input, served.port);
    assert!(within(Duration::from_secs(5), || !events(&yard).is_empty()));
    assert!(within(Duration::from_secs(3), || {
        let state = crate::yard_codex::state(&yard);
        state["events"].as_array().unwrap().is_empty()
            && state["in_flight"].as_array().unwrap().is_empty()
    }));
    assert_eq!(events(&yard)[0]["kind"], "turn.start");
    assert_eq!(
        yard.home_file("sessions.json")["codex~codex-session"],
        "2026-01-01T00:00:00Z"
    );
    let original = served.child.id();
    served.child.kill().unwrap();
    served.child.wait().unwrap();
    let mut end = event(&yard, "Stop");
    end["last_assistant_message"] = json!("PRIVATE MESSAGE");
    hook(&yard, &end);
    let recovered = within(Duration::from_secs(10), || events(&yard).len() == 2);
    assert!(
        recovered,
        "events: {:?}; state: {}; server: {}; worker: {:?}",
        events(&yard),
        crate::yard_codex::state(&yard),
        yard.home_file("server.json"),
        running.0.try_wait()
    );
    assert_ne!(yard.home_file("server.json")["pid"], original);
    assert_eq!(events(&yard)[1]["kind"], "turn.end");
    assert!(
        !serde_json::to_string(&events(&yard))
            .unwrap()
            .contains("PRIVATE")
    );
    hook(&yard, &event(&yard, "SessionEnd"));
    assert!(within(Duration::from_secs(3), || running
        .0
        .try_wait()
        .unwrap()
        .is_some()));
    assert!(!lock::is_held(&yard.scratch.home().join(".yard/server.lock")).unwrap());
}

#[test]
fn codex_workers_are_unique_per_session_and_one_session_end_leaves_the_other_running() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    let input = event(&yard, "UserPromptSubmit");
    hook(&yard, &input);
    let mut one = worker(&yard, &input, served.port);
    let held = yard
        .scratch
        .home()
        .join(".yard/codex/codex-session/worker.lock");
    assert!(within(Duration::from_secs(3), || held.exists()
        && lock::is_held(&held).unwrap()));
    let mut duplicate = worker(&yard, &input, served.port);
    assert!(within(Duration::from_secs(3), || duplicate
        .0
        .try_wait()
        .unwrap()
        .is_some()));
    let mut second = input.clone();
    second["session_id"] = json!("second-session");
    hook(&yard, &second);
    let mut two = worker(&yard, &second, served.port);
    let other_lock = yard
        .scratch
        .home()
        .join(".yard/codex/second-session/worker.lock");
    assert!(within(Duration::from_secs(3), || other_lock.exists()
        && lock::is_held(&other_lock).unwrap()));
    hook(&yard, &event(&yard, "SessionEnd"));
    assert!(within(Duration::from_secs(3), || one
        .0
        .try_wait()
        .unwrap()
        .is_some()));
    assert!(two.0.try_wait().unwrap().is_none());
    assert!(lock::is_held(&other_lock).unwrap());
    assert_eq!(served.get("/api/version").status, 200);
    second["hook_event_name"] = json!("SessionEnd");
    hook(&yard, &second);
    assert!(within(Duration::from_secs(3), || two
        .0
        .try_wait()
        .unwrap()
        .is_some()));
}
