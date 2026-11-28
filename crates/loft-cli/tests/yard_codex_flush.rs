//! The Codex Yard worker flushes the outbox in the background: at each beat where an entry for its
//! address waits, it runs `loft yard flush` as a child with the address and the token Codex gave
//! it, and its heartbeat and its delivery of events go on while the flush runs.

use std::fs;
use std::time::Duration;

use serde_json::Value;
use software_plant::YardServer;
use software_plant::yard_server::TOKEN;

use crate::served::{Served, free_port, within};
use crate::yard::Yard;
use crate::yard_codex::{Worker, event, hook};
use crate::yard_connected::{STAGE_BATCH, connected, entries, write};

/// Starts `yard codex watch` on the hook `input` against the page server at `port`, with
/// `variables` in its environment, as Codex gives its hooks theirs.
fn worker<'a>(
    yard: &'a Yard,
    input: &Value,
    port: u16,
    variables: &[(&'static str, String)],
) -> Worker<'a> {
    let path = yard.scratch.home().join("watch-flush.json");
    fs::write(&path, input.to_string()).unwrap();
    let process = yard
        .scratch
        .command(Some(&yard.git), &yard.clock)
        .envs(variables.iter().cloned())
        .args(["yard", "codex", "watch", "--input", path.to_str().unwrap()])
        .args(["--port", &port.to_string(), "--format", "json"])
        .spawn()
        .unwrap();
    Worker::new(process, yard, input.clone())
}

#[test]
fn the_worker_flushes_at_its_first_beat_with_the_address_and_the_token_codex_gives_it() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    write(&yard, STAGE_BATCH, &variables);
    let input = event(&yard, "SessionStart");
    let _running = worker(&yard, &input, free_port(), &variables);
    assert!(within(Duration::from_secs(5), || !server
        .records()
        .is_empty()));
    let bearer = format!("Bearer {TOKEN}");
    assert_eq!(
        server.requests()[0].header("Authorization"),
        Some(bearer.as_str())
    );
    assert!(within(Duration::from_secs(2), || entries(&yard).is_empty()));
}

#[test]
fn an_entry_written_after_the_worker_started_is_flushed_at_its_next_beat() {
    let yard = Yard::new();
    let server = YardServer::accepting();
    let variables = connected(&server.url());
    let input = event(&yard, "SessionStart");
    let _running = worker(&yard, &input, free_port(), &variables);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(server.requests().len(), 0);
    write(&yard, STAGE_BATCH, &variables);
    assert!(within(Duration::from_secs(8), || !server
        .records()
        .is_empty()));
}

#[test]
fn the_worker_delivers_events_while_its_flush_waits_on_a_silent_yard_server() {
    let yard = Yard::new();
    yard.write("codex-session", STAGE_BATCH, &["--host", "codex"]);
    let silent = YardServer::silent();
    let variables = connected(&silent.url());
    write(&yard, &crate::yard_connected::notes(1), &variables);
    let served = Served::start(&yard, &[]);
    let input = event(&yard, "UserPromptSubmit");
    hook(&yard, &input);
    let _running = worker(&yard, &input, served.port, &variables);
    assert!(within(Duration::from_secs(5), || !silent
        .requests()
        .is_empty()));
    assert!(within(Duration::from_secs(5), || {
        let state = crate::yard_codex::state(&yard);
        state["events"].as_array().unwrap().is_empty()
            && state["in_flight"].as_array().unwrap().is_empty()
    }));
    assert_eq!(entries(&yard).len(), 1);
}
