//! `loft fmt` (F-39): the tree rewritten in canonical layout with missing keys minted, or with
//! `--check` the drift reported and nothing written (RQ-140, RQ-142).

use std::path::Path;
use std::process::ExitCode;

use loftspec::codec::ReadError;
use loftspec::fmt::format;
use loftspec_store::{FileStore, StoreError};
use serde_json::{Value, json};

use crate::json;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-fmt/1";

/// What a run did: the files that change, the keys minted, and whether it wrote them.
#[derive(Default)]
struct Outcome {
    changed: Vec<String>,
    minted: usize,
    written: bool,
}

/// Why the tree was not formatted: its errors as JSON, and as the lines standard error shows.
struct Failure {
    errors: Vec<Value>,
    lines: Vec<String>,
}

impl From<StoreError> for Failure {
    fn from(error: StoreError) -> Self {
        let message = error.to_string();
        Self {
            errors: vec![json::error("store", &message, &[])],
            lines: vec![message],
        }
    }
}

impl From<Vec<ReadError>> for Failure {
    fn from(errors: Vec<ReadError>) -> Self {
        Self {
            errors: json::read_errors(&errors),
            lines: errors.iter().map(ToString::to_string).collect(),
        }
    }
}

pub fn run(root: &Path, check: bool, as_json: bool) -> ExitCode {
    let (outcome, failure) = match attempt(root, check) {
        Ok(outcome) => (outcome, None),
        Err(failure) => (Outcome::default(), Some(failure)),
    };
    if as_json {
        let errors = failure.as_ref().map_or_else(Vec::new, |f| f.errors.clone());
        json::print(&json!({
            "schema": SCHEMA,
            "changed": outcome.changed,
            "minted": outcome.minted,
            "written": outcome.written,
            "errors": errors,
        }));
    } else {
        for path in &outcome.changed {
            println!("{path}");
        }
    }
    match failure {
        Some(failure) => {
            crate::report(&failure.lines);
            ExitCode::FAILURE
        }
        None if check && !outcome.changed.is_empty() => ExitCode::FAILURE,
        None => ExitCode::SUCCESS,
    }
}

fn attempt(root: &Path, check: bool) -> Result<Outcome, Failure> {
    let store = FileStore::open(root)?;
    let formatted = format(&store.read_files()?)?;
    let changed = formatted.changed.keys().map(ToString::to_string).collect();
    let written = !check && !formatted.changed.is_empty();
    if written {
        store.write_files(formatted.changed)?;
    }
    Ok(Outcome {
        changed,
        minted: formatted.minted,
        written,
    })
}
