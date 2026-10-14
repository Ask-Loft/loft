//! `loft verify` (F-42): the roots recomputed from the tree and compared with the lock, with an
//! exit code a CI job can gate on (RQ-148). A tree that does not read compares no root (RQ-149).

use std::path::Path;
use std::process::ExitCode;

use loftspec::canonical::Digest;
use loftspec::publish::{LOCK, ReadTree, Root, Unverified, verify};
use loftspec_store::{FileStore, StoreError};
use serde_json::json;

use crate::json;
use crate::stop::Stop;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-verify/1";

/// Why the tree did not verify: an error that stops it, or roots that differ.
enum Failure {
    Stopped(Stop),
    Mismatch {
        root: Root,
        lock: Digest,
        tree: Digest,
    },
}

impl From<StoreError> for Failure {
    fn from(error: StoreError) -> Self {
        // An unreadable lock is a `lock` error, any other store error a `store` error.
        let kind = if matches!(error, StoreError::Lock(..)) {
            "lock"
        } else {
            "store"
        };
        Self::Stopped(Stop::one(kind, error.to_string()))
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
        Err(Failure::Stopped(stop)) => stop.exit(SCHEMA, as_json),
        Err(Failure::Mismatch { root, lock, tree }) => {
            if as_json {
                let fields = json!({
                    "root": format!("{}Root", root.name()),
                    "lock": lock.to_string(),
                    "tree": tree.to_string(),
                });
                json::print(&json::result(SCHEMA, "mismatch", fields, Vec::new()));
            }
            eprintln!("the {root} differs: {LOCK} has {lock}, the tree has {tree}");
            ExitCode::FAILURE
        }
    }
}

fn attempt(root: &Path) -> Result<Digest, Failure> {
    let store = FileStore::open(root)?;
    let files = store.read_files()?;
    let tree = ReadTree::read(&files).map_err(|errors| Failure::Stopped(errors.into()))?;
    let lock = store.lock()?.ok_or_else(|| {
        let message = format!("{LOCK}: there is no lock; `loft publish` writes one");
        Failure::Stopped(Stop::one("lock", message))
    })?;
    verify(&tree, &lock).map_err(|unverified| match unverified {
        Unverified::Invalid(errors) => Failure::Stopped(Stop::invalid(&errors)),
        Unverified::Mismatch { root, lock, tree } => Failure::Mismatch { root, lock, tree },
    })
}
