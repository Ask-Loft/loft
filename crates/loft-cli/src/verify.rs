//! `loft verify` (F-42): the roots recomputed from the tree and compared with the lock, with an
//! exit code a CI job can gate on (RQ-148). A tree that does not read compares no root (RQ-149).

use std::path::Path;
use std::process::ExitCode;

use loftspec::canonical::Digest;
use loftspec::check::Placed;
use loftspec::codec::{self, ReadError};
use loftspec::publish::{LOCK, Lock, Root, Unverified, verify};
use loftspec_store::FileStore;
use serde_json::{Value, json};

use crate::json;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-verify/1";

/// Why the tree did not verify.
enum Failure {
    Read(Vec<ReadError>),
    Invalid(Vec<Placed>),
    Lock(String),
    Mismatch {
        root: Root,
        lock: Digest,
        tree: Digest,
    },
}

pub fn run(root: &Path, as_json: bool) -> ExitCode {
    match attempt(root) {
        Ok(state) => {
            if as_json {
                json::print(&json!({
                    "schema": SCHEMA,
                    "outcome": "verified",
                    "stateRoot": state.to_string(),
                    "errors": [],
                }));
            } else {
                println!("verified: state root {state}");
            }
            ExitCode::SUCCESS
        }
        Err(failure) => {
            if as_json {
                json::print(&document(&failure));
            }
            report(&failure);
            ExitCode::FAILURE
        }
    }
}

fn attempt(root: &Path) -> Result<Digest, Failure> {
    let store = FileStore::open(root).map_err(|error| Failure::Lock(error.to_string()))?;
    let files = store
        .read_files()
        .map_err(|error| Failure::Lock(error.to_string()))?;
    codec::read(&files).map_err(Failure::Read)?;
    let bytes = store
        .lock()
        .map_err(|error| Failure::Lock(error.to_string()))?
        .ok_or_else(|| {
            Failure::Lock(format!(
                "{LOCK}: there is no lock; `loft publish` writes one"
            ))
        })?;
    let lock = Lock::read(&bytes)
        .map_err(|error| Failure::Lock(format!("{LOCK}: not a valid lock record: {error}")))?;
    verify(&files, &lock).map_err(|unverified| match unverified {
        Unverified::Unread(errors) => Failure::Read(errors),
        Unverified::Invalid(errors) => Failure::Invalid(errors),
        Unverified::Mismatch { root, lock, tree } => Failure::Mismatch { root, lock, tree },
    })
}

fn report(failure: &Failure) {
    match failure {
        Failure::Read(errors) => crate::report(errors),
        Failure::Invalid(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
        }
        Failure::Lock(message) => eprintln!("{message}"),
        Failure::Mismatch { root, lock, tree } => {
            eprintln!("the {root} differs: {LOCK} has {lock}, the tree has {tree}");
        }
    }
}

fn document(failure: &Failure) -> Value {
    let errors: Vec<Value> = match failure {
        Failure::Read(errors) => errors.iter().map(|e| json::read_error("read", e)).collect(),
        Failure::Invalid(errors) => errors
            .iter()
            .map(|placed| json::error("validation", &placed.error.to_string(), &placed.sites))
            .collect(),
        Failure::Lock(message) => vec![json::error("lock", message, &[])],
        Failure::Mismatch { .. } => Vec::new(),
    };
    let mut document = json!({
        "schema": SCHEMA,
        "outcome": if matches!(failure, Failure::Mismatch { .. }) { "mismatch" } else { "error" },
        "errors": errors,
    });
    if let Failure::Mismatch { root, lock, tree } = failure {
        document["root"] = json!(root.field());
        document["lock"] = json!(lock.to_string());
        document["tree"] = json!(tree.to_string());
    }
    document
}
