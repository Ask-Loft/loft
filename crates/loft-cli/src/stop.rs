//! A command stopped on errors (F-45): each error as JSON for `--format json`, the lines standard
//! error shows, and exit 1. A read or validation error's line is
//! `<path>:<line>:<column>: <message>` (RQ-158).

use std::path::Path;

use loftspec::check::Placed;
use loftspec::codec::{Files, ReadError};
use loftspec::trace::{TRACE, TraceError};
use loftspec_store::{FileStore, StoreError};
use serde_json::Value;

use crate::finish::Finished;
use crate::json;
use crate::out::errln;

/// Why a command stopped: its errors as JSON, and the lines standard error shows.
pub struct Stop {
    pub errors: Vec<Value>,
    pub lines: Vec<String>,
}

impl Stop {
    /// One error of `kind`, its message also the line.
    pub fn one(kind: &str, message: String) -> Self {
        Self::all(kind, vec![message])
    }

    /// One error of `kind` for each message, each message also a line.
    pub fn all(kind: &str, messages: Vec<String>) -> Self {
        Self {
            errors: messages
                .iter()
                .map(|message| json::error(kind, message, &[]))
                .collect(),
            lines: messages,
        }
    }

    /// The validator's errors, each at the sites of the elements it names.
    pub fn invalid(errors: &[Placed]) -> Self {
        Self {
            errors: json::validation_errors(errors),
            lines: errors.iter().map(ToString::to_string).collect(),
        }
    }

    /// Prints the failure document under `schema` when JSON was asked for, and each line on
    /// standard error, and exits 1.
    pub fn exit(self, schema: &str, as_json: bool) -> Finished {
        let finished = Finished::stopped(&self.errors);
        if as_json {
            json::print(&json::failure(schema, self.errors));
        }
        for line in &self.lines {
            errln!("{line}");
        }
        finished
    }
}

impl From<StoreError> for Stop {
    fn from(error: StoreError) -> Self {
        Self::one("store", error.to_string())
    }
}

impl From<Vec<ReadError>> for Stop {
    fn from(errors: Vec<ReadError>) -> Self {
        Self {
            errors: json::read_errors(&errors),
            lines: errors.iter().map(ToString::to_string).collect(),
        }
    }
}

impl From<Vec<TraceError>> for Stop {
    fn from(errors: Vec<TraceError>) -> Self {
        Self {
            errors: errors
                .iter()
                .map(|error| {
                    let site = json::site_at(&TRACE, error.location);
                    json::error_at("trace", &error.problem.to_string(), vec![site])
                })
                .collect(),
            lines: errors.iter().map(ToString::to_string).collect(),
        }
    }
}

/// The files of the tree under `root`.
pub fn read_files(root: &Path) -> Result<Files, Stop> {
    Ok(FileStore::open(root)?.read_files()?)
}
