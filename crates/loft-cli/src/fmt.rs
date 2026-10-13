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

impl Outcome {
    /// The outcome's name: `formatted` when files were rewritten, `drift` when `--check` finds a
    /// file that would change, `canonical` when none would.
    fn name(&self, check: bool) -> &'static str {
        match (self.changed.is_empty(), check) {
            (true, _) => "canonical",
            (false, true) => "drift",
            (false, false) => "formatted",
        }
    }
}

/// Why the tree was not formatted: its errors as JSON, and as the lines standard error shows.
struct Failure {
    errors: Vec<Value>,
    lines: Vec<String>,
}

impl From<StoreError> for Failure {
    fn from(error: StoreError) -> Self {
        Self {
            errors: vec![json::store_error(&error)],
            lines: vec![error.to_string()],
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
        json::print(&match &failure {
            Some(failure) => json::failure(SCHEMA, failure.errors.clone()),
            None => json::result(
                SCHEMA,
                outcome.name(check),
                json!({
                    "changed": outcome.changed,
                    "minted": outcome.minted,
                    "written": outcome.written,
                }),
                Vec::new(),
            ),
        });
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
