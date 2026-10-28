//! How a `loft yard` command fails: a usage error, which exits 2 and names the argument at
//! fault; an error that stops it before it has Yard Log to act on, which prints the error
//! document and exits 1; and a refusal by Yard Log's rules, which prints the command's result
//! with the outcome `refused` and its errors, and exits 1.

use std::process::ExitCode;

use loft_ports::PortError;
use loftspec::codec::site_fields;
use loftspec::yaml::Location;
use serde_json::Value;
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

    /// Why a command that needs `~/.yard/` could not reach it.
    pub const NO_HOME: &str = "no home directory is known, so `~/.yard/` cannot be reached";

    pub fn no_home() -> Self {
        Self::one("home", Self::NO_HOME)
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

/// Ends a command: where it succeeded, the document `--format json` asked for, or for a person
/// the `line` its outcome and fields make, exiting 0; where it did not, as [`Failure::finish`].
pub fn end(
    schema: &str,
    as_json: bool,
    done: Result<(&'static str, Value), Failure>,
    line: impl FnOnce(&str, &Value) -> String,
) -> Finished {
    match done {
        Ok((outcome, fields)) => {
            if as_json {
                json::print(&json::result(schema, outcome, fields, Vec::new()));
            } else {
                outln!("{}", line(outcome, &fields));
            }
            Finished::success(outcome)
        }
        Err(failure) => failure.finish(schema, as_json),
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

/// The error a store's failure reports.
pub fn store_error(error: &YardStoreError) -> Value {
    let message = error.to_string();
    match error {
        YardStoreError::NotARepository(_) => self::error("repository", &message),
        YardStoreError::Detached(_) => self::error("detached", &message),
        YardStoreError::Branch(_) => self::error("branch", &message),
        YardStoreError::Session(_) => self::error("session", &message),
        YardStoreError::Unreadable { folder, error } => {
            let path = format!("{folder}/{}", error.file);
            let site = site_fields(&path, error.error.position.map(Location::At));
            json::error_at("unreadable", &message, vec![site])
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
