//! The Codex adapter: native hook input, CLI JSON for Yard, and a Codex-owned background worker.

mod account;
mod app_server;
mod commands;
mod context;
mod notification_discovery;
mod notification_hooks;
mod notification_state;
mod notification_worker;
mod observer;
mod state;
mod task_bindings;
mod task_writes;
mod tasks;
mod translate;
mod usage;
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
    /// Observe an existing local app server at this absolute Unix-domain socket path.
    #[arg(long, value_name = "ABSOLUTE_PATH")]
    app_server_socket: Option<PathBuf>,
    #[command(flatten)]
    output: Output,
}

pub fn run(action: Action) -> Finished {
    let (input, watching, schema) = match action {
        Action::Hook(input) => (input, false, "loft-yard-codex-hook/1"),
        Action::Watch(input) => (input, true, "loft-yard-codex-watch/1"),
    };
    let result = app_server::endpoint(
        input.app_server_socket.as_deref(),
        std::env::var_os("CODEX_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
    .and_then(|socket| {
        super::input::json(&input.source)
            .map_err(message)
            .and_then(translate::Hook::read)
            .and_then(|hook| {
                if watching {
                    watch::run(&hook, input.port, socket.as_ref())
                } else {
                    handle(&hook, input.port)
                }
            })
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
    let locked = files.lock()?;
    let mut state = files.read()?;
    let caller = hook.caller(&state);
    let disposition = notification_hooks::record(hook, caller.as_ref(), &mut state.notifications);
    if disposition == notification_hooks::Disposition::Cancelled {
        files.write(&state)?;
        return Ok(
            json!({"decision":"block","reason":"This Yard notification was cancelled after native interruption or session end."}),
        );
    }
    let events = translate::events(hook, &mut state)?;
    if let Some(caller) = &caller {
        let key = task_writes::actor_key(caller);
        if matches!(
            hook.name(),
            "SessionStart" | "SubagentStart" | "UserPromptSubmit"
        ) {
            state.ended_tasks.remove(&key);
        } else if matches!(
            hook.name(),
            "Stop" | "Interrupt" | "SubagentStop" | "SessionEnd"
        ) {
            state.ended_tasks.insert(key);
        }
    }
    usage::native(hook, &mut state);
    state.keep(events);
    files.write(&state)?;
    // A task write Yard cannot take leaves the hook answered as it would be without Yard: the
    // native call already ran and its events are kept.
    if let Some(caller) = &caller {
        if matches!(
            hook.name(),
            "Stop" | "Interrupt" | "SubagentStop" | "SessionEnd"
        ) {
            let _ = task_writes::finish(hook, caller, port);
        } else if !state.ended_tasks.contains(&task_writes::actor_key(caller)) {
            let _ = tasks::mirror(hook, caller, port);
        }
    }
    drop(locked);
    if disposition != notification_hooks::Disposition::Notification {
        observer::readiness(&files, hook);
    }
    if matches!(
        hook.name(),
        "SessionStart" | "SubagentStart" | "UserPromptSubmit"
    ) && let Some(caller) = caller
    {
        return Ok(context::output(hook, &caller, port));
    }
    Ok(json!({}))
}
