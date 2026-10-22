//! How a `loft yard` command fails: a usage error, which exits 2 and names the argument at
//! fault; an error that stops it before it has Yard Log to act on, which prints the error
//! document and exits 1; and a refusal by Yard Log's rules, which prints the command's result
//! with the outcome `refused` and its errors, and exits 1.

use std::process::ExitCode;

use loft_ports::PortError;
use serde_json::{Map, Value, json};
use yardlog_store::YardStoreError;

use crate::finish::Finished;
use crate::json;
use crate::out::{errln, outln};
use crate::stop::Stop;

/// Why a command did not run to its result.
pub enum Failure {
    /// An argument does not match the command's input.
    Usage(String),
    /// An error that stops the command, each with its kind.
    Stopped(Stop),
    /// A refusal by Yard Log's rules: the result's fields, and the errors that say why.
    Refused { fields: Value, errors: Vec<Value> },
}

impl Failure {
    pub fn one(kind: &str, message: impl Into<String>) -> Self {
        Self::Stopped(Stop::one(kind, message.into()))
    }

    pub fn no_home() -> Self {
        Self::one(
            "home",
            "no home directory is known, so `~/.yard/` cannot be reached",
        )
    }

    /// Ends the command: the document `--format json` asked for, the lines on standard error,
    /// and the exit code.
    pub fn finish(self, schema: &str, as_json: bool) -> Finished {
        match self {
            Self::Usage(message) => {
                errln!("error: {message}");
                Finished::usage()
            }
            Self::Stopped(stop) => stop.exit(schema, as_json),
            Self::Refused { fields, errors } => refuse(schema, fields, errors, as_json),
        }
    }
}

/// Prints a refusal's result, or its reasons for a person, and exits 1.
fn refuse(schema: &str, fields: Value, errors: Vec<Value>, as_json: bool) -> Finished {
    let finished = Finished::with(ExitCode::FAILURE, "refused", &errors);
    let lines: Vec<String> = errors
        .iter()
        .filter_map(|error| error["message"].as_str())
        .map(ToOwned::to_owned)
        .collect();
    if as_json {
        json::print(&json::result(schema, "refused", fields, errors));
    } else {
        for line in &lines {
            outln!("refused: {line}");
        }
    }
    for line in lines {
        errln!("{line}");
    }
    finished
}

/// An error of `kind` with no site.
pub fn error(kind: &str, message: &str) -> Value {
    json::error(kind, message, &[])
}

/// An error of `kind` at a file of Yard Log, and the line and column where it has them.
pub fn error_at_file(kind: &str, message: &str, path: &str, at: Option<(usize, usize)>) -> Value {
    let mut site = Map::new();
    site.insert("path".to_owned(), json!(path));
    if let Some((line, column)) = at {
        site.insert("line".to_owned(), json!(line));
        site.insert("column".to_owned(), json!(column));
    }
    json::error_at(kind, message, vec![site])
}

/// The error a store's failure reports.
pub fn store_error(error: &YardStoreError) -> Value {
    let message = error.to_string();
    match error {
        YardStoreError::NotARepository(_) => self::error("repository", &message),
        YardStoreError::Detached(_) => self::error("detached", &message),
        YardStoreError::Branch(_) => self::error("branch", &message),
        YardStoreError::Unreadable { folder, error } => {
            let path = format!("{folder}/{}", error.file);
            let at = error
                .error
                .position
                .map(|position| (position.line, position.column));
            error_at_file("unreadable", &message, &path, at)
        }
        YardStoreError::Newer { .. } => self::error("newer", &message),
        YardStoreError::Answer { .. }
        | YardStoreError::Port(PortError::Git { .. } | PortError::GitUnavailable(_)) => {
            self::error("git", &message)
        }
        YardStoreError::Port(_) => self::error("store", &message),
    }
}

impl From<YardStoreError> for Failure {
    fn from(error: YardStoreError) -> Self {
        Self::Stopped(Stop {
            lines: vec![error.to_string()],
            errors: vec![store_error(&error)],
        })
    }
}
