//! `loft yard claude hook` (the Claude Yard hook Feature): reads one hook object of Claude
//! Code's and answers it as Claude Code reads a command hook's output. The Loft plugin's command
//! hooks run it where Claude Code's built-in guard passes over a user's mod's hooks, and
//! everywhere else alike, so that it alone serves what the mod cannot hear there: the context as
//! a session or a subagent starts, the continuation of a session after `/clear` or a resume, the
//! main agent's plan mode, compactions and the page's decision of a plan.
//! A hook it does not serve, or one it cannot serve because Yard Log or the page server cannot be
//! reached, it answers with nothing, so that Claude Code goes on as it would without the plugin.

mod handoff;
mod hook;
mod mode;
mod plan;
mod store;

use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use crate::cli::Output;
use crate::finish::Finished;
use crate::json;
use hook::Hook;

const SCHEMA: &str = "loft-yard-claude-hook/1";

/// What the context adds to the briefing for Claude Code: the tool the agent reads with.
const FOOTER: &str = "Read Yard Log with `yard_read` before writing.";

/// The session start sources that give the context: every one but a compaction's.
const BRIEFED: [&str; 4] = ["startup", "resume", "clear", "fork"];

#[derive(Subcommand)]
pub enum Action {
    /// Read a Claude Code hook object and print Claude Code's output for it.
    Hook(Input),
}

#[derive(Args)]
pub struct Input {
    /// The hook object, or - for standard input.
    #[arg(long = "input", default_value = "-", value_name = "FILE")]
    source: PathBuf,
    /// The id of the Claude Code process that runs the hook, by which a session's end hands it on
    /// to the session start that continues it.
    #[arg(long, value_name = "PID")]
    host_pid: Option<u32>,
    /// The page server's port, named where no page server runs.
    #[arg(long, default_value_t = super::args::DEFAULT_PORT)]
    port: u16,
    #[command(flatten)]
    output: Output,
}

pub fn run(action: Action) -> Finished {
    let Action::Hook(input) = action;
    let answered = super::input::json(&input.source)
        .map_err(super::codex::message)
        .and_then(Hook::read)
        .map(|hook| answer(&hook, &input));
    match answered {
        Ok(output) => {
            if input.output.json() {
                json::print(&json::result(
                    SCHEMA,
                    "handled",
                    json!({"hookOutput": output}),
                    vec![],
                ));
            } else if output != json!({}) {
                json::print(&output);
            }
            Finished::success("handled")
        }
        Err(message) => json::stop(
            SCHEMA,
            vec![json::error("claude-hook", &message, &[])],
            &message,
            input.output.json(),
        ),
    }
}

/// Claude Code's output for `hook`: empty where the hook asks for none or cannot be served.
fn answer(hook: &Hook, input: &Input) -> Value {
    let Ok(at) = super::now() else {
        return json!({});
    };
    match hook.event() {
        "SessionStart" => {
            let source = hook.field("source").unwrap_or_default();
            if !BRIEFED.contains(&source) {
                return json!({});
            }
            if let Some(pid) = input.host_pid {
                let _ = handoff::take(hook, pid, at);
            }
            context(hook, input.port)
        }
        "SubagentStart" => context(hook, input.port),
        "SessionEnd" => {
            if let Some(pid) = input.host_pid {
                let _ = handoff::leave(hook, pid, at);
            }
            let _ = mode::forget(hook);
            json!({})
        }
        "PostCompact" => {
            post(&hook.main_event("compaction", at));
            json!({})
        }
        "PermissionRequest" => plan::decide(hook, input.port),
        event if mode::CARRY.contains(&event) => {
            if let Ok(Some(kind)) = mode::heard(hook) {
                post(&hook.main_event(kind, at));
            }
            json!({})
        }
        _ => json!({}),
    }
}

/// Posts an event to the page server that runs; an event no page server takes is dropped.
fn post(event: &Value) {
    if let Some(home) = yardlog_store::home::Home::from_env() {
        let _ = super::post::events(&home, event);
    }
}

/// The calling agent's briefing as context, followed by the line that tells it how to read Yard
/// Log; empty where Yard Log cannot be read.
fn context(hook: &Hook, port: u16) -> Value {
    let Ok(read) = super::read::briefing_of(hook.dir(), hook.runner(), port) else {
        return json!({});
    };
    let Some(text) = read["context"]["text"].as_str() else {
        return json!({});
    };
    json!({"hookSpecificOutput": {
        "hookEventName": hook.event(),
        "additionalContext": format!("{text}\n{FOOTER}"),
    }})
}
