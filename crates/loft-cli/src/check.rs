//! `loft check` (F-38): the reader, then the validator, then the linter, with an exit code a CI
//! job can gate on. In text, each error and finding is one line, errors before findings (AC-522,
//! AC-595); with `--format json`, one document on standard output (RQ-155).

use std::path::Path;
use std::process::ExitCode;

use loftspec::check::{Checked, check};
use loftspec::lint::Severity;
use loftspec_store::FileStore;
use serde_json::{Value, json};

use crate::json;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-check/1";

pub fn run(root: &Path, fail_on: Option<Severity>, as_json: bool) -> ExitCode {
    let files = match FileStore::open(root).and_then(|store| store.read_files()) {
        Ok(files) => files,
        Err(error) => {
            if as_json {
                json::print(&json!({
                    "schema": SCHEMA,
                    "errors": [json::error("store", &error.to_string(), &[])],
                    "findings": [],
                }));
            }
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let checked = check(&files);
    if as_json {
        json::print(&document(&checked));
    } else {
        print(&checked);
    }
    let failed_on = fail_on.is_some_and(|threshold| {
        checked
            .findings()
            .iter()
            .any(|finding| finding.severity >= threshold)
    });
    if checked.has_errors() || failed_on {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print(checked: &Checked) {
    match checked {
        Checked::Unread(errors) => {
            for error in errors {
                println!("{error}");
            }
        }
        Checked::Read {
            invalid,
            config,
            findings,
        } => {
            for error in invalid {
                println!("{error}");
            }
            for error in config {
                println!("{error}");
            }
            for finding in findings {
                println!("{finding}");
            }
        }
    }
}

fn document(checked: &Checked) -> Value {
    let (errors, findings): (Vec<Value>, Vec<Value>) = match checked {
        Checked::Unread(errors) => (json::read_errors(errors), Vec::new()),
        Checked::Read {
            invalid,
            config,
            findings,
        } => (
            json::validation_errors(invalid)
                .into_iter()
                .chain(config.iter().map(|e| json::read_error("lint-config", e)))
                .collect(),
            findings.iter().map(json::finding).collect(),
        ),
    };
    json!({ "schema": SCHEMA, "errors": errors, "findings": findings })
}
