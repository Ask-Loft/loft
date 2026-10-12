//! `loft publish` (F-41): the publisher run over the tree inside a git repository. It prints
//! each label it allocated with its key and the new publish sequence and state root, that nothing
//! changed, or why it refused (RQ-145). A refusal writes nothing and exits 1.

use std::path::Path;
use std::process::ExitCode;

use loftspec::check::Placed;
use loftspec::codec::{self, Files, ReadError};
use loftspec::publish::{LOCK, Lock, Publication, PublishedAt, Refusal, publish};
use loftspec_store::clock::{Clock as _, SystemClock};
use loftspec_store::repository::{GitCommand, Repository as _};
use loftspec_store::{FileStore, StoreError};
use serde_json::{Value, json};

use crate::json;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-publish/1";

/// Why a publish did not happen.
enum Failure {
    Read(Vec<ReadError>),
    Invalid(Vec<Placed>),
    Refused(String),
}

pub fn run(root: &Path, as_json: bool) -> ExitCode {
    match attempt(root) {
        Ok(publication) => {
            if as_json {
                json::print(&published(&publication));
            } else {
                print(&publication);
            }
            ExitCode::SUCCESS
        }
        Err(failure) => {
            if as_json {
                json::print(&refused(&failure));
            }
            report(&failure);
            ExitCode::FAILURE
        }
    }
}

/// Reads the tree, then the repository, the previous lock and the time, runs the publisher and
/// writes what it gives in one save.
fn attempt(root: &Path) -> Result<Publication, Failure> {
    let refused = |error: StoreError| Failure::Refused(error.to_string());
    let store = FileStore::open(root).map_err(refused)?;
    let files = store.read_files().map_err(refused)?;
    codec::read(&files).map_err(Failure::Read)?;
    let repo = GitCommand::default().state(root).map_err(refused)?;
    let previous = previous(&store)?;
    let seconds = SystemClock::from_env().now().map_err(refused)?;
    let at = PublishedAt::from_unix_seconds(seconds)
        .map_err(|error| Failure::Refused(format!("SOURCE_DATE_EPOCH: {error}")))?;
    let publication = decide(&files, &repo, previous.as_ref(), at)?;
    if let Publication::Published { lock, labels, .. } = &publication {
        store
            .publish(labels.clone(), lock.to_bytes())
            .map_err(refused)?;
    }
    Ok(publication)
}

fn previous(store: &FileStore) -> Result<Option<Lock>, Failure> {
    let bytes = store
        .lock()
        .map_err(|error| Failure::Refused(error.to_string()))?;
    bytes
        .map(|bytes| {
            Lock::read(&bytes).map_err(|error| {
                Failure::Refused(format!("{LOCK}: not a valid lock record: {error}"))
            })
        })
        .transpose()
}

fn decide(
    files: &Files,
    repo: &loftspec::publish::RepoState,
    previous: Option<&Lock>,
    at: PublishedAt,
) -> Result<Publication, Failure> {
    publish(files, repo, previous, at).map_err(|refusal| match refusal {
        Refusal::Unread(errors) => Failure::Read(errors),
        Refusal::Invalid(errors) => Failure::Invalid(errors),
        Refusal::Modified(paths) => Failure::Refused(format!(
            "uncommitted changes under spec/: {}; commit them before publishing",
            paths.join(", ")
        )),
        Refusal::Branch {
            current: Some(current),
            publish,
        } => Failure::Refused(format!(
            "on branch `{current}`, but the publish branch is `{publish}`"
        )),
        Refusal::Branch {
            current: None,
            publish,
        } => Failure::Refused(format!(
            "HEAD is detached, not on the publish branch `{publish}`"
        )),
    })
}

fn print(publication: &Publication) {
    match publication {
        Publication::Published {
            lock, allocated, ..
        } => {
            for allocated in allocated {
                println!("{} {}", allocated.label, allocated.element.key());
            }
            println!(
                "published sequence {}, state root {}",
                lock.publish_sequence, lock.state_root
            );
        }
        Publication::Unchanged(lock) => println!(
            "nothing changed since publish sequence {}, state root {}",
            lock.publish_sequence, lock.state_root
        ),
    }
}

fn report(failure: &Failure) {
    match failure {
        Failure::Read(errors) => crate::report(errors),
        Failure::Invalid(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            eprintln!(
                "publish refused: the validator reports {} errors",
                errors.len()
            );
        }
        Failure::Refused(reason) => eprintln!("publish refused: {reason}"),
    }
}

fn published(publication: &Publication) -> Value {
    let (outcome, lock, allocated) = match publication {
        Publication::Published {
            lock, allocated, ..
        } => ("published", lock, allocated.as_slice()),
        Publication::Unchanged(lock) => ("unchanged", lock, &[][..]),
    };
    let allocated: Vec<Value> = allocated
        .iter()
        .map(|allocated| {
            json!({
                "label": allocated.label.to_string(),
                "key": allocated.element.key().to_string(),
            })
        })
        .collect();
    json!({
        "schema": SCHEMA,
        "outcome": outcome,
        "allocated": allocated,
        "publishSequence": lock.publish_sequence.get(),
        "stateRoot": lock.state_root.to_string(),
        "errors": [],
    })
}

fn refused(failure: &Failure) -> Value {
    let errors: Vec<Value> = match failure {
        Failure::Read(errors) => errors.iter().map(|e| json::read_error("read", e)).collect(),
        Failure::Invalid(errors) => errors
            .iter()
            .map(|placed| json::error("validation", &placed.error.to_string(), &placed.sites))
            .collect(),
        Failure::Refused(reason) => vec![json::error("refused", reason, &[])],
    };
    json!({
        "schema": SCHEMA,
        "outcome": "refused",
        "allocated": [],
        "errors": errors,
    })
}
