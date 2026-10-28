//! Yard is reached in-process, through the same functions as the `loft yard` commands, and gives
//! the same JSON documents they print; only the page server runs as a command of its own.

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;
use yardlog::Agent;

use super::{Result, message};
use crate::cli::{Format, Output};
use crate::json;
use crate::yard::args::{Beyond, ReadArgs, ReadDetails, Runner, WriteArgs, Writer};
use crate::yard::failure::Failure;
use crate::yard::{read as yard_read, write as yard_write};

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
        }
    }

    fn writer(&self) -> Writer {
        Writer {
            host: HOST.to_owned(),
            session: self.session.clone(),
            agent: self.agent.as_ref().map(|(id, _)| id.clone()),
            agent_type: self.agent.as_ref().map(|(_, kind)| kind.clone()),
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

fn read_with(dir: &str, caller: &Caller, port: u16, submissions: bool) -> Result<Value> {
    let args = ReadArgs {
        since: None,
        versions: false,
        details: ReadDetails {
            reports: false,
            submissions,
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

/// Yard Log as `loft yard read` gives it to the caller.
pub fn read(dir: &str, caller: &Caller, port: u16) -> Result<Value> {
    read_with(dir, caller, port, false)
}

/// Yard Log with every submission's lifecycle, as `loft yard read --submissions` gives it.
pub fn history(dir: &str, caller: &Caller, port: u16) -> Result<Value> {
    read_with(dir, caller, port, true)
}

/// Writes `batch` as the caller, as `loft yard write --based-on <revision>` does.
pub fn write(dir: &str, caller: &Caller, port: u16, revision: u64, batch: &Value) -> Result<Value> {
    let args = WriteArgs {
        input: "-".into(),
        send: false,
        based_on: Some(revision),
        base: None,
        default_base: None,
        writer: caller.writer(),
        beyond: Beyond { port },
        output: json_output(),
    };
    let done = yard_write::document(Path::new(dir), args, || Ok(batch.clone()));
    document(yard_write::SCHEMA, "written", done)
}

pub fn active<'a>(doc: &'a Value, caller: &Caller) -> Option<&'a Value> {
    doc["stages"].as_array()?.iter().find(|stage| {
        stage["version"]["body"]["status"] == "active" && caller.owns(&stage["owner"])
    })
}
