//! What the telemetry tests share: runs of `loft` with telemetry on, sending to the Software
//! Plant's collector simulator, and the events it received.

use std::fs;
use std::process::{Command, Output};

use crate::common::{EPOCH, Scratch};
use serde_json::{Value, json};
use software_plant::{Clock, Collector, GitScenario};

/// The schema every event follows.
pub const EVENT_SCHEMA: &str = include_str!("../../../../schemas/telemetry-event.json");

/// A run and the events the collector received from it.
pub struct Sent {
    pub output: Output,
    pub events: Vec<Value>,
}

/// Switches telemetry on for `command`, as when `LOFT_TELEMETRY` is unset, and points it at
/// `address`.
pub fn sending<'a>(command: &'a mut Command, address: &str) -> &'a mut Command {
    command
        .env_remove("LOFT_TELEMETRY")
        .env("LOFT_TELEMETRY_ENDPOINT", address)
}

/// Records that the notice was printed for the user of `scratch`, so that a run prints none.
pub fn seen(scratch: &Scratch) {
    let path = scratch.config().join("telemetry.json");
    let mut kept: Value = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| json!({}));
    kept["noticeShown"] = json!(true);
    fs::create_dir_all(scratch.config()).unwrap();
    fs::write(path, kept.to_string()).unwrap();
}

/// The `loft` command in `scratch` at [`EPOCH`], with git answering from `git` if given, and
/// telemetry on and sending to `collector`.
pub fn command(scratch: &Scratch, git: Option<&GitScenario>, collector: &Collector) -> Command {
    let mut command = scratch.command(git, &Clock::fixed(EPOCH));
    sending(&mut command, &collector.url());
    command
}

/// Runs `loft <args>` with telemetry on, with the notice already printed, and gives the events
/// an accepting collector received.
pub fn sent(scratch: &Scratch, args: &[&str], git: Option<&GitScenario>) -> Sent {
    seen(scratch);
    let collector = Collector::accepting();
    let output = command(scratch, git, &collector)
        .args(args)
        .output()
        .unwrap();
    Sent {
        output,
        events: events(&collector),
    }
}

/// The events `collector` accepted, each parsed.
pub fn events(collector: &Collector) -> Vec<Value> {
    collector
        .events()
        .iter()
        .map(|bytes| serde_json::from_slice(bytes).unwrap())
        .collect()
}

/// The one event of a run.
pub fn the_event(sent: &Sent) -> &Value {
    assert_eq!(sent.events.len(), 1, "{:?}", sent.output);
    &sent.events[0]
}
