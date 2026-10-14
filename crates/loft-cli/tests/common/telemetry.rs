//! What the telemetry tests share: runs of `loft` with telemetry on, sending to the Software
//! Plant's collector simulator, and the events it received.

use std::fs;
use std::process::{Command, Output};

use crate::common::{EPOCH, Scratch};
use crate::schema;
use serde_json::{Value, json};
use software_plant::{Clock, Collector, GitScenario};

/// The schema every event follows.
pub const EVENT_SCHEMA: &str = include_str!("../../../../schemas/telemetry-event.json");

/// The HTTP client a contract suite sends its scenarios with: an error status is an answer, not
/// a failure, and no scenario waits longer than ten seconds.
pub fn client() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build()
        .into()
}

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

/// Switches the update check on for `command`, as when `LOFT_UPDATE_CHECK` is unset, and points
/// it at the release feed at `address`.
pub fn asking<'a>(command: &'a mut Command, address: &str) -> &'a mut Command {
    command
        .env_remove("LOFT_UPDATE_CHECK")
        .env("LOFT_UPDATE_ENDPOINT", address)
}

/// Records that the notice was printed for the user of `scratch`, so that a run prints none.
pub fn seen(scratch: &Scratch) {
    let mut kept = scratch.setting("telemetry.json");
    kept["noticeShown"] = json!(true);
    fs::create_dir_all(scratch.config()).unwrap();
    fs::write(scratch.config().join("telemetry.json"), kept.to_string()).unwrap();
}

/// A switched concern as its tests run it: its variable, its subcommand and its schema.
pub struct Concern {
    pub variable: &'static str,
    pub command: &'static str,
    pub schema: &'static str,
}

/// Telemetry: `LOFT_TELEMETRY` and `loft telemetry`.
pub const TELEMETRY: Concern = Concern {
    variable: "LOFT_TELEMETRY",
    command: "telemetry",
    schema: include_str!("../../../../schemas/telemetry.json"),
};

/// The update check: `LOFT_UPDATE_CHECK` and `loft update-check`.
pub const UPDATE_CHECK: Concern = Concern {
    variable: "LOFT_UPDATE_CHECK",
    command: "update-check",
    schema: include_str!("../../../../schemas/update-check.json"),
};

impl Concern {
    /// Runs `loft <command> <args>` with the concern's variable set to `variable`, or unset.
    pub fn run(&self, scratch: &Scratch, variable: Option<&str>, args: &[&str]) -> Output {
        let mut command = scratch.command(None, &Clock::fixed(EPOCH));
        match variable {
            Some(value) => command.env(self.variable, value),
            None => command.env_remove(self.variable),
        };
        command.arg(self.command).args(args).output().unwrap()
    }

    /// The JSON document of `loft <command> <action> --format json`, checked against the
    /// concern's schema.
    pub fn report(&self, scratch: &Scratch, variable: Option<&str>, action: &str) -> Value {
        let output = self.run(scratch, variable, &[action, "--format", "json"]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        schema::conforming(&output.stdout, self.schema)
    }
}

/// The state a report gives: its outcome and what decided it.
pub fn state(document: &Value) -> (&str, &str) {
    (
        document["outcome"].as_str().unwrap(),
        document["decidedBy"].as_str().unwrap(),
    )
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
    sent_with(scratch, args, git, &[])
}

/// [`sent`], with each of `variables` set.
pub fn sent_with(
    scratch: &Scratch,
    args: &[&str],
    git: Option<&GitScenario>,
    variables: &[(&str, &str)],
) -> Sent {
    seen(scratch);
    let collector = Collector::accepting();
    let mut command = command(scratch, git, &collector);
    command.envs(variables.iter().copied());
    let output = command.args(args).output().unwrap();
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
