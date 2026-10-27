//! Yard is reached through the same JSON commands as the other adapters, without a shell.

use std::io::Write as _;
use std::process::{Command, Stdio};

use serde_json::Value;

use super::Result;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Caller {
    pub session: String,
    pub agent: Option<(String, String)>,
}

impl Caller {
    pub fn args(&self) -> Vec<String> {
        let mut args = vec![
            "--host".into(),
            "codex".into(),
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

    pub fn owns(&self, owner: &Value) -> bool {
        owner["host"] == "codex"
            && owner["session"] == self.session
            && owner["agent"].as_str() == self.agent.as_ref().map(|(id, _)| id.as_str())
    }
}

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

pub fn run(dir: &str, args: &[String], port: u16, input: Option<&Value>) -> Result<Value> {
    let mut command = command(dir, args, port)?;
    command.stdout(Stdio::piped()).stdin(if input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    if let Some(input) = input {
        let written = child
            .stdin
            .take()
            .ok_or("no command input")?
            .write_all(input.to_string().as_bytes());
        if let Err(error) = written {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error.to_string());
        }
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    let document: Value = serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    if document["schema"] == Value::Null || document["outcome"] == "error" {
        return Err(format!("Yard command failed: {}", document["errors"]));
    }
    Ok(document)
}

pub fn read(dir: &str, caller: &Caller, port: u16) -> Result<Value> {
    let mut args = vec!["read".into()];
    args.extend(caller.args());
    run(dir, &args, port, None)
}

pub fn history(dir: &str, caller: &Caller, port: u16) -> Result<Value> {
    let mut args = vec!["read".into(), "--submissions".into()];
    args.extend(caller.args());
    run(dir, &args, port, None)
}

pub fn active<'a>(doc: &'a Value, caller: &Caller) -> Option<&'a Value> {
    doc["stages"].as_array()?.iter().find(|stage| {
        stage["version"]["body"]["status"] == "active" && caller.owns(&stage["owner"])
    })
}
