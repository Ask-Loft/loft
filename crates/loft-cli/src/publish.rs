//! `loft publish` (F-41): the publisher run over the tree inside a git repository. It prints
//! each label it allocated with its key, each label it withdrew, each citation of a withdrawn
//! label it rewrote, the new publish sequence and state root, and in a stopped merge or rebase
//! the git command that finishes it; that nothing changed; or why it refused (RQ-145). A
//! refusal writes nothing and exits 1. It runs no git command that changes the repository.

use std::path::Path;
use std::process::ExitCode;

use loftspec::check::Placed;
use loftspec::codec::ReadError;
use loftspec::publish::{Operation, Publication, PublishedAt, ReadTree, Refusal, publish};
use loftspec_store::clock::{Clock as _, SystemClock};
use loftspec_store::repository::{GitCommand, Repository as _};
use loftspec_store::{FileStore, StoreError};
use serde_json::{Value, json};

use crate::json;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-publish/2";

/// Why a publish did not happen.
enum Failure {
    Read(Vec<ReadError>),
    Invalid(Vec<Placed>),
    Refused(String),
}

impl<E: ToString> From<E> for Failure {
    fn from(error: E) -> Self {
        Self::Refused(error.to_string())
    }
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
/// writes what it gives in one save. In a stopped merge or rebase a lock that is not a lock
/// record, such as one holding conflict markers, is read as none.
fn attempt(root: &Path) -> Result<Publication, Failure> {
    let store = FileStore::open(root)?;
    let files = store.read_files()?;
    let tree = ReadTree::read(&files).map_err(Failure::Read)?;
    let repo = GitCommand::default().state(root)?;
    let previous = match store.lock() {
        Err(StoreError::Lock(..)) if repo.stopped.is_some() => None,
        lock => lock?,
    };
    let at = PublishedAt::from_unix_seconds(SystemClock::from_env().now()?)
        .map_err(|error| Failure::Refused(format!("SOURCE_DATE_EPOCH: {error}")))?;
    let publication = publish(tree, &repo, previous.as_ref(), at).map_err(refusal)?;
    if let Publication::Published { lock, files, .. } = &publication {
        store.publish(files.clone(), lock)?;
    }
    Ok(publication)
}

fn refusal(refusal: Refusal) -> Failure {
    match refusal {
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
        Refusal::Unmerged(paths) => Failure::Refused(format!(
            "unmerged paths other than spec/labels.yaml and loft.lock.json: {}; resolve them \
             before publishing",
            paths.join(", ")
        )),
        Refusal::UnmergedBindings => Failure::Refused(
            "spec/labels.yaml is unmerged but holds no conflict record: git could not run \
             `loft merge-driver`, which it runs only when `loft` is on git's search path. Put \
             `loft` there, run `git checkout -m spec/labels.yaml` to merge the file again, and \
             publish again"
                .to_owned(),
        ),
        Refusal::Lock { revision, message } => Failure::Refused(format!(
            "the lock at {revision} is not a lock record: {message}"
        )),
    }
}

fn print(publication: &Publication) {
    match publication {
        Publication::Published {
            lock,
            allocated,
            rewritten,
            withdrawn,
            finish,
            ..
        } => {
            for allocated in allocated {
                println!("{} {}", allocated.label, allocated.element.key());
            }
            for withdrawn in withdrawn {
                println!(
                    "withdrew {} from {}, which now holds {}",
                    withdrawn.label, withdrawn.element, withdrawn.live
                );
            }
            for rewritten in rewritten {
                println!(
                    "rewrote {} as {} in {}",
                    rewritten.withdrawn, rewritten.label, rewritten.element
                );
            }
            println!(
                "published sequence {}, state root {}",
                lock.publish_sequence, lock.state_root
            );
            if let Some(operation) = finish {
                println!("to finish: {}", finishing(*operation));
            }
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
            crate::report(errors);
            eprintln!(
                "publish refused: the validator reports {} errors",
                errors.len()
            );
        }
        Failure::Refused(reason) => eprintln!("publish refused: {reason}"),
    }
}

/// The git command that finishes a stopped merge or rebase once a publish settled it.
const fn finishing(operation: Operation) -> &'static str {
    match operation {
        Operation::Merge => "git add -A && git commit --no-edit",
        Operation::Rebase => "git add -A && git rebase --continue",
    }
}

fn published(publication: &Publication) -> Value {
    let (outcome, lock, allocated, rewritten) = match publication {
        Publication::Published {
            lock,
            allocated,
            rewritten,
            ..
        } => (
            "published",
            lock,
            allocated.as_slice(),
            rewritten.as_slice(),
        ),
        Publication::Unchanged(lock) => ("unchanged", lock, &[][..], &[][..]),
    };
    let rewritten: Vec<Value> = rewritten
        .iter()
        .map(|rewritten| {
            json!({
                "key": rewritten.element.to_string(),
                "withdrawn": rewritten.withdrawn.to_string(),
                "label": rewritten.label.to_string(),
            })
        })
        .collect();
    let allocated: Vec<Value> = allocated
        .iter()
        .map(|allocated| {
            json!({
                "label": allocated.label.to_string(),
                "key": allocated.element.key().to_string(),
            })
        })
        .collect();
    let (withdrawn, finish) = settled(publication);
    let mut document = json!({
        "schema": SCHEMA,
        "outcome": outcome,
        "allocated": allocated,
        "rewritten": rewritten,
        "withdrawn": withdrawn,
        "publishSequence": lock.publish_sequence.get(),
        "stateRoot": lock.state_root.to_string(),
        "errors": [],
    });
    if let Some(finish) = finish {
        document["finish"] = json!(finish);
    }
    document
}

/// The labels a publish withdrew, each with its key and the label the key now holds, and the
/// command that finishes the merge or rebase it settled.
fn settled(publication: &Publication) -> (Vec<Value>, Option<&'static str>) {
    let Publication::Published {
        withdrawn, finish, ..
    } = publication
    else {
        return (Vec::new(), None);
    };
    let withdrawn = withdrawn
        .iter()
        .map(|withdrawn| {
            json!({
                "key": withdrawn.element.to_string(),
                "withdrawn": withdrawn.label.to_string(),
                "label": withdrawn.live.to_string(),
            })
        })
        .collect();
    (withdrawn, finish.map(finishing))
}

fn refused(failure: &Failure) -> Value {
    let errors = match failure {
        Failure::Read(errors) => json::read_errors(errors),
        Failure::Invalid(errors) => json::validation_errors(errors),
        Failure::Refused(reason) => vec![json::error("refused", reason, &[])],
    };
    json!({
        "schema": SCHEMA,
        "outcome": "refused",
        "allocated": [],
        "rewritten": [],
        "withdrawn": [],
        "errors": errors,
    })
}
