//! The Codex adapter: native hook input, CLI JSON for Yard, and a Codex-owned background worker.

mod commands;
mod context;
mod state;
mod tasks;
mod translate;
mod watch;

use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use crate::cli::Output;
use crate::finish::Finished;
use crate::json;

type Result<T> = std::result::Result<T, String>;

#[derive(Subcommand)]
pub enum Action {
    /// Read a native Codex hook from stdin and return its native response.
    Hook(Input),
    /// Run the session heartbeat and event delivery until Codex ends the hook or session.
    Watch(Input),
}

#[derive(Args)]
pub struct Input {
    /// The native hook object, or - for standard input.
    #[arg(long = "input", default_value = "-", value_name = "FILE")]
    source: PathBuf,
    /// The page server's port where no server runs yet.
    #[arg(long, default_value_t = super::args::DEFAULT_PORT)]
    port: u16,
    #[command(flatten)]
    output: Output,
}

pub fn run(action: Action) -> Finished {
    let (input, watching, schema) = match action {
        Action::Hook(input) => (input, false, "loft-yard-codex-hook/1"),
        Action::Watch(input) => (input, true, "loft-yard-codex-watch/1"),
    };
    let result = super::input::json(&input.source)
        .map_err(message)
        .and_then(translate::Hook::read)
        .and_then(|hook| {
            if watching {
                watch::run(&hook, input.port)
            } else {
                handle(&hook, input.port)
            }
        });
    match result {
        Ok(output) => {
            if input.output.json() {
                json::print(&json::result(
                    schema,
                    "handled",
                    json!({"hookOutput": output}),
                    vec![],
                ));
            } else {
                json::print(&output);
            }
            Finished::success("handled")
        }
        Err(message) => json::stop(
            schema,
            vec![json::error("codex-hook", &message, &[])],
            &message,
            input.output.json(),
        ),
    }
}

fn message(error: super::failure::Failure) -> String {
    match error {
        super::failure::Failure::Usage(message) => message,
        super::failure::Failure::Stopped(stop) => stop.lines.join("; "),
        super::failure::Failure::Refused { errors, .. } => json!(errors).to_string(),
    }
}

fn handle(hook: &translate::Hook, port: u16) -> Result<Value> {
    if !hook.supported() {
        return Ok(json!({}));
    }
    let files = state::Files::of(hook)?;
    // Serializes hooks and task mirrors for this session; the worker never holds this lock
    // while posting, so an unavailable page cannot delay a native tool.
    let _locked = files.lock()?;
    let mut state = files.read()?;
    let events = translate::events(hook, &mut state)?;
    state.keep(events);
    files.write(&state)?;
    let caller = hook.caller(&state);
    if let Some(caller) = &caller {
        tasks::mirror(hook, caller, port)?;
    }
    if matches!(hook.name(), "SessionStart" | "SubagentStart")
        && let Some(caller) = caller
    {
        return Ok(context::output(hook, &caller, port));
    }
    Ok(json!({}))
}
