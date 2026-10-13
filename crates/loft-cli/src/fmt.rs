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
struct Outcome {
    changed: Vec<String>,
    minted: usize,
    written: bool,
}

/// Why the tree was not formatted.
enum Failure {
    Read(Vec<ReadError>),
    Store(String),
}

impl From<StoreError> for Failure {
    fn from(error: StoreError) -> Self {
        Self::Store(error.to_string())
    }
}

pub fn run(root: &Path, check: bool, as_json: bool) -> ExitCode {
    match attempt(root, check) {
        Ok(outcome) => {
            if as_json {
                json::print(&json!({
                    "schema": SCHEMA,
                    "changed": outcome.changed,
                    "minted": outcome.minted,
                    "written": outcome.written,
                    "errors": [],
                }));
            } else {
                for path in &outcome.changed {
                    println!("{path}");
                }
            }
            if check && !outcome.changed.is_empty() {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(failure) => {
            if as_json {
                json::print(&document(&failure));
            }
            match failure {
                Failure::Read(errors) => crate::report(&errors),
                Failure::Store(message) => eprintln!("{message}"),
            }
            ExitCode::FAILURE
        }
    }
}

fn attempt(root: &Path, check: bool) -> Result<Outcome, Failure> {
    let store = FileStore::open(root)?;
    let formatted = format(&store.read_files()?).map_err(Failure::Read)?;
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

fn document(failure: &Failure) -> Value {
    let errors = match failure {
        Failure::Read(errors) => json::read_errors(errors),
        Failure::Store(message) => vec![json::error("store", message, &[])],
    };
    json!({ "schema": SCHEMA, "changed": [], "minted": 0, "written": false, "errors": errors })
}
