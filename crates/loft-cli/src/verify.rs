//! `loft verify` (F-42): the roots recomputed from the tree and compared with the lock, with an
//! exit code a CI job can gate on (RQ-148). A tree that does not read compares no root (RQ-149).

use std::path::Path;
use std::process::ExitCode;

use loftspec::canonical::Digest;
use loftspec::codec::ReadError;
use loftspec::publish::{LOCK, ReadTree, Unverified, verify};
use loftspec_store::{FileStore, StoreError};
use serde_json::{Value, json};

use crate::json;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-verify/1";

/// Why the tree did not verify.
enum Failure {
    Read(Vec<ReadError>),
    /// The tree or the lock cannot be read, by the JSON error kind that names which.
    Store(&'static str, String),
    Unverified(Unverified),
}

impl From<StoreError> for Failure {
    fn from(error: StoreError) -> Self {
        let kind = if matches!(error, StoreError::Lock(..)) {
            "lock"
        } else {
            "store"
        };
        Self::Store(kind, error.to_string())
    }
}

pub fn run(root: &Path, as_json: bool) -> ExitCode {
    match attempt(root) {
        Ok(state) => {
            if as_json {
                let fields = json!({ "stateRoot": state.to_string() });
                json::print(&json::result(SCHEMA, "verified", fields, Vec::new()));
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
    let store = FileStore::open(root)?;
    let files = store.read_files()?;
    let tree = ReadTree::read(&files).map_err(Failure::Read)?;
    let lock = store.lock()?.ok_or_else(|| {
        Failure::Store(
            "lock",
            format!("{LOCK}: there is no lock; `loft publish` writes one"),
        )
    })?;
    verify(&tree, &lock).map_err(Failure::Unverified)
}

fn report(failure: &Failure) {
    match failure {
        Failure::Read(errors) => crate::report(errors),
        Failure::Store(_, message) => eprintln!("{message}"),
        Failure::Unverified(Unverified::Invalid(errors)) => crate::report(errors),
        Failure::Unverified(Unverified::Mismatch { root, lock, tree }) => {
            eprintln!("the {root} differs: {LOCK} has {lock}, the tree has {tree}");
        }
    }
}

fn document(failure: &Failure) -> Value {
    let errors = match failure {
        Failure::Read(errors) => json::read_errors(errors),
        Failure::Store(kind, message) => vec![json::error(kind, message, &[])],
        Failure::Unverified(Unverified::Invalid(errors)) => json::validation_errors(errors),
        Failure::Unverified(Unverified::Mismatch { root, lock, tree }) => {
            let fields = json!({
                "root": format!("{}Root", root.name()),
                "lock": lock.to_string(),
                "tree": tree.to_string(),
            });
            return json::result(SCHEMA, "mismatch", fields, Vec::new());
        }
    };
    json::failure(SCHEMA, errors)
}
