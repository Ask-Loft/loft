//! Yard is reached in-process, through the same functions as the `loft yard` commands, and gives
//! the same JSON documents they print; only the page server runs as a command of its own.

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;
use yardlog::write::Context;
use yardlog::{Agent, YardLog};

use super::{Result, message};
use crate::cli::{Format, Output};
use crate::json;
use crate::yard::args::{Beyond, ReadArgs, ReadDetails, Runner};
use crate::yard::branch::Branch;
use crate::yard::failure::Failure;
use crate::yard::read as yard_read;
use crate::yard::task::{self, Change, Stop};
use crate::yard::working_tree::{Follows, target};

/// The host Codex's agents are named under.
pub const HOST: &str = "codex";

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Caller {
    pub session: String,
    pub agent: Option<(String, String)>,
}

impl Caller {
    /// The arguments that name the caller to a `loft yard` command.
    pub fn args(&self) -> Vec<String> {
        let mut args = vec![
            "--host".into(),
            HOST.into(),
            "--session".into(),
            self.session.clone(),
        ];
        if let Some((id, kind)) = &self.agent {
            args.extend([
                "--agent".into(),
                id.clone(),
                "--agent-type".into(),
                kind.clone(),
            ]);
        }
        args
    }

    /// The agent the caller is, as Yard Log names it.
    pub fn agent(&self) -> Agent {
        match &self.agent {
            Some((id, kind)) => Agent::sub_in(HOST, &self.session, id, kind),
            None => Agent::main_in(HOST, &self.session),
        }
    }

    /// The Codex thread the caller runs in: its subagent's, or the session's.
    pub fn thread(&self) -> &str {
        self.agent
            .as_ref()
            .map_or(self.session.as_str(), |(id, _)| id.as_str())
    }

    /// Whether `owner`, an agent as Yard Log renders it, is this caller.
    pub fn owns(&self, owner: &Value) -> bool {
        yardlog::identity::agent(owner).is_ok_and(|owner| owner.same(&self.agent()))
    }

    fn runner(&self) -> Runner {
        Runner {
            host: HOST.to_owned(),
            session: Some(self.session.clone()),
            agent: self.agent.as_ref().map(|(id, _)| id.clone()),
            agent_type: self.agent.as_ref().map(|(_, kind)| kind.clone()),
            tree: None,
        }
    }
}

/// The `loft yard` command of `args`, run in `dir` against the page server at `port`.
pub fn command(dir: &str, args: &[String], port: u16) -> Result<Command> {
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command
        .current_dir(dir)
        .arg("yard")
        .args(args)
        .args(["--port", &port.to_string(), "--format", "json"])
        .env("LOFT_TELEMETRY", "off")
        .env("LOFT_UPDATE_CHECK", "off")
        .stderr(Stdio::null());
    Ok(command)
}

const fn json_output() -> Output {
    Output {
        format: Some(Format::Json),
    }
}

/// A command's document from what it did: its result, a refusal with its errors, or an error.
fn document(
    schema: &str,
    outcome: &str,
    done: std::result::Result<Value, Failure>,
) -> Result<Value> {
    match done {
        Ok(fields) => Ok(json::result(schema, outcome, fields, Vec::new())),
        Err(Failure::Refused { fields, errors }) => {
            Ok(json::result(schema, "refused", fields, errors))
        }
        Err(failure) => Err(format!("Yard command failed: {}", message(failure))),
    }
}

/// The caller's briefing, as `loft yard read --context --mark-read` gives it: the read it shows
/// is recorded as the caller's last.
pub fn briefing(dir: &str, caller: &Caller, port: u16) -> Result<Value> {
    let done = yard_read::briefing_of(Path::new(dir), caller.runner(), port);
    document(yard_read::SCHEMA, "read", done)
}

/// Yard Log with every submission's lifecycle, as `loft yard read --submissions` gives it.
pub fn history(dir: &str, caller: &Caller, port: u16) -> Result<Value> {
    let args = ReadArgs {
        since: None,
        versions: false,
        details: ReadDetails {
            reports: false,
            submissions: true,
        },
        mark_read: false,
        context: false,
        agent: caller.runner(),
        beyond: Beyond { port },
        output: json_output(),
    };
    let done = yard_read::document(Path::new(dir), args);
    document(yard_read::SCHEMA, "read", done)
}

/// Changes the caller's tasks as `loft yard task` does: `change` applied to Yard Log as it
/// stands under its lock, with no read before it and no retry. A branch with no Yard Log
/// changes nothing and gives no value.
pub fn change_tasks<T>(
    dir: &str,
    caller: &Caller,
    port: u16,
    change: impl FnOnce(&mut YardLog, &Context) -> Change<T>,
) -> Result<Option<T>> {
    let agent = caller.agent();
    let changed = target(Path::new(dir), None, Some(&agent), Follows::Work)
        .and_then(|dir| Branch::locate(&dir, Beyond { port }))
        .and_then(|branch| task::under_lock(&branch, &agent, change));
    match changed.map_err(message)? {
        (_, Ok(value) | Err(Stop::Unchanged(value))) => Ok(Some(value)),
        (_, Err(Stop::Absent)) => Ok(None),
        (_, Err(Stop::Refused(refusal))) => Err(format!("Yard refused a task change: {refusal}")),
    }
}
