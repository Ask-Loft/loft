//! `loft check` (F-38): the reader, then the validator, then the linter, with an exit code a CI
//! job can gate on. In text, each error and finding is one line, errors before findings (AC-522,
//! AC-595); with `--format json`, one document on standard output (RQ-155).

use std::path::Path;
use std::process::ExitCode;

use loftspec::check::{Checked, check};
use loftspec::lint::Severity;
use serde_json::{Value, json};

use crate::json;
use crate::stop;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-check/1";

pub fn run(root: &Path, fail_on: Option<Severity>, as_json: bool) -> ExitCode {
    let files = match stop::read_files(root) {
        Ok(files) => files,
        Err(stop) => return stop.exit(SCHEMA, as_json),
    };
    let checked = check(&files);
    let failed_on = fail_on.is_some_and(|threshold| {
        checked
            .findings()
            .iter()
            .any(|finding| finding.severity >= threshold)
    });
    let failed = checked.has_errors() || failed_on;
    if as_json {
        json::print(&document(
            &checked,
            if failed { "failed" } else { "passed" },
        ));
    } else {
        print(&checked);
    }
    if failed {
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

fn document(checked: &Checked, outcome: &str) -> Value {
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
    json::result(SCHEMA, outcome, json!({ "findings": findings }), errors)
}
